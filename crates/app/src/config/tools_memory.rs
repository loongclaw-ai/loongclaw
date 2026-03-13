use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::shared::{default_loongclaw_home, expand_path, DEFAULT_SQLITE_FILE};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolConfig {
    #[serde(default = "default_shell_allowlist")]
    pub shell_allowlist: Vec<String>,
    #[serde(default)]
    pub file_root: Option<String>,
    #[serde(default)]
    pub sessions: SessionToolConfig,
    #[serde(default)]
    pub messages: MessageToolConfig,
    #[serde(default)]
    pub delegate: DelegateToolConfig,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SessionVisibility {
    #[serde(rename = "self")]
    SelfOnly,
    #[default]
    #[serde(rename = "children")]
    Children,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionToolConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub visibility: SessionVisibility,
    #[serde(default = "default_session_list_limit")]
    pub list_limit: usize,
    #[serde(default = "default_session_history_limit")]
    pub history_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageToolConfig {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DelegateToolConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_delegate_max_depth")]
    pub max_depth: usize,
    #[serde(default = "default_delegate_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_delegate_child_tool_allowlist")]
    pub child_tool_allowlist: Vec<String>,
    #[serde(default)]
    pub allow_shell_in_child: bool,
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
            sessions: SessionToolConfig::default(),
            messages: MessageToolConfig::default(),
            delegate: DelegateToolConfig::default(),
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

impl Default for SessionToolConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            visibility: SessionVisibility::default(),
            list_limit: default_session_list_limit(),
            history_limit: default_session_history_limit(),
        }
    }
}

impl Default for MessageToolConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl Default for DelegateToolConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            max_depth: default_delegate_max_depth(),
            timeout_seconds: default_delegate_timeout_seconds(),
            child_tool_allowlist: default_delegate_child_tool_allowlist(),
            allow_shell_in_child: false,
        }
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

const fn default_enabled() -> bool {
    true
}

const fn default_session_list_limit() -> usize {
    100
}

const fn default_session_history_limit() -> usize {
    200
}

const fn default_delegate_max_depth() -> usize {
    1
}

const fn default_delegate_timeout_seconds() -> u64 {
    60
}

fn default_delegate_child_tool_allowlist() -> Vec<String> {
    vec!["file.read".to_owned(), "file.write".to_owned()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_config_defaults_enable_safe_session_and_delegate_policy() {
        let config = ToolConfig::default();
        assert!(config.sessions.enabled);
        assert_eq!(config.sessions.visibility, SessionVisibility::Children);
        assert_eq!(config.sessions.list_limit, 100);
        assert_eq!(config.sessions.history_limit, 200);
        assert!(!config.messages.enabled);
        assert!(!crate::tools::runtime_tool_view_for_config(&config).contains("sessions_send"));
        assert!(config.delegate.enabled);
        assert_eq!(config.delegate.max_depth, 1);
        assert_eq!(config.delegate.timeout_seconds, 60);
        assert!(!config.delegate.allow_shell_in_child);
        assert_eq!(
            config.delegate.child_tool_allowlist,
            vec!["file.read", "file.write",]
        );
    }

    #[cfg(feature = "config-toml")]
    #[test]
    fn tool_config_parses_children_visibility() {
        let raw = r#"
[tools.sessions]
visibility = "children"

[tools.delegate]
allow_shell_in_child = true
"#;
        let parsed =
            toml::from_str::<crate::config::LoongClawConfig>(raw).expect("parse tool config");
        assert_eq!(
            parsed.tools.sessions.visibility,
            SessionVisibility::Children
        );
        assert!(parsed.tools.delegate.allow_shell_in_child);
    }

    #[cfg(feature = "config-toml")]
    #[test]
    fn tool_config_parses_messages_enabled() {
        let raw = r#"
[tools.messages]
enabled = true
"#;
        let parsed =
            toml::from_str::<crate::config::LoongClawConfig>(raw).expect("parse tool config");
        assert!(crate::tools::runtime_tool_view_for_config(&parsed.tools).contains("sessions_send"));
        assert!(
            !crate::tools::delegate_child_tool_view_for_config(&parsed.tools)
                .contains("sessions_send")
        );
    }
}
