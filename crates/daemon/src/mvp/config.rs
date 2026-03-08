use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::CliResult;

const DEFAULT_CONFIG_FILE: &str = "loongclaw.toml";
const DEFAULT_SQLITE_FILE: &str = "memory.sqlite3";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[default]
    OpenaiCompatible,
    VolcengineCustom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default)]
    pub kind: ProviderKind,
    pub model: String,
    #[serde(default = "default_provider_base_url")]
    pub base_url: String,
    #[serde(default = "default_openai_chat_path")]
    pub chat_completions_path: String,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default = "default_provider_timeout_ms")]
    pub request_timeout_ms: u64,
    #[serde(default = "default_provider_retry_max_attempts")]
    pub retry_max_attempts: usize,
    #[serde(default = "default_provider_retry_initial_backoff_ms")]
    pub retry_initial_backoff_ms: u64,
    #[serde(default = "default_provider_retry_max_backoff_ms")]
    pub retry_max_backoff_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliChannelConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    #[serde(default = "default_exit_commands")]
    pub exit_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramChannelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: Option<String>,
    #[serde(default)]
    pub bot_token_env: Option<String>,
    #[serde(default = "default_telegram_base_url")]
    pub base_url: String,
    #[serde(default = "default_telegram_timeout_seconds")]
    pub polling_timeout_s: u64,
    #[serde(default)]
    pub allowed_chat_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuChannelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub app_id: Option<String>,
    #[serde(default)]
    pub app_secret: Option<String>,
    #[serde(default)]
    pub app_id_env: Option<String>,
    #[serde(default)]
    pub app_secret_env: Option<String>,
    #[serde(default = "default_feishu_base_url")]
    pub base_url: String,
    #[serde(default = "default_feishu_receive_id_type")]
    pub receive_id_type: String,
    #[serde(default = "default_feishu_webhook_bind")]
    pub webhook_bind: String,
    #[serde(default = "default_feishu_webhook_path")]
    pub webhook_path: String,
    #[serde(default)]
    pub verification_token: Option<String>,
    #[serde(default)]
    pub verification_token_env: Option<String>,
    #[serde(default)]
    pub allowed_chat_ids: Vec<String>,
    #[serde(default = "default_true")]
    pub ignore_bot_messages: bool,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoongClawConfig {
    #[serde(default)]
    pub provider: ProviderConfig,
    #[serde(default)]
    pub cli: CliChannelConfig,
    #[serde(default)]
    pub telegram: TelegramChannelConfig,
    #[serde(default)]
    pub feishu: FeishuChannelConfig,
    #[serde(default)]
    pub tools: ToolConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            kind: ProviderKind::OpenaiCompatible,
            model: "gpt-4o-mini".to_owned(),
            base_url: default_provider_base_url(),
            chat_completions_path: default_openai_chat_path(),
            endpoint: None,
            api_key: None,
            api_key_env: Some("OPENAI_API_KEY".to_owned()),
            headers: BTreeMap::new(),
            temperature: default_temperature(),
            max_tokens: None,
            request_timeout_ms: default_provider_timeout_ms(),
            retry_max_attempts: default_provider_retry_max_attempts(),
            retry_initial_backoff_ms: default_provider_retry_initial_backoff_ms(),
            retry_max_backoff_ms: default_provider_retry_max_backoff_ms(),
        }
    }
}

impl ProviderConfig {
    pub fn endpoint(&self) -> String {
        match self.kind {
            ProviderKind::OpenaiCompatible => {
                let base = self.base_url.trim_end_matches('/');
                let path = self.chat_completions_path.trim();
                if path.is_empty() {
                    format!("{base}/v1/chat/completions")
                } else if path.starts_with('/') {
                    format!("{base}{path}")
                } else {
                    format!("{base}/{path}")
                }
            }
            ProviderKind::VolcengineCustom => self.endpoint.clone().unwrap_or_else(|| {
                "https://ark.cn-beijing.volces.com/api/v3/chat/completions".to_owned()
            }),
        }
    }

    pub fn api_key(&self) -> Option<String> {
        if let Some(raw) = self.api_key.as_deref() {
            let value = raw.trim();
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
        if let Some(env_key) = self.api_key_env.as_deref() {
            let value = env::var(env_key).ok()?;
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
        None
    }
}

impl Default for CliChannelConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            system_prompt: default_system_prompt(),
            exit_commands: default_exit_commands(),
        }
    }
}

