use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    ChildBudgetPolicy, CoreModelError, SessionBudgetOverlay, SessionEvent, Task, TaskBudget,
    TaskEvent, TaskExecutionMode, WorkspaceContext,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    workspace: WorkspaceContext,
    pub overlay_budget: SessionBudgetOverlay,
    tasks: BTreeMap<String, Task>,
    events: Vec<SessionEvent>,
}

impl Session {
    pub fn new(
        session_id: impl Into<String>,
        workspace: WorkspaceContext,
        overlay_budget: SessionBudgetOverlay,
    ) -> Self {
        let session_id = session_id.into();
        Self {
            session_id: session_id.clone(),
            workspace: workspace.clone(),
            overlay_budget,
            tasks: BTreeMap::new(),
            events: vec![SessionEvent::SessionCreated {
                session_id,
                workspace,
            }],
        }
    }

    pub fn workspace(&self) -> &WorkspaceContext {
        &self.workspace
    }

    pub fn tasks(&self) -> impl Iterator<Item = &Task> {
        self.tasks.values()
    }

    pub fn task(&self, task_id: &str) -> Option<&Task> {
        self.tasks.get(task_id)
    }

    pub fn task_mut(&mut self, task_id: &str) -> Option<&mut Task> {
        self.tasks.get_mut(task_id)
    }

    pub fn session_events(&self) -> &[SessionEvent] {
        &self.events
    }

    pub fn spawn_task(
        &mut self,
        task_id: impl Into<String>,
        objective: impl Into<String>,
        budget: TaskBudget,
        execution_mode: TaskExecutionMode,
    ) -> Result<(), CoreModelError> {
        let task_id = task_id.into();
        if self.tasks.contains_key(&task_id) {
            return Err(CoreModelError::DuplicateTask {
                session_id: self.session_id.clone(),
                task_id,
            });
        }
        let task = Task::new(
            task_id.clone(),
            objective,
            self.workspace.clone(),
            budget,
            execution_mode,
        );
        self.tasks.insert(task_id, task);
        Ok(())
    }

    pub fn spawn_subtask(
        &mut self,
        parent_task_id: &str,
        child_task_id: impl Into<String>,
        objective: impl Into<String>,
        execution_mode: TaskExecutionMode,
        budget_policy: ChildBudgetPolicy,
        workspace_override: Option<WorkspaceContext>,
    ) -> Result<(), CoreModelError> {
        let child_task_id = child_task_id.into();
        if self.tasks.contains_key(&child_task_id) {
            return Err(CoreModelError::DuplicateTask {
                session_id: self.session_id.clone(),
                task_id: child_task_id,
            });
        }

        let child = {
            let parent =
                self.tasks
                    .get_mut(parent_task_id)
                    .ok_or_else(|| CoreModelError::UnknownTask {
                        session_id: self.session_id.clone(),
                        task_id: parent_task_id.to_owned(),
                    })?;
            parent.spawn_subtask(
                child_task_id.clone(),
                objective,
                execution_mode,
                budget_policy,
                workspace_override,
            )
        };

        self.tasks.insert(child_task_id, child);
        Ok(())
    }

    pub fn switch_worktree(&mut self, workspace: WorkspaceContext) -> Result<(), CoreModelError> {
        if self.workspace.repo_root != workspace.repo_root {
            return Err(CoreModelError::RepositoryMismatch {
                expected: self.workspace.repo_root.clone(),
                actual: workspace.repo_root,
            });
        }
        let previous_workspace = self.workspace.clone();
        self.workspace = workspace.clone();
        self.events.push(SessionEvent::WorktreeSwitched {
            session_id: self.session_id.clone(),
            previous_workspace,
            next_workspace: workspace,
        });
        Ok(())
    }

    pub fn fact_events(&self) -> Vec<&TaskEvent> {
        self.tasks
            .values()
            .flat_map(|task| task.fact_events().iter())
            .collect()
    }
}
