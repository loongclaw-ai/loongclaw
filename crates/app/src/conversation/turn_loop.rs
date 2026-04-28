use crate::CliResult;
use crate::session::store;

use super::super::config::LoongConfig;
use super::ProviderErrorMode;
use super::runtime::{ConversationRuntime, load_default_conversation_runtime};
use super::runtime_binding::ConversationRuntimeBinding;
use super::turn_budget::TurnRoundBudget;
use super::turn_engine::DefaultAppToolDispatcher;
use super::turn_loop_request::{RequestedTurn, request_round_turn};
use super::turn_loop_round::{
    apply_turn_loop_terminal_action, evaluate_round_kernel, initialize_turn_loop_session,
    resolve_round_kernel_terminal_action,
};
use super::turn_loop_state::{
    TurnLoopPolicy, TurnLoopTerminalAction, build_round_limit_terminal_action,
    decide_round_kernel_action,
};
use super::turn_shared::{ReplyPersistenceMode, tool_loop_circuit_breaker_reply};

#[derive(Default)]
pub struct ConversationTurnLoop;

impl ConversationTurnLoop {
    pub fn new() -> Self {
        Self
    }

    pub async fn handle_turn(
        &self,
        config: &LoongConfig,
        session_id: &str,
        user_input: &str,
        error_mode: ProviderErrorMode,
        binding: ConversationRuntimeBinding<'_>,
    ) -> CliResult<String> {
        let runtime = load_default_conversation_runtime(config)?;
        self.handle_turn_with_runtime(
            config, session_id, user_input, error_mode, &runtime, binding,
        )
        .await
    }

    pub async fn handle_turn_with_runtime<R: ConversationRuntime + ?Sized>(
        &self,
        config: &LoongConfig,
        session_id: &str,
        user_input: &str,
        error_mode: ProviderErrorMode,
        runtime: &R,
        binding: ConversationRuntimeBinding<'_>,
    ) -> CliResult<String> {
        let policy = TurnLoopPolicy::from_config(config);
        let session_context = runtime.session_context(config, session_id, binding)?;
        let tool_view = session_context.tool_view.clone();
        let app_dispatcher = DefaultAppToolDispatcher::with_config(
            store::session_store_config_from_memory_config(&config.memory),
            config.clone(),
        );
        let turn_id = super::turn_shared::next_conversation_turn_id();
        let mut session = initialize_turn_loop_session(
            runtime
                .build_messages(config, session_id, true, &tool_view, binding)
                .await?,
            user_input,
            &policy,
        );

        for round_index in 0..policy.max_rounds {
            let turn = match request_round_turn(
                runtime,
                config,
                session_id,
                turn_id.as_str(),
                &session.messages,
                &tool_view,
                error_mode,
                binding,
            )
            .await?
            {
                RequestedTurn::Continue(turn) => turn,
                RequestedTurn::Terminal(action) => {
                    return apply_turn_loop_terminal_action(
                        runtime, session_id, user_input, action, binding,
                    )
                    .await;
                }
            };

            // Global circuit breaker: prospective check before dispatching tools.
            // Trips if adding this round's intents would exceed the per-turn limit,
            // ensuring the configured max remains inclusive for executed tool calls.
            let prospective_total = session
                .total_tool_calls
                .saturating_add(turn.tool_intents.len());
            if let Some(reply) =
                tool_loop_circuit_breaker_reply(prospective_total, policy.max_total_tool_calls)
            {
                return apply_turn_loop_terminal_action(
                    runtime,
                    session_id,
                    user_input,
                    TurnLoopTerminalAction::PersistReply {
                        reply,
                        persistence_mode: ReplyPersistenceMode::Success,
                    },
                    binding,
                )
                .await;
            }

            let evaluation = evaluate_round_kernel(
                config,
                &policy,
                &turn,
                &session_context,
                &app_dispatcher,
                binding,
                &mut session.loop_supervisor,
            )
            .await;

            session.total_tool_calls = prospective_total;

            let reply_phase = evaluation.reply_phase(session.raw_tool_output_requested);
            if let Some(raw_reply) = reply_phase.raw_reply() {
                session.last_raw_reply = raw_reply.to_owned();
            }
            let decision = decide_round_kernel_action(
                TurnRoundBudget::for_round_index(round_index, policy.max_rounds),
                evaluation,
                reply_phase,
            );

            if let Some(action) = resolve_round_kernel_terminal_action(
                runtime,
                config,
                &mut session,
                user_input,
                decision,
                binding,
            )
            .await?
            {
                return apply_turn_loop_terminal_action(
                    runtime, session_id, user_input, action, binding,
                )
                .await;
            }
        }

        apply_turn_loop_terminal_action(
            runtime,
            session_id,
            user_input,
            build_round_limit_terminal_action(session.last_raw_reply.as_str()),
            binding,
        )
        .await
    }
}

impl TurnLoopPolicy {
    fn from_config(config: &LoongConfig) -> Self {
        let turn_loop = &config.conversation.turn_loop;
        Self {
            max_rounds: super::TURN_LOOP_MAX_ROUNDS,
            max_tool_steps_per_round: super::FAST_LANE_MAX_TOOL_STEPS_PER_TURN,
            max_followup_tool_payload_chars: turn_loop.max_followup_tool_payload_chars.max(256),
            max_followup_tool_payload_chars_total: turn_loop
                .max_followup_tool_payload_chars_total
                .max(1),
            max_total_tool_calls: super::TURN_LOOP_MAX_TOTAL_TOOL_CALLS,
            max_consecutive_same_tool: super::TURN_LOOP_MAX_CONSECUTIVE_SAME_TOOL,
        }
    }
}

#[cfg(test)]
#[path = "turn_loop_tests.rs"]
mod tests;
