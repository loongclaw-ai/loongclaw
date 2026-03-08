use crate::CliResult;

use super::runtime::ConversationRuntime;

pub(super) fn format_provider_error_reply(error: &str) -> String {
    format!("[provider_error] {error}")
}

pub(super) fn persist_success_turns<R: ConversationRuntime + ?Sized>(
    runtime: &R,
    session_id: &str,
    user_input: &str,
    assistant_reply: &str,
) -> CliResult<()> {
    runtime.persist_turn(session_id, "user", user_input)?;
    runtime.persist_turn(session_id, "assistant", assistant_reply)?;
    Ok(())
}

pub(super) fn persist_error_turns<R: ConversationRuntime + ?Sized>(
    runtime: &R,
    session_id: &str,
    user_input: &str,
    synthetic_reply: &str,
) -> CliResult<()> {
    runtime.persist_turn(session_id, "user", user_input)?;
    runtime.persist_turn(session_id, "assistant", synthetic_reply)?;
    Ok(())
}
