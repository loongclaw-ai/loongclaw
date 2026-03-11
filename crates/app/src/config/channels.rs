#[cfg(feature = "channel-telegram")]
use std::env;

use serde::{Deserialize, Serialize};

#[cfg(feature = "channel-feishu")]
use super::shared::read_secret_prefer_inline;
use super::shared::{validate_env_pointer_field, ConfigValidationIssue, EnvPointerValidationHint};

const TELEGRAM_BOT_TOKEN_ENV: &str = "TELEGRAM_BOT_TOKEN";
const FEISHU_APP_ID_ENV: &str = "FEISHU_APP_ID";
const FEISHU_APP_SECRET_ENV: &str = "FEISHU_APP_SECRET";
const FEISHU_VERIFICATION_TOKEN_ENV: &str = "FEISHU_VERIFICATION_TOKEN";
const FEISHU_ENCRYPT_KEY_ENV: &str = "FEISHU_ENCRYPT_KEY";

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
    pub encrypt_key: Option<String>,
    #[serde(default)]
    pub encrypt_key_env: Option<String>,
    #[serde(default)]
    pub allowed_chat_ids: Vec<String>,
    #[serde(default = "default_true")]
    pub ignore_bot_messages: bool,
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
            bot_token_env: Some(TELEGRAM_BOT_TOKEN_ENV.to_owned()),
            base_url: default_telegram_base_url(),
            polling_timeout_s: default_telegram_timeout_seconds(),
            allowed_chat_ids: Vec::new(),
        }
    }
}

impl TelegramChannelConfig {
    pub(super) fn validate(&self) -> Vec<ConfigValidationIssue> {
        let mut issues = Vec::new();
        if let Err(issue) = validate_env_pointer_field(
            "telegram.bot_token_env",
            self.bot_token_env.as_deref(),
            EnvPointerValidationHint {
                inline_field_path: "telegram.bot_token",
                example_env_name: TELEGRAM_BOT_TOKEN_ENV,
                detect_telegram_token_shape: true,
            },
        ) {
            issues.push(*issue);
        }
        issues
    }

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
            app_id_env: Some(FEISHU_APP_ID_ENV.to_owned()),
            app_secret_env: Some(FEISHU_APP_SECRET_ENV.to_owned()),
            base_url: default_feishu_base_url(),
            receive_id_type: default_feishu_receive_id_type(),
            webhook_bind: default_feishu_webhook_bind(),
            webhook_path: default_feishu_webhook_path(),
            verification_token: None,
            verification_token_env: Some(FEISHU_VERIFICATION_TOKEN_ENV.to_owned()),
            encrypt_key: None,
            encrypt_key_env: Some(FEISHU_ENCRYPT_KEY_ENV.to_owned()),
            allowed_chat_ids: Vec::new(),
            ignore_bot_messages: true,
        }
    }
}

impl FeishuChannelConfig {
    pub(super) fn validate(&self) -> Vec<ConfigValidationIssue> {
        let mut issues = Vec::new();
        if let Err(issue) = validate_env_pointer_field(
            "feishu.app_id_env",
            self.app_id_env.as_deref(),
            EnvPointerValidationHint {
                inline_field_path: "feishu.app_id",
                example_env_name: FEISHU_APP_ID_ENV,
                detect_telegram_token_shape: false,
            },
        ) {
            issues.push(*issue);
        }
        if let Err(issue) = validate_env_pointer_field(
            "feishu.app_secret_env",
            self.app_secret_env.as_deref(),
            EnvPointerValidationHint {
                inline_field_path: "feishu.app_secret",
                example_env_name: FEISHU_APP_SECRET_ENV,
                detect_telegram_token_shape: false,
            },
        ) {
            issues.push(*issue);
        }
        if let Err(issue) = validate_env_pointer_field(
            "feishu.verification_token_env",
            self.verification_token_env.as_deref(),
            EnvPointerValidationHint {
                inline_field_path: "feishu.verification_token",
                example_env_name: FEISHU_VERIFICATION_TOKEN_ENV,
                detect_telegram_token_shape: false,
            },
        ) {
            issues.push(*issue);
        }
        if let Err(issue) = validate_env_pointer_field(
            "feishu.encrypt_key_env",
            self.encrypt_key_env.as_deref(),
            EnvPointerValidationHint {
                inline_field_path: "feishu.encrypt_key",
                example_env_name: FEISHU_ENCRYPT_KEY_ENV,
                detect_telegram_token_shape: false,
            },
        ) {
            issues.push(*issue);
        }
        issues
    }

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

    #[cfg(feature = "channel-feishu")]
    pub fn encrypt_key(&self) -> Option<String> {
        read_secret_prefer_inline(self.encrypt_key.as_deref(), self.encrypt_key_env.as_deref())
    }
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

const fn default_true() -> bool {
    true
}