impl Default for TelegramChannelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bot_token: None,
            bot_token_env: Some("TELEGRAM_BOT_TOKEN".to_owned()),
            base_url: default_telegram_base_url(),
            polling_timeout_s: default_telegram_timeout_seconds(),
            allowed_chat_ids: Vec::new(),
        }
    }
}

impl TelegramChannelConfig {
    #[cfg(feature = "channel-telegram")]
    pub fn bot_token(&self) -> Option<String> {
        if let Some(raw) = self.bot_token.as_deref() {
            let value = raw.trim();
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
        if let Some(env_key) = self.bot_token_env.as_deref() {
            let value = env::var(env_key).ok()?;
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
        None
    }
}

impl Default for FeishuChannelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            app_id: None,
            app_secret: None,
            app_id_env: Some("FEISHU_APP_ID".to_owned()),
            app_secret_env: Some("FEISHU_APP_SECRET".to_owned()),
            base_url: default_feishu_base_url(),
            receive_id_type: default_feishu_receive_id_type(),
            webhook_bind: default_feishu_webhook_bind(),
            webhook_path: default_feishu_webhook_path(),
            verification_token: None,
            verification_token_env: Some("FEISHU_VERIFICATION_TOKEN".to_owned()),
            allowed_chat_ids: Vec::new(),
            ignore_bot_messages: true,
        }
    }
}

impl FeishuChannelConfig {
    #[cfg(feature = "channel-feishu")]
    pub fn app_id(&self) -> Option<String> {
        read_secret_prefer_inline(self.app_id.as_deref(), self.app_id_env.as_deref())
    }

    #[cfg(feature = "channel-feishu")]
    pub fn app_secret(&self) -> Option<String> {
        read_secret_prefer_inline(self.app_secret.as_deref(), self.app_secret_env.as_deref())
    }

    #[cfg(feature = "channel-feishu")]
    pub fn verification_token(&self) -> Option<String> {
        read_secret_prefer_inline(
            self.verification_token.as_deref(),
            self.verification_token_env.as_deref(),
        )
    }
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
}

impl Default for LoongClawConfig {
    fn default() -> Self {
        Self {
            provider: ProviderConfig::default(),
            cli: CliChannelConfig::default(),
            telegram: TelegramChannelConfig::default(),
            feishu: FeishuChannelConfig::default(),
            tools: ToolConfig::default(),
            memory: MemoryConfig::default(),
        }
    }
}

pub fn load(path: Option<&str>) -> CliResult<(PathBuf, LoongClawConfig)> {
    let config_path = path.map(expand_path).unwrap_or_else(default_config_path);
    let raw = fs::read_to_string(&config_path).map_err(|error| {
        format!(
            "failed to read config {}: {error}. run `loongclawd setup` first",
            config_path.display()
        )
    })?;
    parse_toml_config(&raw).map(|config| (config_path, config))
}

pub fn write_template(path: Option<&str>, force: bool) -> CliResult<PathBuf> {
    let output_path = path.map(expand_path).unwrap_or_else(default_config_path);
    if output_path.exists() && !force {
        return Err(format!(
            "config {} already exists (use --force to overwrite)",
            output_path.display()
        ));
    }

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create config directory: {error}"))?;
        }
    }

    let encoded = encode_toml_config(&LoongClawConfig::default())?;
    fs::write(&output_path, encoded).map_err(|error| {
        format!(
            "failed to write config file {}: {error}",
            output_path.display()
        )
    })?;
    Ok(output_path)
}

pub fn default_config_path() -> PathBuf {
    default_loongclaw_home().join(DEFAULT_CONFIG_FILE)
}

pub fn default_loongclaw_home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".loongclaw")
}

fn expand_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed == "~" {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
    }
    if let Some(stripped) = trimmed.strip_prefix("~/") {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(stripped);
    }
    Path::new(trimmed).to_path_buf()
}

fn default_provider_base_url() -> String {
    "https://api.openai.com".to_owned()
}

fn default_openai_chat_path() -> String {
    "/v1/chat/completions".to_owned()
}

fn default_telegram_base_url() -> String {
    "https://api.telegram.org".to_owned()
}

