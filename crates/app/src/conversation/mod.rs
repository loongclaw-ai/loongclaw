mod persistence;
mod runtime;
pub mod turn_engine;
mod turn_loop;

pub use turn_loop::ConversationTurnLoop;
#[cfg(feature = "memory-sqlite")]
pub use turn_loop::{run_delegate_child_turn, run_delegate_child_turn_with_runtime};
pub type ConversationOrchestrator = ConversationTurnLoop;
#[allow(unused_imports)]
pub use runtime::{ConversationRuntime, DefaultConversationRuntime, SessionContext};
pub use turn_engine::{
    AppToolDispatcher, DefaultAppToolDispatcher, NoopAppToolDispatcher, ProviderTurn, ToolDecision,
    ToolIntent, ToolOutcome, TurnEngine, TurnResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderErrorMode {
    #[cfg_attr(
        not(any(feature = "channel-telegram", feature = "channel-feishu")),
        allow(dead_code)
    )]
    Propagate,
    InlineMessage,
}

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod tests;
