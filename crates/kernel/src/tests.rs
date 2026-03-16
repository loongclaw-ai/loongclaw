use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use proptest::prelude::*;
use serde_json::json;

use crate::audit::{AuditEventKind, InMemoryAuditSink};
use crate::clock::FixedClock;
use crate::contracts::{Capability, HarnessOutcome, TaskIntent};
use crate::errors::{KernelError, PolicyError};
use crate::kernel::LoongClawKernel;
use crate::policy::{PolicyEngine, StaticPolicyEngine};
use crate::task_supervisor::TaskSupervisor;
use crate::{Fault, TaskState};

use crate::test_support::*;

#[test]
fn pack_validation_rejects_invalid_semver() {
    let mut pack = sample_pack();
    pack.version = "version-one".to_owned();

    let error = pack.validate().expect_err("invalid semver should fail");
    assert!(matches!(error, crate::PackError::InvalidVersion(_)));
}

#[test]
fn token_generation_increments_on_each_issue() {
    let engine = StaticPolicyEngine::default();
    let pack = sample_pack();
    let t1 = engine.issue_token(&pack, "a1", 1_000_000, 3600).unwrap();
    let t2 = engine.issue_token(&pack, "a2", 1_000_000, 3600).unwrap();
    let t3 = engine.issue_token(&pack, "a3", 1_000_000, 3600).unwrap();
    assert_eq!(t1.generation, 1);
    assert_eq!(t2.generation, 2);
    assert_eq!(t3.generation, 3);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn prop_pack_capability_boundary_for_task_dispatch(
        pack_mask in 1_u16..(1_u16 << 9),
        required_mask in 0_u16..(1_u16 << 9)
    ) {
        let pack_capabilities = capability_set_from_mask(pack_mask);
        let required_capabilities = capability_set_from_mask(required_mask);

        let mut kernel = LoongClawKernel::new(StaticPolicyEngine::default());
        let mut pack = sample_pack();
        pack.granted_capabilities = pack_capabilities.clone();
        kernel
            .register_pack(pack)
            .expect("pack should register");
        kernel.register_harness_adapter(MockEmbeddedPiHarness {
            seen_tasks: Mutex::new(Vec::new()),
        });

        let token = kernel
            .issue_token("sales-intel", "agent-prop", 120)
            .expect("token should issue");

        let task = TaskIntent {
            task_id: "task-prop".to_owned(),
            objective: "property boundary check".to_owned(),
            required_capabilities: required_capabilities.clone(),
            payload: json!({}),
        };

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime should build");
        let result = runtime.block_on(kernel.execute_task("sales-intel", &token, task));

        if required_capabilities.is_subset(&pack_capabilities) {
            prop_assert!(result.is_ok());
        } else {
            let boundary_error = matches!(result, Err(KernelError::PackCapabilityBoundary { .. }));
            prop_assert!(boundary_error);
        }
    }
}

// ---------------------------------------------------------------------------
// Fault enum tests
// ---------------------------------------------------------------------------

#[test]
fn fault_display_is_human_readable() {
    let fault = Fault::CapabilityViolation {
        token_id: "tok-1".to_owned(),
        capability: Capability::InvokeTool,
    };
    let msg = fault.to_string();
    assert!(msg.contains("tok-1"));
    assert!(msg.contains("InvokeTool"));
}

#[test]
fn fault_from_policy_error_maps_expired_token() {
    let policy_err = PolicyError::ExpiredToken {
        token_id: "tok-2".to_owned(),
        expires_at_epoch_s: 1000,
    };
    let fault = Fault::from_policy_error(policy_err);
    assert!(
        matches!(fault, Fault::TokenExpired { token_id, expires_at_epoch_s } if token_id == "tok-2" && expires_at_epoch_s == 1000)
    );
}

#[test]
fn fault_from_policy_error_maps_missing_capability() {
    let policy_err = PolicyError::MissingCapability {
        token_id: "tok-3".to_owned(),
        capability: Capability::MemoryWrite,
    };
    let fault = Fault::from_policy_error(policy_err);
    assert!(matches!(fault, Fault::CapabilityViolation { .. }));
}

#[test]
fn fault_from_kernel_error_maps_policy() {
    let kernel_err = KernelError::Policy(PolicyError::RevokedToken {
        token_id: "tok-4".to_owned(),
    });
    let fault = Fault::from_kernel_error(kernel_err);
    assert!(matches!(fault, Fault::PolicyDenied { .. }));
}

#[test]
fn fault_from_kernel_error_maps_pack_boundary() {
    let kernel_err = KernelError::PackCapabilityBoundary {
        pack_id: "my-pack".to_owned(),
        capability: Capability::NetworkEgress,
    };
    let fault = Fault::from_kernel_error(kernel_err);
    assert!(matches!(fault, Fault::CapabilityViolation { .. }));
}

