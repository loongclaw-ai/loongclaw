use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceContext {
    pub workspace_root: PathBuf,
    pub repo_root: PathBuf,
    pub worktree_root: PathBuf,
    pub cwd: PathBuf,
    pub branch_identity: String,
}

impl WorkspaceContext {
    pub fn new(
        workspace_root: impl Into<PathBuf>,
        repo_root: impl Into<PathBuf>,
        worktree_root: impl Into<PathBuf>,
        cwd: impl Into<PathBuf>,
        branch_identity: impl Into<String>,
    ) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            repo_root: repo_root.into(),
            worktree_root: worktree_root.into(),
            cwd: cwd.into(),
            branch_identity: branch_identity.into(),
        }
    }

    pub fn same_repository(&self, other: &Self) -> bool {
        self.repo_root == other.repo_root
    }
}
