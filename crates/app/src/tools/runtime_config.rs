use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::OnceLock;

use super::policy_ext::ShellPolicyDefault;

/// Typed runtime configuration for tool executors.
///
/// Replaces per-call `std::env::var` lookups with a single read from a
/// process-wide singleton that is populated once at startup.
#[derive(Debug, Clone)]
pub struct ToolRuntimeConfig {
    pub file_root: Option<PathBuf>,
    pub shell_allow: BTreeSet<String>,
    pub shell_deny: BTreeSet<String>,
    pub shell_approval_required: BTreeSet<String>,
    pub shell_default_mode: ShellPolicyDefault,
}

impl Default for ToolRuntimeConfig {
    fn default() -> Self {
        Self {
            file_root: None,
            shell_allow: BTreeSet::new(),
            shell_deny: BTreeSet::new(),
            shell_approval_required: BTreeSet::new(),
            shell_default_mode: ShellPolicyDefault::Deny,
        }
    }
}

impl ToolRuntimeConfig {
    /// Build a config by reading the legacy environment variables.
    ///
    /// Keeps full backward compatibility for callers that still rely on
    /// `LOONGCLAW_FILE_ROOT`.
    pub fn from_env() -> Self {
        let file_root = std::env::var("LOONGCLAW_FILE_ROOT").ok().map(PathBuf::from);

        Self {
            file_root,
            ..Self::default()
        }
    }
}

static TOOL_RUNTIME_CONFIG: OnceLock<ToolRuntimeConfig> = OnceLock::new();

/// Initialise the process-wide tool runtime config.
///
/// Returns `Ok(())` on the first call.  Subsequent calls return
/// `Err` because the `OnceLock` rejects duplicate initialisation.
pub fn init_tool_runtime_config(config: ToolRuntimeConfig) -> Result<(), String> {
    TOOL_RUNTIME_CONFIG.set(config).map_err(|_err| {
        "tool runtime config already initialised (duplicate init_tool_runtime_config call)"
            .to_owned()
    })
}

/// Return the process-wide tool runtime config.
///
/// If `init_tool_runtime_config` was never called the config is lazily
/// populated from environment variables (backward-compat path).
pub fn get_tool_runtime_config() -> &'static ToolRuntimeConfig {
    TOOL_RUNTIME_CONFIG.get_or_init(ToolRuntimeConfig::from_env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_runtime_config_from_env_defaults() {
        let config = ToolRuntimeConfig::default();
        assert!(config.file_root.is_none());
    }

    #[test]
    fn file_root_uses_injected_config() {
        let config = ToolRuntimeConfig {
            file_root: Some(PathBuf::from("/tmp/test-root")),
            ..ToolRuntimeConfig::default()
        };
        assert_eq!(config.file_root, Some(PathBuf::from("/tmp/test-root")));
    }

    #[cfg(feature = "tool-shell")]
    #[test]
    fn injected_config_overrides_global() {
        let config = ToolRuntimeConfig {
            file_root: Some(PathBuf::from("/tmp/injected-root")),
            ..ToolRuntimeConfig::default()
        };
        let result = crate::tools::execute_tool_core_with_config(
            loongclaw_contracts::ToolCoreRequest {
                tool_name: "shell.exec".to_owned(),
                payload: serde_json::json!({"command": "echo", "args": ["injected"]}),
            },
            &config,
        );
        let outcome = result.expect("echo should be allowed with injected config");
        assert_eq!(outcome.status, "ok");
        assert!(
            outcome.payload["stdout"]
                .as_str()
                .unwrap()
                .contains("injected")
        );
    }
}
