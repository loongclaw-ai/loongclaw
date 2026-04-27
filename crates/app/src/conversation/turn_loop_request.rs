use std::sync::Arc;

use serde_json::Value;

use crate::CliResult;
use crate::acp::{AcpTurnEventSink, JsonlAcpTurnEventSink};

use super::super::config::LoongConfig;
use super::ProviderErrorMode;
use super::runtime::ConversationRuntime;
use super::runtime_binding::ConversationRuntimeBinding;
use super::turn_engine::ProviderTurn;
use super::turn_loop_state::TurnLoopTerminalAction;
use super::turn_observer::map_streaming_callback_data_to_token_event;
use super::turn_shared::{
    ProviderTurnRequestAction, ReplyPersistenceMode, decide_provider_turn_request_action,
};
use crate::tools::ToolView;

pub(super) enum RequestedTurn {
    Continue(ProviderTurn),
    Terminal(TurnLoopTerminalAction),
}

pub(super) async fn request_round_turn<R: ConversationRuntime + ?Sized>(
    runtime: &R,
    config: &LoongConfig,
    session_id: &str,
    turn_id: &str,
    messages: &[Value],
    tool_view: &ToolView,
    error_mode: ProviderErrorMode,
    binding: ConversationRuntimeBinding<'_>,
) -> CliResult<RequestedTurn> {
    let use_streaming = crate::provider::supports_turn_streaming_events(config);
    let on_token = if use_streaming {
        streaming_token_callback()
    } else {
        None
    };

    let request_result = if use_streaming {
        runtime
            .request_turn_streaming(
                config, session_id, turn_id, messages, tool_view, binding, on_token,
            )
            .await
    } else {
        runtime
            .request_turn(config, session_id, turn_id, messages, tool_view, binding)
            .await
    };

    Ok(
        match decide_provider_turn_request_action(request_result, error_mode) {
            ProviderTurnRequestAction::Continue { turn } => RequestedTurn::Continue(turn),
            ProviderTurnRequestAction::FinalizeInlineProviderError { reply } => {
                RequestedTurn::Terminal(TurnLoopTerminalAction::PersistReply {
                    reply,
                    persistence_mode: ReplyPersistenceMode::InlineProviderError,
                })
            }
            ProviderTurnRequestAction::ReturnError { error } => {
                RequestedTurn::Terminal(TurnLoopTerminalAction::ReturnError { error })
            }
        },
    )
}

fn streaming_token_callback() -> crate::provider::StreamingTokenCallback {
    let sink = JsonlAcpTurnEventSink::stderr_with_prefix("");
    Some(Arc::new(
        move |data: crate::provider::StreamingCallbackData| {
            let event = map_streaming_callback_data_to_token_event(data);
            let _ = sink.on_event(&serde_json::to_value(&event).unwrap_or_default());
        },
    ))
}
