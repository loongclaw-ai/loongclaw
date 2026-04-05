use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use loongclaw_contracts::ToolCoreOutcome;
use serde_json::{Value, json};

use super::payload::{optional_payload_string, required_payload_string};
use crate::conversation::{
    ConstrainedSubagentContractView, ConstrainedSubagentHandle, ConstrainedSubagentIdentity,
    coordination_actions_for_subagent_handle,
};

#[cfg(test)]
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DelegateRequest {
    pub task: String,
    pub label: Option<String>,
    pub specialization: Option<String>,
    pub timeout_seconds: u64,
}

#[cfg(test)]
pub(crate) fn parse_delegate_request(payload: &Value) -> Result<DelegateRequest, String> {
    parse_delegate_request_with_default_timeout(payload, DEFAULT_TIMEOUT_SECONDS)
}

pub(crate) fn parse_delegate_request_with_default_timeout(
    payload: &Value,
    default_timeout_seconds: u64,
) -> Result<DelegateRequest, String> {
    let task = required_payload_string(payload, "task", "delegate tool")?;
    let label = optional_payload_string(payload, "label");
    let specialization = optional_payload_string(payload, "specialization");
    let timeout_seconds = payload
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(default_timeout_seconds);

    Ok(DelegateRequest {
        task,
        label,
        specialization,
        timeout_seconds,
    })
}

pub(crate) fn subagent_identity_for_delegate_request(
    request: &DelegateRequest,
) -> Option<ConstrainedSubagentIdentity> {
    let identity = ConstrainedSubagentIdentity {
        nickname: request.label.clone(),
        specialization: request.specialization.clone(),
    };
    (!identity.is_empty()).then_some(identity)
}

pub(crate) fn next_delegate_session_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("delegate:{now_ms:x}{counter:x}")
}

pub(crate) fn delegate_success_outcome(
    child_session_id: String,
    parent_session_id: Option<String>,
    label: Option<String>,
    subagent_contract: Option<&ConstrainedSubagentContractView>,
    final_output: String,
    turn_count: usize,
    duration_ms: u64,
) -> ToolCoreOutcome {
    let subagent = delegate_subagent_handle(
        child_session_id.clone(),
        parent_session_id,
        label.clone(),
        Some("completed".to_owned()),
        Some("completed".to_owned()),
        subagent_contract,
    );
    ToolCoreOutcome {
        status: "ok".to_owned(),
        payload: json!({
            "child_session_id": child_session_id,
            "label": label,
            "subagent_identity": subagent_contract.and_then(ConstrainedSubagentContractView::resolved_identity),
            "subagent_contract": subagent_contract,
            "subagent": subagent,
            "final_output": final_output,
            "turn_count": turn_count,
            "duration_ms": duration_ms,
        }),
    }
}

pub(crate) fn delegate_async_queued_outcome(
    child_session_id: String,
    parent_session_id: Option<String>,
    label: Option<String>,
    subagent_contract: Option<&ConstrainedSubagentContractView>,
    timeout_seconds: u64,
) -> ToolCoreOutcome {
    let subagent = delegate_subagent_handle(
        child_session_id.clone(),
        parent_session_id,
        label.clone(),
        Some("ready".to_owned()),
        Some("queued".to_owned()),
        subagent_contract,
    );
    ToolCoreOutcome {
        status: "ok".to_owned(),
        payload: json!({
            "child_session_id": child_session_id,
            "label": label,
            "subagent_identity": subagent_contract.and_then(ConstrainedSubagentContractView::resolved_identity),
            "subagent_contract": subagent_contract,
            "subagent": subagent,
            "mode": "async",
            "state": "queued",
            "timeout_seconds": timeout_seconds,
        }),
    }
}

pub(crate) fn delegate_timeout_outcome(
    child_session_id: String,
    parent_session_id: Option<String>,
    label: Option<String>,
    subagent_contract: Option<&ConstrainedSubagentContractView>,
    duration_ms: u64,
) -> ToolCoreOutcome {
    let subagent = delegate_subagent_handle(
        child_session_id.clone(),
        parent_session_id,
        label.clone(),
        Some("timed_out".to_owned()),
        Some("timed_out".to_owned()),
        subagent_contract,
    );
    ToolCoreOutcome {
        status: "timeout".to_owned(),
        payload: json!({
            "child_session_id": child_session_id,
            "label": label,
            "subagent_identity": subagent_contract.and_then(ConstrainedSubagentContractView::resolved_identity),
            "subagent_contract": subagent_contract,
            "subagent": subagent,
            "duration_ms": duration_ms,
            "error": "delegate_timeout",
        }),
    }
}

pub(crate) fn delegate_error_outcome(
    child_session_id: String,
    parent_session_id: Option<String>,
    label: Option<String>,
    subagent_contract: Option<&ConstrainedSubagentContractView>,
    error: String,
    duration_ms: u64,
) -> ToolCoreOutcome {
    let subagent = delegate_subagent_handle(
        child_session_id.clone(),
        parent_session_id,
        label.clone(),
        Some("failed".to_owned()),
        Some("failed".to_owned()),
        subagent_contract,
    );
    ToolCoreOutcome {
        status: "error".to_owned(),
        payload: json!({
            "child_session_id": child_session_id,
            "label": label,
            "subagent_identity": subagent_contract.and_then(ConstrainedSubagentContractView::resolved_identity),
            "subagent_contract": subagent_contract,
            "subagent": subagent,
            "duration_ms": duration_ms,
            "error": error,
        }),
    }
}

fn delegate_subagent_handle(
    child_session_id: String,
    parent_session_id: Option<String>,
    label: Option<String>,
    state: Option<String>,
    phase: Option<String>,
    subagent_contract: Option<&ConstrainedSubagentContractView>,
) -> ConstrainedSubagentHandle {
    let coordination = coordination_actions_for_subagent_handle(
        matches!(
            phase.as_deref().or(state.as_deref()),
            Some("completed" | "failed" | "timed_out")
        ),
        phase.as_deref().or(state.as_deref()),
        subagent_contract.and_then(|contract| contract.mode),
        false,
    );
    ConstrainedSubagentHandle::new(child_session_id)
        .with_parent_session_id(parent_session_id)
        .with_label(label)
        .with_state(state)
        .with_phase(phase)
        .with_contract(subagent_contract.cloned())
        .with_coordination(coordination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_delegate_request_requires_task() {
        let error =
            parse_delegate_request(&json!({})).expect_err("missing task should be rejected");
        assert!(error.contains("payload.task"), "error: {error}");
    }

    #[test]
    fn parse_delegate_request_uses_defaults() {
        let request = parse_delegate_request(&json!({
            "task": "research"
        }))
        .expect("delegate request");
        assert_eq!(request.task, "research");
        assert_eq!(request.label, None);
        assert_eq!(request.specialization, None);
        assert_eq!(request.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);
    }

    #[test]
    fn parse_delegate_request_includes_optional_specialization() {
        let request = parse_delegate_request(&json!({
            "task": "research",
            "label": "child",
            "specialization": "reviewer"
        }))
        .expect("delegate request");
        assert_eq!(request.specialization.as_deref(), Some("reviewer"));
        assert_eq!(
            subagent_identity_for_delegate_request(&request),
            Some(ConstrainedSubagentIdentity {
                nickname: Some("child".to_owned()),
                specialization: Some("reviewer".to_owned())
            })
        );
    }

    #[test]
    fn delegate_session_ids_use_expected_prefix() {
        let session_id = next_delegate_session_id();
        assert!(session_id.starts_with("delegate:"));
    }
}
