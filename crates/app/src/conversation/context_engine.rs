use async_trait::async_trait;
#[cfg(feature = "memory-sqlite")]
use loongclaw_contracts::Capability;
use serde_json::Value;

use crate::config::LoongClawConfig;
use crate::{CliResult, KernelContext};

#[cfg(feature = "memory-sqlite")]
use crate::memory;
use std::collections::BTreeSet;

#[cfg(feature = "memory-sqlite")]
use super::compaction::{CompactPolicy, compact_window};
use super::runtime_binding::ConversationRuntimeBinding;

pub const CONTEXT_ENGINE_API_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContextEngineCapability {
    KernelMemoryWindowRead,
    LegacyMessageAssembly,
    SessionBootstrap,
    MessageIngestion,
    ContextCompaction,
    SystemPromptAddition,
    SubagentLifecycle,
}

impl ContextEngineCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            ContextEngineCapability::KernelMemoryWindowRead => "kernel_memory_window_read",
            ContextEngineCapability::LegacyMessageAssembly => "legacy_message_assembly",
            ContextEngineCapability::SessionBootstrap => "session_bootstrap",
            ContextEngineCapability::MessageIngestion => "message_ingestion",
            ContextEngineCapability::ContextCompaction => "context_compaction",
            ContextEngineCapability::SystemPromptAddition => "system_prompt_addition",
            ContextEngineCapability::SubagentLifecycle => "subagent_lifecycle",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEngineMetadata {
    pub id: &'static str,
    pub api_version: u16,
    pub capabilities: BTreeSet<ContextEngineCapability>,
}

impl ContextEngineMetadata {
    pub fn new(
        id: &'static str,
        capabilities: impl IntoIterator<Item = ContextEngineCapability>,
    ) -> Self {
        Self {
            id,
            api_version: CONTEXT_ENGINE_API_VERSION,
            capabilities: capabilities.into_iter().collect(),
        }
    }