const fn default_telegram_timeout_seconds() -> u64 {
    15
}

fn default_feishu_base_url() -> String {
    "https://open.feishu.cn".to_owned()
}

fn default_feishu_receive_id_type() -> String {
    "chat_id".to_owned()
}

fn default_feishu_webhook_bind() -> String {
    "127.0.0.1:8080".to_owned()
}

fn default_feishu_webhook_path() -> String {
    "/feishu/events".to_owned()
}

fn default_system_prompt() -> String {
    "You are LoongClaw, a practical assistant.".to_owned()
}

fn default_exit_commands() -> Vec<String> {
    vec!["/exit".to_owned(), "/quit".to_owned()]
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

const fn default_temperature() -> f64 {
    0.2
}

const fn default_provider_timeout_ms() -> u64 {
    30_000
}

const fn default_provider_retry_max_attempts() -> usize {
    3
}

const fn default_provider_retry_initial_backoff_ms() -> u64 {
    300
}

const fn default_provider_retry_max_backoff_ms() -> u64 {
    3_000
}

const fn default_true() -> bool {
    true
}

#[cfg(feature = "channel-feishu")]
fn read_secret_prefer_inline(inline: Option<&str>, env_key: Option<&str>) -> Option<String> {
    if let Some(raw) = inline {
        let value = raw.trim();
        if !value.is_empty() {
            return Some(value.to_owned());
        }
    }
    if let Some(key) = env_key {
        let value = env::var(key).ok()?;
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }
    None
}

#[cfg(feature = "config-toml")]
fn parse_toml_config(raw: &str) -> CliResult<LoongClawConfig> {
    toml::from_str(raw).map_err(|error| format!("failed to parse TOML config: {error}"))
}

#[cfg(not(feature = "config-toml"))]
fn parse_toml_config(_raw: &str) -> CliResult<LoongClawConfig> {
    Err("config-toml feature is disabled for this build".to_owned())
}

#[cfg(feature = "config-toml")]
fn encode_toml_config(config: &LoongClawConfig) -> CliResult<String> {
    toml::to_string_pretty(config).map_err(|error| format!("failed to encode TOML config: {error}"))
}

#[cfg(not(feature = "config-toml"))]
fn encode_toml_config(_config: &LoongClawConfig) -> CliResult<String> {
    Err("config-toml feature is disabled for this build".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_resolution_for_openai_compatible_is_stable() {
        let config = ProviderConfig {
            base_url: "https://api.openai.com/".to_owned(),
            chat_completions_path: "/v1/chat/completions".to_owned(),
            ..ProviderConfig::default()
        };
        assert_eq!(
            config.endpoint(),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn endpoint_resolution_for_volcengine_prefers_explicit_endpoint() {
        let config = ProviderConfig {
            kind: ProviderKind::VolcengineCustom,
            endpoint: Some("https://example.volcengine.com/chat/completions".to_owned()),
            ..ProviderConfig::default()
        };
        assert_eq!(
            config.endpoint(),
            "https://example.volcengine.com/chat/completions"
        );
    }

    #[test]
    #[cfg(feature = "channel-telegram")]
    fn telegram_token_prefers_inline_secret() {
        let config = TelegramChannelConfig {
            bot_token: Some("inline-token".to_owned()),
            bot_token_env: Some("SHOULD_NOT_BE_READ".to_owned()),
            ..TelegramChannelConfig::default()
        };
        assert_eq!(config.bot_token().as_deref(), Some("inline-token"));
    }

    #[test]
    fn feishu_defaults_are_stable() {
        let config = FeishuChannelConfig::default();
        assert_eq!(config.base_url, "https://open.feishu.cn");
        assert_eq!(config.receive_id_type, "chat_id");
        assert_eq!(config.webhook_bind, "127.0.0.1:8080");
        assert_eq!(config.webhook_path, "/feishu/events");
        assert!(config.ignore_bot_messages);
    }

    #[test]
    fn provider_retry_defaults_are_stable() {
        let config = ProviderConfig::default();
        assert_eq!(config.request_timeout_ms, 30_000);
        assert_eq!(config.retry_max_attempts, 3);
        assert_eq!(config.retry_initial_backoff_ms, 300);
        assert_eq!(config.retry_max_backoff_ms, 3_000);
    }
}
