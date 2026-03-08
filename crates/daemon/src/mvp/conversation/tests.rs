use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::super::config::{
    CliChannelConfig, FeishuChannelConfig, LoongClawConfig, MemoryConfig, ProviderConfig,
    TelegramChannelConfig, ToolConfig,
};
use super::persistence::format_provider_error_reply;
use super::*;
use crate::CliResult;

struct FakeRuntime {
    seed_messages: Vec<Value>,
    completion: Result<String, String>,
    persisted: Mutex<Vec<(String, String, String)>>,
    requested_messages: Mutex<Vec<Value>>,
}

impl FakeRuntime {
    fn new(seed_messages: Vec<Value>, completion: Result<String, String>) -> Self {
        Self {
            seed_messages,
            completion,
            persisted: Mutex::new(Vec::new()),
            requested_messages: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ConversationRuntime for FakeRuntime {
    fn build_messages(
        &self,
        _config: &LoongClawConfig,
        _session_id: &str,
        _include_system_prompt: bool,
    ) -> CliResult<Vec<Value>> {
        Ok(self.seed_messages.clone())
    }

    async fn request_completion(
        &self,
        _config: &LoongClawConfig,
        messages: &[Value],
    ) -> CliResult<String> {
        *self.requested_messages.lock().expect("request lock") = messages.to_vec();
        self.completion.clone().map_err(|error| error.to_owned())
    }

    fn persist_turn(&self, session_id: &str, role: &str, content: &str) -> CliResult<()> {
        self.persisted.lock().expect("persist lock").push((
            session_id.to_owned(),
            role.to_owned(),
            content.to_owned(),
        ));
        Ok(())
    }
}

fn test_config() -> LoongClawConfig {
    LoongClawConfig {
        provider: ProviderConfig::default(),
        cli: CliChannelConfig::default(),
        telegram: TelegramChannelConfig::default(),
        feishu: FeishuChannelConfig::default(),
        tools: ToolConfig::default(),
        memory: MemoryConfig::default(),
    }
}

#[tokio::test]
async fn handle_turn_with_runtime_success_persists_user_and_assistant_turns() {
    let runtime = FakeRuntime::new(
        vec![json!({"role": "system", "content": "sys"})],
        Ok("assistant-reply".to_owned()),
    );
    let orchestrator = ConversationOrchestrator::new();
    let reply = orchestrator
        .handle_turn_with_runtime(
            &test_config(),
            "session-1",
            "hello",
            ProviderErrorMode::Propagate,
            &runtime,
        )
        .await
        .expect("handle turn success");

    assert_eq!(reply, "assistant-reply");

    let requested = runtime.requested_messages.lock().expect("requested lock");
    assert_eq!(requested.len(), 2);
    assert_eq!(requested[1]["role"], "user");
    assert_eq!(requested[1]["content"], "hello");

    let persisted = runtime.persisted.lock().expect("persisted lock");
    assert_eq!(persisted.len(), 2);
    assert_eq!(
        persisted[0],
        (
            "session-1".to_owned(),
            "user".to_owned(),
            "hello".to_owned()
        )
    );
    assert_eq!(
        persisted[1],
        (
            "session-1".to_owned(),
            "assistant".to_owned(),
            "assistant-reply".to_owned(),
        )
    );
}

#[tokio::test]
async fn handle_turn_with_runtime_propagates_error_without_persisting() {
    let runtime = FakeRuntime::new(vec![], Err("timeout".to_owned()));
    let orchestrator = ConversationOrchestrator::new();
    let error = orchestrator
        .handle_turn_with_runtime(
            &test_config(),
            "session-2",
            "hello",
            ProviderErrorMode::Propagate,
            &runtime,
        )
        .await
        .expect_err("propagate mode should return error");

    assert!(error.contains("timeout"));
    assert!(runtime.persisted.lock().expect("persisted lock").is_empty());
}

#[tokio::test]
async fn handle_turn_with_runtime_inline_mode_returns_synthetic_reply_and_persists() {
    let runtime = FakeRuntime::new(vec![], Err("timeout".to_owned()));
    let orchestrator = ConversationOrchestrator::new();
    let output = orchestrator
        .handle_turn_with_runtime(
            &test_config(),
            "session-3",
            "hello",
            ProviderErrorMode::InlineMessage,
            &runtime,
        )
        .await
        .expect("inline mode should return synthetic reply");

    assert_eq!(output, "[provider_error] timeout");

    let persisted = runtime.persisted.lock().expect("persisted lock");
    assert_eq!(persisted.len(), 2);
    assert_eq!(
        persisted[0],
        (
            "session-3".to_owned(),
            "user".to_owned(),
            "hello".to_owned()
        )
    );
    assert_eq!(
        persisted[1],
        (
            "session-3".to_owned(),
            "assistant".to_owned(),
            "[provider_error] timeout".to_owned(),
        )
    );
}

#[test]
fn format_provider_error_reply_is_stable() {
    let output = format_provider_error_reply("timeout");
    assert_eq!(output, "[provider_error] timeout");
}
