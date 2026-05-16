use std::path::PathBuf;

use loong_core::{
    ApprovalState, ArtifactDurabilityClass, ChildBudgetPolicy, ExecutionArtifact,
    ExecutionArtifactKind, Session, SessionBudgetOverlay, Task, TaskBudget, TaskEvent,
    TaskExecutionMode, TaskLifecycle, TurnStatus, WorkspaceContext,
};

fn workspace(branch: &str, worktree_suffix: &str) -> WorkspaceContext {
    let repo_root = PathBuf::from("/tmp/loong");
    let worktree_root = repo_root.join(".worktrees").join(worktree_suffix);
    WorkspaceContext::new(
        worktree_root.clone(),
        repo_root,
        worktree_root.clone(),
        worktree_root.join("crates/loong-core"),
        branch.to_owned(),
    )
}

#[test]
fn lifecycle_transitions_emit_fact_events() {
    let mut task = Task::new(
        "task-1",
        "land the executable kernel object model",
        workspace("feat/phase-2", "phase-2"),
        TaskBudget::default(),
        TaskExecutionMode::InteractiveAttached,
    );

    assert_eq!(task.lifecycle(), &TaskLifecycle::Queued);

    let running = task.transition_to(TaskLifecycle::Running).unwrap();
    assert!(matches!(
        running,
        TaskEvent::LifecycleChanged {
            from: TaskLifecycle::Queued,
            to: TaskLifecycle::Running,
            ..
        }
    ));

    task.begin_turn("turn-1", "advance model and tool execution")
        .unwrap();
    task.finish_current_turn(TurnStatus::Completed).unwrap();
    task.transition_to(TaskLifecycle::WaitingForApproval)
        .unwrap();
    task.transition_to(TaskLifecycle::Running).unwrap();
    task.transition_to(TaskLifecycle::Completed).unwrap();

    assert_eq!(task.current_turn().unwrap().turn_id, "turn-1");
    assert_eq!(task.current_turn().unwrap().status, TurnStatus::Completed);
    assert_eq!(task.lifecycle(), &TaskLifecycle::Completed);
    assert!(
        task.transition_to(TaskLifecycle::Running).is_err(),
        "terminal tasks must reject further lifecycle transitions"
    );

    let lifecycle_events = task
        .fact_events()
        .iter()
        .filter(|event| matches!(event, TaskEvent::LifecycleChanged { .. }))
        .count();
    assert_eq!(lifecycle_events, 4);
}

#[test]
fn workspace_binding_is_durable_and_subtasks_inherit_context() {
    let first_workspace = workspace("feat/phase-2", "phase-2-a");
    let second_workspace = workspace("feat/phase-2-next", "phase-2-b");
    let split_budget = TaskBudget {
        max_child_tasks: 1,
        max_tool_concurrency: 1,
        max_steps: Some(3),
        deadline_epoch_ms: Some(55),
        artifact_retention: Default::default(),
        event_retention: Default::default(),
    };

    let mut session = Session::new(
        "session-1",
        first_workspace.clone(),
        SessionBudgetOverlay::default(),
    );

    session
        .spawn_task(
            "task-root",
            "own the first workspace binding",
            TaskBudget::default(),
            TaskExecutionMode::InteractiveAttached,
        )
        .unwrap();
    assert_eq!(
        session.task("task-root").unwrap().workspace(),
        &first_workspace
    );

    session.switch_worktree(second_workspace.clone()).unwrap();
    session
        .spawn_task(
            "task-detached",
            "run detached against the switched worktree",
            TaskBudget::default(),
            TaskExecutionMode::DetachedBackground,
        )
        .unwrap();
    session
        .spawn_subtask(
            "task-detached",
            "task-child",
            "inherit the task worktree while splitting budget",
            TaskExecutionMode::DelegatedChild,
            ChildBudgetPolicy::Split {
                child_budget: split_budget.clone(),
            },
            None,
        )
        .unwrap();

    let child = session.task("task-child").unwrap();
    assert_eq!(child.workspace(), &second_workspace);
    assert_eq!(child.budget(), &split_budget);
    assert_eq!(child.parent_task_id.as_deref(), Some("task-detached"));
    assert_eq!(session.task("task-detached").unwrap().subtasks().len(), 1);
}

#[test]
fn execution_artifacts_preserve_durability_contracts() {
    let mut task = Task::new(
        "task-2",
        "capture durable truth and projections separately",
        workspace("feat/artifacts", "artifacts"),
        TaskBudget::default(),
        TaskExecutionMode::DetachedBackground,
    );

    task.record_artifact(ExecutionArtifact::new(
        "patch-1",
        ExecutionArtifactKind::PatchEditRecord {
            paths: vec![PathBuf::from("crates/loong-core/src/task.rs")],
            summary: "add subtask lineage".to_owned(),
        },
    ));
    task.record_artifact(ExecutionArtifact::new(
        "approval-1",
        ExecutionArtifactKind::ApprovalCheckpoint {
            gate: "sandbox".to_owned(),
            state: ApprovalState::Approved,
            actor: Some("operator".to_owned()),
        },
    ));
    task.record_artifact(ExecutionArtifact::new(
        "assistant-1",
        ExecutionArtifactKind::AssistantTextOutput {
            text: "summarized completion note".to_owned(),
        },
    ));
    task.record_artifact(ExecutionArtifact::with_durability(
        "cache-1",
        ExecutionArtifactKind::GeneratedArtifactReference {
            location: PathBuf::from("/tmp/preview.txt"),
            description: "scratch preview".to_owned(),
        },
        ArtifactDurabilityClass::DiscardableCache,
    ));

    assert_eq!(task.artifacts().len(), 4);
    assert_eq!(task.artifacts().durable_truth().count(), 2);
    assert_eq!(task.artifacts().derived_projection().count(), 1);
    assert_eq!(task.artifacts().discardable_cache().count(), 1);

    let artifact_events = task
        .fact_events()
        .iter()
        .filter(|event| matches!(event, TaskEvent::ArtifactRecorded { .. }))
        .count();
    assert_eq!(artifact_events, 4);
}
