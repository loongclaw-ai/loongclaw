use serde_json::json;

use crate::CliResult;

use super::super::config::LoongClawConfig;
use super::persistence::{format_provider_error_reply, persist_error_turns, persist_success_turns};
use super::runtime::{ConversationRuntime, DefaultConversationRuntime};
use super::ProviderErrorMode;

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
