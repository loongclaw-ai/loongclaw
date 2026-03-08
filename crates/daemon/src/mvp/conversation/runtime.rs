use async_trait::async_trait;
use serde_json::Value;

use crate::CliResult;

#[cfg(feature = "memory-sqlite")]
use super::super::memory;
use super::super::{config::LoongClawConfig, provider};

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
