use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskLifecycle {
    Queued,
    Running,
    WaitingForApproval,
    WaitingForInput,
    Blocked,
    Completed,
    Failed,
    Cancelled,
}

impl TaskLifecycle {
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub const fn can_transition_to(&self, next: &Self) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::Running | Self::Cancelled)
                | (
                    Self::Running,
                    Self::WaitingForApproval
                        | Self::WaitingForInput
                        | Self::Blocked
                        | Self::Completed
                        | Self::Failed
                        | Self::Cancelled,
                )
                | (
                    Self::WaitingForApproval | Self::WaitingForInput | Self::Blocked,
                    Self::Running | Self::Failed | Self::Cancelled,
                )
        )
    }
}
