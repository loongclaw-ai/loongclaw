use std::collections::BTreeSet;

use async_trait::async_trait;
use loongclaw_contracts::{Capability, MemoryCoreRequest};
use serde_json::{json, Value};

use crate::tools::{
    delegate_child_tool_view_for_config, delegate_child_tool_view_for_config_with_delegate,
    runtime_tool_view, runtime_tool_view_for_config, ToolView,
};
use crate::CliResult;
use crate::KernelContext;

#[cfg(feature = "memory-sqlite")]
use super::super::memory;
use super::super::{config::LoongClawConfig, provider};
use super::turn_engine::ProviderTurn;
#[cfg(feature = "memory-sqlite")]
use crate::memory::runtime_config::MemoryRuntimeConfig;
#[cfg(feature = "memory-sqlite")]
use crate::session::repository::{SessionKind, SessionRepository};

pub struct DefaultConversationRuntime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionContext {
    pub session_id: String,
    pub parent_session_id: Option<String>,
    pub tool_view: ToolView,
}

impl SessionContext {
    pub fn root(session_id: impl Into<String>) -> Self {
        Self::root_with_tool_view(session_id, runtime_tool_view())
    }

    pub fn root_with_tool_view(session_id: impl Into<String>, tool_view: ToolView) -> Self {
        Self {
            session_id: normalize_session_id(session_id.into()),
            parent_session_id: None,
            tool_view,
        }
    }

    pub fn child(
        session_id: impl Into<String>,
        parent_session_id: impl Into<String>,
        tool_view: ToolView,
    ) -> Self {
        Self {
            session_id: normalize_session_id(session_id.into()),
            parent_session_id: Some(normalize_session_id(parent_session_id.into())),
            tool_view,
        }
    }
}

fn normalize_session_id(session_id: String) -> String {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        "default".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[async_trait]
pub trait ConversationRuntime: Send + Sync {
    fn session_context(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        kernel_ctx: Option<&KernelContext>,
    ) -> CliResult<SessionContext> {
        Ok(SessionContext::root_with_tool_view(
            session_id,
            self.tool_view(config, session_id, kernel_ctx)?,
        ))
    }

    fn tool_view(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        kernel_ctx: Option<&KernelContext>,
    ) -> CliResult<ToolView>;

    fn build_messages(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        include_system_prompt: bool,
        tool_view: &ToolView,
        kernel_ctx: Option<&KernelContext>,
    ) -> CliResult<Vec<Value>>;

    async fn request_completion(
        &self,
        config: &LoongClawConfig,
        messages: &[Value],
    ) -> CliResult<String>;

    async fn request_turn(
        &self,
        config: &LoongClawConfig,
        messages: &[Value],
        tool_view: &ToolView,
    ) -> CliResult<ProviderTurn>;

    async fn persist_turn(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        kernel_ctx: Option<&KernelContext>,
    ) -> CliResult<()>;
}

#[async_trait]
impl ConversationRuntime for DefaultConversationRuntime {
    fn tool_view(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        _kernel_ctx: Option<&KernelContext>,
    ) -> CliResult<ToolView> {
        #[cfg(feature = "memory-sqlite")]
        {
            let memory_config = MemoryRuntimeConfig {
                sqlite_path: Some(config.memory.resolved_sqlite_path()),
            };
            if let Ok(repo) = SessionRepository::new(&memory_config) {
                if let Some(session) = repo
                    .load_session(session_id)
                    .map_err(|error| format!("load session tool-view context failed: {error}"))?
                {
                    if session.parent_session_id.is_some() {
                        let depth = repo.session_lineage_depth(session_id).map_err(|error| {
                            format!("compute session lineage depth for tool view failed: {error}")
                        })?;
                        let allow_nested_delegate = depth < config.tools.delegate.max_depth;
                        return Ok(delegate_child_tool_view_for_config_with_delegate(
                            &config.tools,
                            allow_nested_delegate,
                        ));
                    }
                } else if repo
                    .load_session_summary_with_legacy_fallback(session_id)
                    .map_err(|error| {
                        format!("load legacy session tool-view context failed: {error}")
                    })?
                    .is_some_and(|session| session.kind == SessionKind::DelegateChild)
                {
                    return Ok(delegate_child_tool_view_for_config(&config.tools));
                }
            }
        }
        Ok(runtime_tool_view_for_config(&config.tools))
    }

    // TODO(task-11): Route memory window loading through kernel when kernel_ctx is Some.
    // Currently `build_messages_for_session` couples system-prompt construction with
    // memory window loading in a single function. Routing the memory portion through
    // kernel requires splitting that function into (a) system prompt building and
    // (b) memory window loading. Deferred to avoid invasive refactoring of the
    // provider module.
    fn build_messages(
        &self,
        config: &LoongClawConfig,
        session_id: &str,
        include_system_prompt: bool,
        tool_view: &ToolView,
        _kernel_ctx: Option<&KernelContext>,
    ) -> CliResult<Vec<Value>> {
        provider::build_messages_for_session(config, session_id, include_system_prompt, tool_view)
    }

    async fn request_completion(
        &self,
        config: &LoongClawConfig,
        messages: &[Value],
    ) -> CliResult<String> {
        provider::request_completion(config, messages).await
    }

    async fn request_turn(
        &self,
        config: &LoongClawConfig,
        messages: &[Value],
        tool_view: &ToolView,
    ) -> CliResult<ProviderTurn> {
        provider::request_turn(config, messages, tool_view).await
    }

    async fn persist_turn(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        kernel_ctx: Option<&KernelContext>,
    ) -> CliResult<()> {
        if let Some(ctx) = kernel_ctx {
            let request = MemoryCoreRequest {
                operation: "append_turn".to_owned(),
                payload: json!({
                    "session_id": session_id,
                    "role": role,
                    "content": content,
                }),
            };
            let caps = BTreeSet::from([Capability::MemoryWrite]);
            ctx.kernel
                .execute_memory_core(ctx.pack_id(), &ctx.token, &caps, None, request)
                .await
                .map_err(|error| format!("persist {role} turn via kernel failed: {error}"))?;
            return Ok(());
        }

        #[cfg(feature = "memory-sqlite")]
        {
            memory::append_turn_direct(
                session_id,
                role,
                content,
                memory::runtime_config::get_memory_runtime_config(),
            )
            .map_err(|error| format!("persist {role} turn failed: {error}"))?;
        }

        #[cfg(not(feature = "memory-sqlite"))]
        {
            let _ = (session_id, role, content);
        }

        Ok(())
    }
}
