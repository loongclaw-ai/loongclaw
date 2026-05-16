use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RetentionBudget {
    pub max_records: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskBudget {
    pub max_child_tasks: usize,
    pub max_tool_concurrency: usize,
    pub max_steps: Option<u64>,
    pub deadline_epoch_ms: Option<u64>,
    pub artifact_retention: RetentionBudget,
    pub event_retention: RetentionBudget,
}

impl Default for TaskBudget {
    fn default() -> Self {
        Self {
            max_child_tasks: 2,
            max_tool_concurrency: 1,
            max_steps: None,
            deadline_epoch_ms: None,
            artifact_retention: RetentionBudget::default(),
            event_retention: RetentionBudget::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBudgetOverlay {
    pub max_parallel_tasks: usize,
    pub max_parallel_child_tasks: usize,
    pub max_total_artifacts: Option<usize>,
    pub max_total_events: Option<usize>,
}

impl Default for SessionBudgetOverlay {
    fn default() -> Self {
        Self {
            max_parallel_tasks: 4,
            max_parallel_child_tasks: 4,
            max_total_artifacts: None,
            max_total_events: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChildBudgetPolicy {
    Inherit,
    Split {
        child_budget: TaskBudget,
    },
    Override {
        child_budget: TaskBudget,
        authorized_by: String,
    },
}

impl ChildBudgetPolicy {
    pub fn resolve(&self, parent_budget: &TaskBudget) -> TaskBudget {
        match self {
            Self::Inherit => parent_budget.clone(),
            Self::Split { child_budget } | Self::Override { child_budget, .. } => {
                child_budget.clone()
            }
        }
    }
}
