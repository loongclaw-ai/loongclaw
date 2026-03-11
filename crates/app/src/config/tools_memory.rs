use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::shared::{
    default_loongclaw_home, expand_path, validate_numeric_range, ConfigValidationIssue,
    DEFAULT_SQLITE_FILE,
};

pub(crate) const MIN_MEMORY_SLIDING_WINDOW: usize = 1;
pub(crate) const MAX_MEMORY_SLIDING_WINDOW: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    #[serde(default = "default_shell_allowlist")]
    pub shell_allowlist: Vec<String>,
    #[serde(default)]
    pub file_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_sqlite_path")]
    pub sqlite_path: String,
    #[serde(default = "default_sliding_window")]
    pub sliding_window: usize,
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            shell_allowlist: default_shell_allowlist(),
            file_root: None,
        }
    }
}

impl ToolConfig {
    pub fn resolved_file_root(&self) -> PathBuf {
        if let Some(path) = self.file_root.as_deref() {
            return expand_path(path);
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            sqlite_path: default_sqlite_path(),
            sliding_window: default_sliding_window(),
        }
    }
}

impl MemoryConfig {
    pub fn resolved_sqlite_path(&self) -> PathBuf {
        expand_path(&self.sqlite_path)
    }

    pub(super) fn validate(&self) -> Vec<ConfigValidationIssue> {
        let mut issues = Vec::new();
        if let Err(issue) = validate_numeric_range(
            "memory.sliding_window",
            self.sliding_window,
            MIN_MEMORY_SLIDING_WINDOW,
            MAX_MEMORY_SLIDING_WINDOW,
        ) {
            issues.push(*issue);
        }
        issues
    }
}

fn default_sqlite_path() -> String {
    default_loongclaw_home()
        .join(DEFAULT_SQLITE_FILE)
        .display()
        .to_string()
}

fn default_shell_allowlist() -> Vec<String> {
    vec![
        "echo".to_owned(),
        "cat".to_owned(),
        "ls".to_owned(),
        "pwd".to_owned(),
    ]
}

const fn default_sliding_window() -> usize {
    12
}
