use serde::{Deserialize, Serialize};

use crate::ChildBudgetPolicy;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskExecutionMode {
    InteractiveAttached,
    DetachedBackground,
    DelegatedChild,
    RemoteControlledAttached,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnStatus {
    Running,
    WaitingOnTool,
    WaitingOnApproval,
    Completed,
    Failed,
}

impl TurnStatus {
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turn {
    pub turn_id: String,
    pub sequence: u32,
    pub summary: String,
    pub status: TurnStatus,
}

impl Turn {
    pub fn new(turn_id: impl Into<String>, sequence: u32, summary: impl Into<String>) -> Self {
        Self {
            turn_id: turn_id.into(),
            sequence,
            summary: summary.into(),
            status: TurnStatus::Running,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CancellationPolicy {
    CascadeFromParent,
    Independent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subtask {
    pub parent_task_id: String,
    pub task_id: String,
    pub budget_policy: ChildBudgetPolicy,
    pub cancellation: CancellationPolicy,
}
