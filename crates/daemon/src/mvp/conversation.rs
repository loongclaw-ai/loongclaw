use async_trait::async_trait;
use serde_json::{json, Value};

use crate::CliResult;

#[cfg(feature = "memory-sqlite")]
use super::memory;
use super::{config::LoongClawConfig, provider};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderErrorMode {
    #[cfg_attr(
        not(any(feature = "channel-telegram", feature = "channel-feishu")),
        allow(dead_code)
    )]
    Propagate,
    InlineMessage,
}

pub struct DefaultConversationRuntime;

#[async_trait]
pub trait ConversationRuntime: Send + Sync {
    fn build_messages(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        include_system_prompt: bool,
    ) -> CliResult<Vec<Value>>;

    async fn request_completion(
        &self,
        config: &LoongClawConfig,
        messages: &[Value],
    ) -> CliResult<String>;

    fn persist_turn(&self, session_id: &str, role: &str, content: &str) -> CliResult<()>;
}

#[async_trait]
impl ConversationRuntime for DefaultConversationRuntime {
    fn build_messages(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        include_system_prompt: bool,
    ) -> CliResult<Vec<Value>> {
        provider::build_messages_for_session(config, session_id, include_system_prompt)
    }

    async fn request_completion(
        &self,
        config: &LoongClawConfig,
        messages: &[Value],
    ) -> CliResult<String> {
        provider::request_completion(config, messages).await
    }

    fn persist_turn(&self, session_id: &str, role: &str, content: &str) -> CliResult<()> {
        #[cfg(feature = "memory-sqlite")]
        {
            memory::append_turn_direct(session_id, role, content)
                .map_err(|error| format!("persist {role} turn failed: {error}"))?;
        }

        #[cfg(not(feature = "memory-sqlite"))]
        {
            let _ = (session_id, role, content);
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct ConversationOrchestrator;

impl ConversationOrchestrator {
    pub fn new() -> Self {
        Self
    }

    pub async fn handle_turn(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        user_input: &str,
        error_mode: ProviderErrorMode,
    ) -> CliResult<String> {
        let runtime = DefaultConversationRuntime;
        self.handle_turn_with_runtime(config, session_id, user_input, error_mode, &runtime)
            .await
    }

    pub async fn handle_turn_with_runtime<R: ConversationRuntime + ?Sized>(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        user_input: &str,
        error_mode: ProviderErrorMode,
        runtime: &R,
    ) -> CliResult<String> {
        let mut messages = runtime.build_messages(config, session_id, true)?;
        messages.push(json!({
            "role": "user",
            "content": user_input,
        }));

        let provider_result = runtime.request_completion(config, &messages).await;
        match provider_result {
            Ok(reply) => {
                persist_success_turns(runtime, session_id, user_input, &reply)?;
                Ok(reply)
            }
            Err(error) => match error_mode {
                ProviderErrorMode::Propagate => Err(error),
                ProviderErrorMode::InlineMessage => {
                    let synthetic = format_provider_error_reply(&error);
                    persist_error_turns(runtime, session_id, user_input, &synthetic)?;
                    Ok(synthetic)
                }
            },
        }
    }
}

fn format_provider_error_reply(error: &str) -> String {
    format!("[provider_error] {error}")
}

fn persist_success_turns<R: ConversationRuntime + ?Sized>(
    runtime: &R,
    session_id: &str,
    user_input: &str,
    assistant_reply: &str,
) -> CliResult<()> {
    runtime.persist_turn(session_id, "user", user_input)?;
    runtime.persist_turn(session_id, "assistant", assistant_reply)?;
    Ok(())
}

fn persist_error_turns<R: ConversationRuntime + ?Sized>(
    runtime: &R,
    session_id: &str,
    user_input: &str,
    synthetic_reply: &str,
) -> CliResult<()> {
    runtime.persist_turn(session_id, "user", user_input)?;
    runtime.persist_turn(session_id, "assistant", synthetic_reply)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::mvp::config::{
        CliChannelConfig, FeishuChannelConfig, MemoryConfig, ProviderConfig, TelegramChannelConfig,
        ToolConfig,
    };

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
}
