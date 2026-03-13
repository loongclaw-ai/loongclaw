use std::path::Path;

use crate::config::LoongClawConfig;

pub fn initialize_runtime_environment(
    config: &LoongClawConfig,
    resolved_config_path: Option<&Path>,
) {
    if let Some(path) = resolved_config_path {
        std::env::set_var("LOONGCLAW_CONFIG_PATH", path.display().to_string());
    }
    std::env::set_var(
        "LOONGCLAW_SQLITE_PATH",
        config.memory.resolved_sqlite_path().display().to_string(),
    );
    std::env::set_var(
        "LOONGCLAW_SLIDING_WINDOW",
        config.memory.sliding_window.to_string(),
    );
    std::env::set_var(
        "LOONGCLAW_SHELL_ALLOWLIST",
        config.tools.shell_allowlist.join(","),
    );
    std::env::set_var(
        "LOONGCLAW_FILE_ROOT",
        config.tools.resolved_file_root().display().to_string(),
    );

    let tool_rt = crate::tools::runtime_config::ToolRuntimeConfig {
        shell_allowlist: config
            .tools
            .shell_allowlist
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect(),
        file_root: Some(config.tools.resolved_file_root()),
    };
    let _ = crate::tools::runtime_config::init_tool_runtime_config(tool_rt);

    let memory_rt = crate::memory::runtime_config::MemoryRuntimeConfig {
        sqlite_path: Some(config.memory.resolved_sqlite_path()),
    };
    let _ = crate::memory::runtime_config::init_memory_runtime_config(memory_rt);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_path_env_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    #[test]
    fn initialize_runtime_environment_exports_config_path() {
        let _guard = config_path_env_lock().lock().expect("env lock");
        std::env::remove_var("LOONGCLAW_CONFIG_PATH");

        let config = LoongClawConfig::default();
        let config_path = std::env::temp_dir().join("loongclaw-runtime-env-config.toml");
        initialize_runtime_environment(&config, Some(&config_path));
        let exported = std::env::var("LOONGCLAW_CONFIG_PATH").expect("config path env");

        assert_eq!(exported, config_path.display().to_string());

        std::env::remove_var("LOONGCLAW_CONFIG_PATH");
    }
}