#[test]
fn fault_panic_carries_message() {
    let fault = Fault::Panic {
        message: "unexpected state".to_owned(),
    };
    assert!(fault.to_string().contains("unexpected state"));
}

// ── TaskState FSM tests ──────────────────────────────────────────────

#[test]
fn task_state_transitions_runnable_to_in_send() {
    let intent = TaskIntent {
        task_id: "t-1".to_owned(),
        objective: "test".to_owned(),
        required_capabilities: BTreeSet::from([Capability::InvokeTool]),
        payload: json!({}),
    };
    let state = TaskState::Runnable(intent);
    let next = state.transition_to_in_send();
    assert!(next.is_ok());
    assert!(matches!(next.unwrap(), TaskState::InSend { .. }));
}

#[test]
fn task_state_rejects_invalid_transition_from_completed() {
    let state = TaskState::Completed(HarnessOutcome {
        status: "ok".to_owned(),
        output: json!({}),
    });
    let err = state.transition_to_in_send();
    assert!(err.is_err());
}

#[test]
fn task_state_faulted_carries_fault() {
    let fault = Fault::Panic {
        message: "boom".to_owned(),
    };
    let state = TaskState::Faulted(fault.clone());
    if let TaskState::Faulted(f) = state {
        assert_eq!(f, fault);
    } else {
        panic!("expected Faulted");
    }
}

#[test]
fn task_state_full_transition_chain() {
    let intent = TaskIntent {
        task_id: "t-chain".to_owned(),
        objective: "chain test".to_owned(),
        required_capabilities: BTreeSet::from([Capability::InvokeTool]),
        payload: json!({}),
    };
    let state = TaskState::Runnable(intent);
    let state = state.transition_to_in_send().unwrap();
    assert!(matches!(state, TaskState::InSend { .. }));
    let state = state.transition_to_in_reply().unwrap();
    assert!(matches!(state, TaskState::InReply { .. }));
    let outcome = HarnessOutcome {
        status: "ok".to_owned(),
        output: json!({"result": "done"}),
    };
    let state = state.transition_to_completed(outcome).unwrap();
    assert!(matches!(state, TaskState::Completed(_)));
    assert!(state.is_terminal());
}

#[test]
fn task_state_faulted_from_non_terminal_succeeds() {
    let state = TaskState::InSend {
        task_id: "t-fault".to_owned(),
    };
    let fault = Fault::Panic {
        message: "oops".to_owned(),
    };
    let state = state.transition_to_faulted(fault);
    assert!(matches!(state, TaskState::Faulted(_)));
}

#[test]
fn task_state_faulted_from_terminal_is_noop() {
    let state = TaskState::Completed(HarnessOutcome {
        status: "ok".to_owned(),
        output: json!({}),
    });
    let fault = Fault::Panic {
        message: "late".to_owned(),
    };
    let state = state.transition_to_faulted(fault);
    // Should remain Completed, not change to Faulted
    assert!(matches!(state, TaskState::Completed(_)));
}

#[test]
fn task_supervisor_rejects_execute_after_completion() {
    let intent = TaskIntent {
        task_id: "t-double".to_owned(),
        objective: "test".to_owned(),
        required_capabilities: BTreeSet::from([Capability::InvokeTool]),
        payload: json!({}),
    };
    let mut supervisor = TaskSupervisor::new(intent);
    supervisor.force_state(TaskState::Completed(HarnessOutcome {
        status: "ok".to_owned(),
        output: json!({}),
    }));
    assert!(!supervisor.is_runnable());
}

#[test]
fn record_tool_call_denial_audits_extension_denied_errors() {
    let clock: Arc<FixedClock> = Arc::new(FixedClock::new(1_700_004_000));
    let audit = Arc::new(InMemoryAuditSink::default());
    let mut kernel =
        LoongClawKernel::with_runtime(StaticPolicyEngine::default(), clock, audit.clone());
    let pack = sample_pack();
    kernel
        .register_pack(pack.clone())
        .expect("pack should register");
    let token = kernel
        .issue_token(&pack.pack_id, "agent-extension-denied", 120)
        .expect("token should issue");
    let error = PolicyError::ExtensionDenied {
        extension: "policy".to_owned(),
        reason: "unexpected policy decision for tool `shell.exec`".to_owned(),
    };

    kernel
        .record_tool_call_denial(&pack, &token, 1_700_004_000, &error)
        .expect("audit record should succeed");

    let events = audit.snapshot();
    assert_eq!(events.len(), 2);
    assert!(matches!(
        &events[1].kind,
        AuditEventKind::AuthorizationDenied { pack_id, token_id, reason }
            if pack_id == &pack.pack_id
                && token_id == &token.token_id
                && reason.contains("unexpected policy decision for tool `shell.exec`")
    ));
}
