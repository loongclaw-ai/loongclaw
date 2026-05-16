#![forbid(unsafe_code)]

//! Transitional runtime spine.
//! Delete the temporary legacy turn executor adapter after later phases move
//! the live one-shot and interactive runtime execution fully under this crate.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub use loong_core::{
    ApprovalState, ArtifactDurabilityClass, ChildBudgetPolicy, DiagnosticSeverity,
    ExecutionArtifact, ExecutionArtifactKind, Session, SessionBudgetOverlay, SessionEvent, Task,
    TaskBudget, TaskEvent, TaskExecutionMode, TaskLifecycle, TurnStatus, WorkspaceContext,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeSurface {
    ProviderAdapters,
    DefaultCodingTools,
    BrowserAdapter,
    SessionStorage,
    ArtifactStorage,
    ProjectionCompaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSurfaceBinding {
    pub surface: RuntimeSurface,
    pub ownership_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSpine {
    pub core_contract: String,
    pub detached_execution_supported: bool,
    pub surfaces: Vec<RuntimeSurfaceBinding>,
}

impl Default for RuntimeSpine {
    fn default() -> Self {
        Self {
            core_contract: "loong-core".to_owned(),
            detached_execution_supported: true,
            surfaces: vec![
                RuntimeSurfaceBinding {
                    surface: RuntimeSurface::ProviderAdapters,
                    ownership_summary: "Official model/provider adapters stay above loong-core"
                        .to_owned(),
                },
                RuntimeSurfaceBinding {
                    surface: RuntimeSurface::DefaultCodingTools,
                    ownership_summary: "Default shell/file/browser tools live in the runtime layer"
                        .to_owned(),
                },
                RuntimeSurfaceBinding {
                    surface: RuntimeSurface::BrowserAdapter,
                    ownership_summary:
                        "Browser automation stays a runtime adapter, not a kernel concern"
                            .to_owned(),
                },
                RuntimeSurfaceBinding {
                    surface: RuntimeSurface::SessionStorage,
                    ownership_summary:
                        "Session truth storage binds durable facts without redefining the kernel"
                            .to_owned(),
                },
                RuntimeSurfaceBinding {
                    surface: RuntimeSurface::ArtifactStorage,
                    ownership_summary:
                        "Artifact references and durability storage are runtime-owned adapters"
                            .to_owned(),
                },
                RuntimeSurfaceBinding {
                    surface: RuntimeSurface::ProjectionCompaction,
                    ownership_summary:
                        "Projection compaction is optional runtime optimization only".to_owned(),
                },
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeOneshotRequest {
    pub session_hint: Option<String>,
    pub message: String,
    pub workspace: WorkspaceContext,
    pub acp: bool,
    pub acp_event_stream: bool,
    pub acp_bootstrap_mcp_servers: Vec<String>,
    pub acp_cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeExecutorRequest {
    pub session_hint: Option<String>,
    pub message: String,
    pub acp: bool,
    pub acp_event_stream: bool,
    pub acp_bootstrap_mcp_servers: Vec<String>,
    pub acp_cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeExecutorResult {
    pub session_id: String,
    pub output_text: String,
    pub state: Option<String>,
    pub stop_reason: Option<String>,
    pub event_count: usize,
}

#[async_trait]
pub trait RuntimeOneshotExecutor: Send + Sync {
    async fn execute(
        &self,
        request: RuntimeExecutorRequest,
    ) -> Result<RuntimeExecutorResult, String>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSessionDescriptor {
    pub session_id: String,
    pub workspace: WorkspaceContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeTaskDescriptor {
    pub task_id: String,
    pub objective: String,
    pub lifecycle: TaskLifecycle,
    pub execution_mode: TaskExecutionMode,
    pub current_turn_id: Option<String>,
    pub artifact_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeOneshotExecution {
    pub session: RuntimeSessionDescriptor,
    pub task: RuntimeTaskDescriptor,
    pub session_events: Vec<SessionEvent>,
    pub task_events: Vec<TaskEvent>,
    pub output_text: String,
    pub state: Option<String>,
    pub stop_reason: Option<String>,
    pub event_count: usize,
}

pub async fn execute_oneshot_turn<E: RuntimeOneshotExecutor>(
    request: &RuntimeOneshotRequest,
    executor: &E,
) -> Result<RuntimeOneshotExecution, String> {
    if request.message.trim().is_empty() {
        return Err("turn message must not be empty".to_owned());
    }

    let executor_result = executor
        .execute(RuntimeExecutorRequest {
            session_hint: request.session_hint.clone(),
            message: request.message.clone(),
            acp: request.acp,
            acp_event_stream: request.acp_event_stream,
            acp_bootstrap_mcp_servers: request.acp_bootstrap_mcp_servers.clone(),
            acp_cwd: request.acp_cwd.clone(),
        })
        .await?;

    let session_id = executor_result.session_id.clone();
    let task_id = format!("turn-run:{session_id}");
    let mut session = Session::new(
        session_id.clone(),
        request.workspace.clone(),
        SessionBudgetOverlay::default(),
    );
    session
        .spawn_task(
            task_id.as_str(),
            request.message.as_str(),
            TaskBudget::default(),
            TaskExecutionMode::RemoteControlledAttached,
        )
        .map_err(|error| error.to_string())?;
    let task = session
        .task_mut(task_id.as_str())
        .ok_or_else(|| "migrated runtime task missing after spawn".to_owned())?;
    task.transition_to(TaskLifecycle::Running)
        .map_err(|error| error.to_string())?;
    task.begin_turn("turn-1", "execute one-shot turn through the additive spine")
        .map_err(|error| error.to_string())?;
    task.record_artifact(ExecutionArtifact::new(
        "artifact-output-text",
        ExecutionArtifactKind::AssistantTextOutput {
            text: executor_result.output_text.clone(),
        },
    ));
    task.finish_current_turn(TurnStatus::Completed);
    task.transition_to(TaskLifecycle::Completed)
        .map_err(|error| error.to_string())?;

    let task = session
        .task(task_id.as_str())
        .ok_or_else(|| "migrated runtime task missing after completion".to_owned())?;

    Ok(RuntimeOneshotExecution {
        session: RuntimeSessionDescriptor {
            session_id,
            workspace: session.workspace().clone(),
        },
        task: RuntimeTaskDescriptor {
            task_id: task.task_id.clone(),
            objective: task.objective.clone(),
            lifecycle: task.lifecycle().clone(),
            execution_mode: task.execution_mode.clone(),
            current_turn_id: task.current_turn().map(|turn| turn.turn_id.clone()),
            artifact_count: task.artifacts().len(),
        },
        session_events: session.session_events().to_vec(),
        task_events: task.fact_events().to_vec(),
        output_text: executor_result.output_text,
        state: executor_result.state,
        stop_reason: executor_result.stop_reason,
        event_count: executor_result.event_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockExecutor {
        session_id: String,
        output_text: String,
    }

    #[async_trait]
    impl RuntimeOneshotExecutor for MockExecutor {
        async fn execute(
            &self,
            _request: RuntimeExecutorRequest,
        ) -> Result<RuntimeExecutorResult, String> {
            Ok(RuntimeExecutorResult {
                session_id: self.session_id.clone(),
                output_text: self.output_text.clone(),
                state: Some("ok".to_owned()),
                stop_reason: Some("completed".to_owned()),
                event_count: 0,
            })
        }
    }

    #[tokio::test]
    async fn runtime_oneshot_execution_builds_visible_core_truth() {
        let workspace = WorkspaceContext::new(
            "/tmp/phase3",
            "/tmp/phase3",
            "/tmp/phase3",
            "/tmp/phase3",
            "feature/phase3".to_owned(),
        );
        let request = RuntimeOneshotRequest {
            session_hint: Some("latest".to_owned()),
            message: "Summarize this repository.".to_owned(),
            workspace: workspace.clone(),
            acp: false,
            acp_event_stream: false,
            acp_bootstrap_mcp_servers: Vec::new(),
            acp_cwd: None,
        };
        let executor = MockExecutor {
            session_id: "resolved-session".to_owned(),
            output_text: "phase3 migrated reply".to_owned(),
        };

        let execution = execute_oneshot_turn(&request, &executor)
            .await
            .expect("execute oneshot turn");

        assert_eq!(execution.session.session_id, "resolved-session");
        assert_eq!(execution.session.workspace, workspace);
        assert_eq!(execution.task.task_id, "turn-run:resolved-session");
        assert_eq!(
            execution.task.execution_mode,
            TaskExecutionMode::RemoteControlledAttached
        );
        assert_eq!(execution.task.lifecycle, TaskLifecycle::Completed);
        assert_eq!(execution.task.current_turn_id.as_deref(), Some("turn-1"));
        assert_eq!(execution.task.artifact_count, 1);
        assert_eq!(execution.output_text, "phase3 migrated reply");
        assert!(matches!(
            execution.session_events.as_slice(),
            [SessionEvent::SessionCreated { .. }]
        ));
        assert!(
            execution
                .task_events
                .iter()
                .any(|event| matches!(event, TaskEvent::ArtifactRecorded { .. })),
            "task events should include artifact truth"
        );
    }
}
