use std::collections::BTreeSet;
use std::sync::Mutex;

use loongclaw_kernel::test_support::*;
use loongclaw_kernel::*;
use serde_json::json;

#[tokio::test]
async fn integration_kernel_executes_task() {
    let mut kernel = LoongClawKernel::new(StaticPolicyEngine::default());
    kernel.register_pack(sample_pack()).unwrap();
    kernel.register_harness_adapter(MockEmbeddedPiHarness {
        seen_tasks: Mutex::new(Vec::new()),
    });
    let token = kernel
        .issue_token("sales-intel", "agent-alpha", 120)
        .unwrap();
    let task = TaskIntent {
        task_id: "t-1".to_owned(),
        objective: "test".to_owned(),
        required_capabilities: BTreeSet::from([Capability::InvokeTool]),
        payload: json!({}),
    };
    let dispatch = kernel
        .execute_task("sales-intel", &token, task)
        .await
        .unwrap();
    assert_eq!(dispatch.outcome.status, "ok");
}
