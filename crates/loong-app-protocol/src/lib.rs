#![forbid(unsafe_code)]

//! Transitional Phase 2 app-facing protocol spine.
//! Delete overlapping task/session orchestration inside legacy CLI surfaces once
//! Phase 3 retargets a real user path through this protocol layer.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub use loong_runtime::WorkspaceContext as AppProtocolWorkspaceContext;
use loong_runtime::{
    ChildBudgetPolicy, ExecutionArtifact, RuntimeExecutorRequest, RuntimeExecutorResult,
    RuntimeInteractiveExecution, RuntimeInteractiveExecutor, RuntimeInteractiveExecutorRequest,
    RuntimeInteractiveExecutorResult, RuntimeInteractiveRequest, RuntimeOneshotExecution,
    RuntimeOneshotExecutor, Session, SessionEvent, Task, TaskBudget, TaskEvent, TaskExecutionMode,
    TaskLifecycle, WorkspaceContext,
};
pub use loong_runtime::{
    RuntimeExecutorRequest as AppProtocolRuntimeExecutorRequest,
    RuntimeExecutorResult as AppProtocolRuntimeExecutorResult,
    RuntimeInteractiveExecutorRequest as AppProtocolRuntimeInteractiveExecutorRequest,
    RuntimeInteractiveExecutorResult as AppProtocolRuntimeInteractiveExecutorResult,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartTaskRequest {
    pub session_id: String,
    pub objective: String,
    pub budget: TaskBudget,
    pub execution_mode: TaskExecutionMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForkTaskRequest {
    pub session_id: String,
    pub parent_task_id: String,
    pub objective: String,
    pub execution_mode: TaskExecutionMode,
    pub budget_policy: ChildBudgetPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendInputRequest {
    pub session_id: String,
    pub task_id: String,
    pub input: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApproveRequest {
    pub session_id: String,
    pub task_id: String,
    pub checkpoint_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptRequest {
    pub session_id: String,
    pub task_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwitchWorktreeRequest {
    pub session_id: String,
    pub workspace: WorkspaceContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadArtifactsRequest {
    pub session_id: String,
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamEventsRequest {
    pub session_id: String,
    pub task_id: Option<String>,
    pub from_event_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppCommand {
    StartTask(StartTaskRequest),
    ResumeTask { session_id: String, task_id: String },
    ForkTask(ForkTaskRequest),
    SendInput(SendInputRequest),
    Approve(ApproveRequest),
    Interrupt(InterruptRequest),
    SwitchWorktree(SwitchWorktreeRequest),
    ReadArtifacts(ReadArtifactsRequest),
    ListTasks { session_id: String },
    ListSessions,
    StreamEvents(StreamEventsRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskProjection {
    pub task_id: String,
    pub parent_task_id: Option<String>,
    pub objective: String,
    pub lifecycle: TaskLifecycle,
    pub execution_mode: TaskExecutionMode,
    pub current_turn_id: Option<String>,
    pub artifact_count: usize,
    pub subtask_count: usize,
}

impl From<&Task> for TaskProjection {
    fn from(task: &Task) -> Self {
        Self {
            task_id: task.task_id.clone(),
            parent_task_id: task.parent_task_id.clone(),
            objective: task.objective.clone(),
            lifecycle: task.lifecycle().clone(),
            execution_mode: task.execution_mode.clone(),
            current_turn_id: task.current_turn().map(|turn| turn.turn_id.clone()),
            artifact_count: task.artifacts().len(),
            subtask_count: task.subtasks().len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionProjection {
    pub session_id: String,
    pub workspace: WorkspaceContext,
    pub tracked_task_count: usize,
    pub active_task_count: usize,
}

impl From<&Session> for SessionProjection {
    fn from(session: &Session) -> Self {
        let tracked_task_count = session.tasks().count();
        let active_task_count = session
            .tasks()
            .filter(|task| !task.lifecycle().is_terminal())
            .count();
        Self {
            session_id: session.session_id.clone(),
            workspace: session.workspace().clone(),
            tracked_task_count,
            active_task_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppResponse {
    Ack,
    Task(TaskProjection),
    Tasks(Vec<TaskProjection>),
    Session(SessionProjection),
    Sessions(Vec<SessionProjection>),
    SessionEvents(Vec<SessionEvent>),
    Events(Vec<TaskEvent>),
    Artifacts(Vec<ExecutionArtifact>),
    OneshotTurn(RuntimeOneshotExecution),
    InteractiveShell(RuntimeInteractiveExecution),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneshotTurnRequest {
    pub config_path: Option<String>,
    pub session_hint: Option<String>,
    pub message: String,
    pub acp: bool,
    pub acp_event_stream: bool,
    pub acp_bootstrap_mcp_servers: Vec<String>,
    pub acp_cwd: Option<String>,
}

#[async_trait]
pub trait AppProtocolOneshotExecutor: Send + Sync {
    async fn execute(
        &self,
        request: RuntimeExecutorRequest,
    ) -> Result<RuntimeExecutorResult, String>;
}

pub fn build_runtime_oneshot_request(
    request: &OneshotTurnRequest,
    workspace: WorkspaceContext,
) -> Result<loong_runtime::RuntimeOneshotRequest, String> {
    if request.message.trim().is_empty() {
        return Err("turn message must not be empty".to_owned());
    }

    Ok(loong_runtime::RuntimeOneshotRequest {
        session_hint: request.session_hint.clone(),
        message: request.message.clone(),
        workspace,
        acp: request.acp,
        acp_event_stream: request.acp_event_stream,
        acp_bootstrap_mcp_servers: request.acp_bootstrap_mcp_servers.clone(),
        acp_cwd: request.acp_cwd.clone(),
    })
}

struct RuntimeExecutorAdapter<'a, E> {
    inner: &'a E,
}

#[async_trait]
impl<E> RuntimeOneshotExecutor for RuntimeExecutorAdapter<'_, E>
where
    E: AppProtocolOneshotExecutor,
{
    async fn execute(
        &self,
        request: RuntimeExecutorRequest,
    ) -> Result<RuntimeExecutorResult, String> {
        self.inner.execute(request).await
    }
}

pub async fn execute_oneshot_turn<E: AppProtocolOneshotExecutor>(
    request: &OneshotTurnRequest,
    workspace: WorkspaceContext,
    executor: &E,
) -> Result<RuntimeOneshotExecution, String> {
    let runtime_request = build_runtime_oneshot_request(request, workspace)?;
    let adapter = RuntimeExecutorAdapter { inner: executor };
    loong_runtime::execute_oneshot_turn(&runtime_request, &adapter).await
}

pub fn render_oneshot_turn_output(execution: &RuntimeOneshotExecution) -> &str {
    execution.output_text.as_str()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveShellRequest {
    pub config_path: Option<String>,
    pub session_hint: Option<String>,
    pub acp: bool,
    pub acp_event_stream: bool,
    pub acp_bootstrap_mcp_servers: Vec<String>,
    pub acp_cwd: Option<String>,
}

#[async_trait]
pub trait AppProtocolInteractiveExecutor: Send + Sync {
    async fn run_interactive(
        &self,
        request: RuntimeInteractiveExecutorRequest,
    ) -> Result<RuntimeInteractiveExecutorResult, String>;
}

struct RuntimeInteractiveExecutorAdapter<'a, E> {
    inner: &'a E,
}

#[async_trait]
impl<E> RuntimeInteractiveExecutor for RuntimeInteractiveExecutorAdapter<'_, E>
where
    E: AppProtocolInteractiveExecutor,
{
    async fn run_interactive(
        &self,
        request: RuntimeInteractiveExecutorRequest,
    ) -> Result<RuntimeInteractiveExecutorResult, String> {
        self.inner.run_interactive(request).await
    }
}

pub fn build_runtime_interactive_request(
    request: &InteractiveShellRequest,
    workspace: WorkspaceContext,
) -> RuntimeInteractiveRequest {
    RuntimeInteractiveRequest {
        session_hint: request.session_hint.clone(),
        workspace,
        acp: request.acp,
        acp_event_stream: request.acp_event_stream,
        acp_bootstrap_mcp_servers: request.acp_bootstrap_mcp_servers.clone(),
        acp_cwd: request.acp_cwd.clone(),
    }
}

pub async fn execute_interactive_shell<E: AppProtocolInteractiveExecutor>(
    request: &InteractiveShellRequest,
    workspace: WorkspaceContext,
    executor: &E,
) -> Result<RuntimeInteractiveExecution, String> {
    let runtime_request = build_runtime_interactive_request(request, workspace);
    let adapter = RuntimeInteractiveExecutorAdapter { inner: executor };
    loong_runtime::execute_interactive_shell(&runtime_request, &adapter).await
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockExecutor;

    #[async_trait]
    impl AppProtocolOneshotExecutor for MockExecutor {
        async fn execute(
            &self,
            request: RuntimeExecutorRequest,
        ) -> Result<RuntimeExecutorResult, String> {
            assert_eq!(request.session_hint.as_deref(), Some("latest"));
            assert_eq!(request.message, "summarize via protocol");
            assert!(request.acp_event_stream);
            assert_eq!(
                request.acp_bootstrap_mcp_servers,
                vec!["filesystem".to_owned()]
            );
            Ok(RuntimeExecutorResult {
                session_id: "resolved-session".to_owned(),
                output_text: "protocol result".to_owned(),
                state: Some("ok".to_owned()),
                stop_reason: Some("completed".to_owned()),
                event_count: 1,
            })
        }
    }

    struct MockInteractiveExecutor;

    #[async_trait]
    impl AppProtocolInteractiveExecutor for MockInteractiveExecutor {
        async fn run_interactive(
            &self,
            request: RuntimeInteractiveExecutorRequest,
        ) -> Result<RuntimeInteractiveExecutorResult, String> {
            assert_eq!(request.session_hint.as_deref(), Some("latest"));
            Ok(RuntimeInteractiveExecutorResult {
                session_id: "interactive-session".to_owned(),
                exit_state: "completed".to_owned(),
            })
        }
    }

    #[tokio::test]
    async fn protocol_oneshot_turn_routes_into_runtime_execution() {
        let request = OneshotTurnRequest {
            config_path: None,
            session_hint: Some("latest".to_owned()),
            message: "summarize via protocol".to_owned(),
            acp: false,
            acp_event_stream: true,
            acp_bootstrap_mcp_servers: vec!["filesystem".to_owned()],
            acp_cwd: Some("/tmp/project".to_owned()),
        };
        let workspace = WorkspaceContext::new(
            "/tmp/project",
            "/tmp/project",
            "/tmp/project",
            "/tmp/project",
            "feature/phase3".to_owned(),
        );

        let execution = execute_oneshot_turn(&request, workspace.clone(), &MockExecutor)
            .await
            .expect("execute protocol oneshot turn");

        assert_eq!(execution.session.session_id, "resolved-session");
        assert_eq!(execution.session.workspace, workspace);
        assert_eq!(execution.output_text, "protocol result");
        assert_eq!(execution.task.task_id, "turn-run:resolved-session");
        assert_eq!(execution.task.lifecycle, TaskLifecycle::Completed);
    }

    #[tokio::test]
    async fn protocol_interactive_shell_routes_into_runtime_execution() {
        let request = InteractiveShellRequest {
            config_path: None,
            session_hint: Some("latest".to_owned()),
            acp: false,
            acp_event_stream: false,
            acp_bootstrap_mcp_servers: Vec::new(),
            acp_cwd: None,
        };
        let workspace = WorkspaceContext::new(
            "/tmp/chat",
            "/tmp/chat",
            "/tmp/chat",
            "/tmp/chat",
            "feature/chat".to_owned(),
        );

        let execution =
            execute_interactive_shell(&request, workspace.clone(), &MockInteractiveExecutor)
                .await
                .expect("execute interactive shell");

        assert_eq!(execution.session.session_id, "interactive-session");
        assert_eq!(execution.session.workspace, workspace);
        assert_eq!(execution.task.task_id, "chat-shell:interactive-session");
        assert_eq!(execution.task.lifecycle, TaskLifecycle::Completed);
        assert_eq!(execution.exit_state, "completed");
    }
}