    pub fn capability_names(&self) -> Vec<&'static str> {
        self.capabilities
            .iter()
            .copied()
            .map(ContextEngineCapability::as_str)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssembledConversationContext {
    pub messages: Vec<Value>,
    pub estimated_tokens: Option<usize>,
    pub system_prompt_addition: Option<String>,
}

impl AssembledConversationContext {
    pub fn from_messages(messages: Vec<Value>) -> Self {
        Self {
            messages,
            estimated_tokens: None,
            system_prompt_addition: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContextEngineBootstrapResult {
    pub bootstrapped: bool,
    pub imported_messages: Option<usize>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContextEngineIngestResult {
    pub ingested: bool,
}

#[async_trait]
pub trait ConversationContextEngine: Send + Sync {
    fn id(&self) -> &'static str;

    fn metadata(&self) -> ContextEngineMetadata {
        ContextEngineMetadata::new(self.id(), [])
    }

    async fn bootstrap(
        &self,
        _config: &LoongClawConfig,
        _session_id: &str,
        _kernel_ctx: &KernelContext,
    ) -> CliResult<ContextEngineBootstrapResult> {
        Ok(ContextEngineBootstrapResult::default())
    }

    async fn ingest(
        &self,
        _session_id: &str,
        _message: &Value,
        _kernel_ctx: &KernelContext,
    ) -> CliResult<ContextEngineIngestResult> {
        Ok(ContextEngineIngestResult::default())
    }

    async fn after_turn(
        &self,
        _session_id: &str,
        _user_input: &str,
        _assistant_reply: &str,
        _messages: &[Value],
        _kernel_ctx: &KernelContext,
    ) -> CliResult<()> {
        Ok(())
    }

    async fn compact_context(
        &self,
        _config: &LoongClawConfig,
        _session_id: &str,
        _messages: &[Value],
        _kernel_ctx: &KernelContext,
    ) -> CliResult<()> {
        Ok(())
    }

    async fn prepare_subagent_spawn(
        &self,
        _parent_session_id: &str,
        _subagent_session_id: &str,
        _kernel_ctx: &KernelContext,
    ) -> CliResult<()> {
        Ok(())
    }

    async fn on_subagent_ended(
        &self,
        _parent_session_id: &str,
        _subagent_session_id: &str,
        _kernel_ctx: &KernelContext,
    ) -> CliResult<()> {
        Ok(())
    }

    async fn assemble_context(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        include_system_prompt: bool,
        binding: ConversationRuntimeBinding<'_>,
    ) -> CliResult<AssembledConversationContext> {
        self.assemble_messages(config, session_id, include_system_prompt, binding)
            .await
            .map(AssembledConversationContext::from_messages)
    }

    async fn assemble_messages(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        include_system_prompt: bool,
        binding: ConversationRuntimeBinding<'_>,
    ) -> CliResult<Vec<Value>>;
}

#[async_trait]
impl<T> ConversationContextEngine for Box<T>
where
    T: ConversationContextEngine + ?Sized,
{
    fn id(&self) -> &'static str {
        self.as_ref().id()
    }

    fn metadata(&self) -> ContextEngineMetadata {
        self.as_ref().metadata()
    }

    async fn bootstrap(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        kernel_ctx: &KernelContext,
    ) -> CliResult<ContextEngineBootstrapResult> {
        self.as_ref()
            .bootstrap(config, session_id, kernel_ctx)
            .await
    }

    async fn ingest(
        &self,
        session_id: &str,
        message: &Value,
        kernel_ctx: &KernelContext,
    ) -> CliResult<ContextEngineIngestResult> {
        self.as_ref().ingest(session_id, message, kernel_ctx).await
    }

    async fn after_turn(
        &self,
        session_id: &str,
        user_input: &str,
        assistant_reply: &str,
        messages: &[Value],
        kernel_ctx: &KernelContext,
    ) -> CliResult<()> {
        self.as_ref()
            .after_turn(
                session_id,
                user_input,
                assistant_reply,
                messages,
                kernel_ctx,
            )
            .await
    }

    async fn compact_context(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        messages: &[Value],
        kernel_ctx: &KernelContext,
    ) -> CliResult<()> {
        self.as_ref()
            .compact_context(config, session_id, messages, kernel_ctx)
            .await
    }

    async fn prepare_subagent_spawn(
        &self,
        parent_session_id: &str,
        subagent_session_id: &str,
        kernel_ctx: &KernelContext,
    ) -> CliResult<()> {
        self.as_ref()
            .prepare_subagent_spawn(parent_session_id, subagent_session_id, kernel_ctx)
            .await
    }

    async fn on_subagent_ended(
        &self,
        parent_session_id: &str,
        subagent_session_id: &str,
        kernel_ctx: &KernelContext,
    ) -> CliResult<()> {
        self.as_ref()
            .on_subagent_ended(parent_session_id, subagent_session_id, kernel_ctx)
            .await
    }

    async fn assemble_context(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        include_system_prompt: bool,
        binding: ConversationRuntimeBinding<'_>,
    ) -> CliResult<AssembledConversationContext> {
        self.as_ref()
            .assemble_context(config, session_id, include_system_prompt, binding)
            .await
    }

    async fn assemble_messages(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        include_system_prompt: bool,
        binding: ConversationRuntimeBinding<'_>,
    ) -> CliResult<Vec<Value>> {
        self.as_ref()
            .assemble_messages(config, session_id, include_system_prompt, binding)
            .await
    }
}

#[derive(Default)]
pub struct DefaultContextEngine;

#[derive(Default)]
pub struct LegacyContextEngine;

#[async_trait]
impl ConversationContextEngine for DefaultContextEngine {
    fn id(&self) -> &'static str {
        "default"
    }

    fn metadata(&self) -> ContextEngineMetadata {
        #[cfg(feature = "memory-sqlite")]
        let capabilities = [
            ContextEngineCapability::KernelMemoryWindowRead,
            ContextEngineCapability::ContextCompaction,
        ];
        #[cfg(not(feature = "memory-sqlite"))]
        let capabilities: [ContextEngineCapability; 0] = [];
        ContextEngineMetadata::new("default", capabilities)
    }

    async fn compact_context(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        _messages: &[Value],
        kernel_ctx: &KernelContext,
    ) -> CliResult<()> {
        #[cfg(feature = "memory-sqlite")]
        {
            let binding = ConversationRuntimeBinding::kernel(kernel_ctx);
            let has_summary_checkpoint = load_memory_context_entries(config, session_id, binding)
                .await?
                .into_iter()
                .any(|entry| entry.kind == memory::MemoryContextKind::Summary);
            if has_summary_checkpoint {
                return Ok(());
            }

            let turns = load_memory_window(config, session_id, binding).await?;
            let preserve_recent_turns = config
                .conversation
                .compact_preserve_recent_turns()
                .min(config.memory.sliding_window.saturating_sub(1));
            if preserve_recent_turns == 0 {
                return Ok(());
            }
            let Some(compacted) = compact_window(&turns, CompactPolicy::new(preserve_recent_turns))
            else {
                return Ok(());
            };

            persist_memory_window(session_id, &compacted, kernel_ctx).await?;
        }

        #[cfg(not(feature = "memory-sqlite"))]
        {
            let _ = (config, session_id, kernel_ctx);
        }

        Ok(())
    }

    async fn assemble_messages(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        include_system_prompt: bool,
        binding: ConversationRuntimeBinding<'_>,
    ) -> CliResult<Vec<Value>> {
        if !binding.is_kernel_bound() {
            return crate::provider::build_messages_for_session(
                config,
                session_id,
                include_system_prompt,
            );
        }

        #[cfg_attr(not(feature = "memory-sqlite"), allow(unused_mut))]
        let mut messages = crate::provider::build_base_messages(config, include_system_prompt);

        #[cfg(feature = "memory-sqlite")]
        {
            let turns = load_memory_window(config, session_id, binding).await?;
            for turn in turns {
                crate::provider::push_history_message(
                    &mut messages,
                    turn.role.as_str(),
                    turn.content.as_str(),
                );
            }
        }

        #[cfg(not(feature = "memory-sqlite"))]
        {
            let _ = (session_id, binding);
        }

        Ok(messages)
    }
}

#[async_trait]
impl ConversationContextEngine for LegacyContextEngine {
    fn id(&self) -> &'static str {
        "legacy"
    }

    fn metadata(&self) -> ContextEngineMetadata {
        ContextEngineMetadata::new("legacy", [ContextEngineCapability::LegacyMessageAssembly])
    }

    async fn assemble_messages(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        include_system_prompt: bool,
        _binding: ConversationRuntimeBinding<'_>,
    ) -> CliResult<Vec<Value>> {
        crate::provider::build_messages_for_session(config, session_id, include_system_prompt)
    }
}

#[cfg(feature = "memory-sqlite")]
async fn load_memory_window(
    config: &LoongClawConfig,
    session_id: &str,
    binding: ConversationRuntimeBinding<'_>,
) -> CliResult<Vec<memory::WindowTurn>> {
    use std::collections::BTreeSet;

    if let Some(ctx) = binding.kernel_context() {
        let request = memory::build_window_request(session_id, config.memory.sliding_window);
        let caps = BTreeSet::from([Capability::MemoryRead]);
        let outcome = ctx
            .kernel
            .execute_memory_core(ctx.pack_id(), &ctx.token, &caps, None, request)
            .await
            .map_err(|error| format!("load memory window via kernel failed: {error}"))?;

        if outcome.status != "ok" {
            return Err(format!(
                "load memory window via kernel returned non-ok status: {}",
                outcome.status
            ));
        }

        return Ok(memory::decode_window_turns(&outcome.payload));
    }

    let runtime_config =
        memory::runtime_config::MemoryRuntimeConfig::from_memory_config(&config.memory);
    let turns = memory::window_direct(session_id, config.memory.sliding_window, &runtime_config)
        .map_err(|error| format!("load memory window failed: {error}"))?;
    Ok(turns
        .into_iter()
        .map(|turn| memory::WindowTurn {
            role: turn.role,
            content: turn.content,
            ts: Some(turn.ts),
        })
        .collect())
}

#[cfg(feature = "memory-sqlite")]
async fn load_memory_context_entries(
    config: &LoongClawConfig,
    session_id: &str,
    binding: ConversationRuntimeBinding<'_>,
) -> CliResult<Vec<memory::MemoryContextEntry>> {
    if let Some(ctx) = binding.kernel_context() {
        let request = memory::build_read_context_request(session_id);
        let caps = BTreeSet::from([Capability::MemoryRead]);
        let outcome = ctx
            .kernel
            .execute_memory_core(ctx.pack_id(), &ctx.token, &caps, None, request)
            .await
            .map_err(|error| format!("load memory context via kernel failed: {error}"))?;

        if outcome.status != "ok" {
            return Err(format!(
                "load memory context via kernel returned non-ok status: {}",
                outcome.status
            ));
        }

        return Ok(memory::decode_memory_context_entries(&outcome.payload));
    }

    let runtime_config =
        memory::runtime_config::MemoryRuntimeConfig::from_memory_config(&config.memory);
    let hydrated = memory::hydrate_memory_context(session_id, &runtime_config)
        .map_err(|error| format!("load memory context failed: {error}"))?;
    Ok(hydrated.entries)
}

async fn persist_memory_window(
    session_id: &str,
    turns: &[memory::WindowTurn],
    kernel_ctx: &KernelContext,
) -> CliResult<()> {
    let request = memory::build_replace_turns_request(session_id, turns);
    let caps = BTreeSet::from([Capability::MemoryWrite]);
    let outcome = kernel_ctx
        .kernel
        .execute_memory_core(
            kernel_ctx.pack_id(),
            &kernel_ctx.token,
            &caps,
            None,
            request,
        )
        .await
        .map_err(|error| format!("persist compacted memory window via kernel failed: {error}"))?;

    if outcome.status != "ok" {
        return Err(format!(
            "persist compacted memory window via kernel returned non-ok status: {}",
            outcome.status
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_engine_metadata_has_stable_identity() {
        let metadata = DefaultContextEngine.metadata();
        assert_eq!(metadata.id, "default");
        assert_eq!(metadata.api_version, CONTEXT_ENGINE_API_VERSION);
    }

    #[test]
    fn legacy_engine_metadata_includes_legacy_capability() {
        let metadata = LegacyContextEngine.metadata();
        assert_eq!(metadata.id, "legacy");
        assert!(
            metadata
                .capabilities
                .contains(&ContextEngineCapability::LegacyMessageAssembly),
            "legacy engine should expose legacy assembly capability"
        );
        assert_eq!(metadata.capability_names(), vec!["legacy_message_assembly"]);
    }

    #[test]
    fn capability_names_for_future_hooks_are_stable() {
        assert_eq!(
            ContextEngineCapability::SessionBootstrap.as_str(),
            "session_bootstrap"
        );
        assert_eq!(
            ContextEngineCapability::MessageIngestion.as_str(),
            "message_ingestion"
        );
        assert_eq!(
            ContextEngineCapability::SystemPromptAddition.as_str(),
            "system_prompt_addition"
        );
        assert_eq!(
            ContextEngineCapability::SubagentLifecycle.as_str(),
            "subagent_lifecycle"
        );
    }
}
