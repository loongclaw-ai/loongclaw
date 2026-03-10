use std::path::PathBuf;
use std::sync::OnceLock;

/// Typed runtime configuration for the memory (SQLite) subsystem.
///
/// Mirrors [`crate::tools::runtime_config::ToolRuntimeConfig`] — a
/// process-wide singleton populated once at startup so that per-call
/// `std::env::var` lookups are avoided on the hot path.
#[derive(Debug, Clone, Default)]
pub struct MemoryRuntimeConfig {
    pub sqlite_path: Option<PathBuf>,
}

impl MemoryRuntimeConfig {
    /// Build a config by reading the legacy environment variable.
    ///
    /// Keeps full backward compatibility for callers that still rely on
    /// `LOONGCLAW_SQLITE_PATH`.
    pub fn from_env() -> Self {
        let sqlite_path = std::env::var("LOONGCLAW_SQLITE_PATH")
            .ok()
            .map(PathBuf::from);
        Self { sqlite_path }
    }
}

static MEMORY_RUNTIME_CONFIG: OnceLock<MemoryRuntimeConfig> = OnceLock::new();

/// Initialise the process-wide memory runtime config.
///
/// Returns `Ok(())` on the first call.  Subsequent calls return
/// `Err` because the `OnceLock` rejects duplicate initialisation.
pub fn init_memory_runtime_config(config: MemoryRuntimeConfig) -> Result<(), String> {
    MEMORY_RUNTIME_CONFIG.set(config).map_err(|_err| {
        "memory runtime config already initialised (duplicate init_memory_runtime_config call)"
            .to_owned()
    })
}

/// Return the process-wide memory runtime config.
///
/// If `init_memory_runtime_config` was never called the config is lazily
/// populated from environment variables (backward-compat path).
pub fn get_memory_runtime_config() -> &'static MemoryRuntimeConfig {
    MEMORY_RUNTIME_CONFIG.get_or_init(MemoryRuntimeConfig::from_env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_runtime_config_default_has_no_path() {
        let config = MemoryRuntimeConfig::default();
        assert!(config.sqlite_path.is_none());
    }

    #[test]
    fn explicit_path_overrides_default() {
        let config = MemoryRuntimeConfig {
            sqlite_path: Some(PathBuf::from("/tmp/test-memory.sqlite3")),
        };
        assert_eq!(
            config.sqlite_path,
            Some(PathBuf::from("/tmp/test-memory.sqlite3"))
        );
    }
}
