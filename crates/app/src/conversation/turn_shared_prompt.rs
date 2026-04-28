use serde_json::Value;

use super::tool_result::{
    followup_prompt_needs_truncation_hint, followup_prompt_uses_discovery_guidance,
    proactive_followup_continuation_context,
};

pub const TOOL_FOLLOWUP_PROMPT: &str = "Use the tool result above to continue satisfying the original user request. Prefer the next bounded tool call or completion step over narrating intermediate status. Only stop to answer in natural language when the request is actually complete, blocked on a real approval or input gate, or the available evidence is already sufficient. Do not include raw JSON, payload wrappers, or status markers unless the user explicitly asks for raw output.";
pub const DISCOVERY_RESULT_FOLLOWUP_PROMPT: &str = "The tool result above is a discovery result, not the final evidence. Choose the best matching discovered tool, reuse its lease when invoking it, continue with the next tool call needed to satisfy the original user request, and only answer directly if the discovery results already contain the final user-facing information.";
pub const TOOL_TRUNCATION_HINT_PROMPT: &str = "One or more tool results were truncated for context safety. If exact missing details are needed, explicitly state the truncation and request a narrower rerun.";
pub const EXTERNAL_SKILL_FOLLOWUP_PROMPT: &str = "An external skill has been loaded into runtime context. Follow its instructions while answering the original user request. Do not restate the skill verbatim unless the user explicitly asks for it.";
pub const DISCOVERY_RECOVERY_FOLLOWUP_PROMPT: &str = "The previous tool call could not be executed as requested. If you still need a hidden or discoverable capability, call tool.search with a short natural-language description of the missing capability. If tool.search returns a grouped hidden surface such as `skills`, `agent`, or `channel`, do not call that surface name directly; reuse its fresh lease through tool.invoke and place the requested operation inside payload.arguments. Otherwise, provide the best possible answer with the currently available evidence.";
pub const TOOL_LOOP_GUARD_PROMPT: &str = "Detected tool-loop behavior across rounds. Do not repeat identical or cyclical tool calls without new evidence. Adjust strategy (different tool, arguments, or decomposition) or provide the best possible final answer and clearly state remaining gaps.";

#[cfg(test)]
#[allow(dead_code)]
pub fn build_tool_followup_user_prompt(
    user_input: &str,
    loop_warning_reason: Option<&str>,
    tool_result_text: Option<&str>,
    rendered_tool_result_text: Option<&str>,
    _tool_request_summary: Option<&str>,
) -> String {
    build_tool_followup_user_prompt_with_context(
        user_input,
        loop_warning_reason,
        tool_result_text,
        rendered_tool_result_text,
        None,
    )
}

pub fn build_tool_followup_user_prompt_with_context(
    user_input: &str,
    loop_warning_reason: Option<&str>,
    tool_result_text: Option<&str>,
    rendered_tool_result_text: Option<&str>,
    extra_context: Option<&str>,
) -> String {
    let prompt =
        if followup_prompt_uses_discovery_guidance(tool_result_text, rendered_tool_result_text) {
            DISCOVERY_RESULT_FOLLOWUP_PROMPT
        } else {
            TOOL_FOLLOWUP_PROMPT
        };

    let mut sections = vec![prompt.to_owned()];
    if let Some(reason) = loop_warning_reason {
        sections.push(format!(
            "Loop warning:\n{reason}\nAvoid repeating the same tool call with unchanged results. Try a different tool, adjust arguments, or provide a best-effort final answer if evidence is sufficient."
        ));
    }
    if followup_prompt_needs_truncation_hint(tool_result_text, rendered_tool_result_text) {
        sections.push(TOOL_TRUNCATION_HINT_PROMPT.to_owned());
    }
    if let Some(continuation_guidance) =
        proactive_followup_continuation_context(tool_result_text, rendered_tool_result_text)
    {
        sections.push(continuation_guidance);
    }
    if let Some(extra_context) = extra_context {
        sections.push(extra_context.to_owned());
    }
    sections.push(format!("Original request:\n{user_input}"));
    sections.join("\n\n")
}

pub(crate) fn combine_followup_extra_context(parts: &[Option<&str>]) -> Option<String> {
    let joined = parts
        .iter()
        .flatten()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    (!joined.is_empty()).then(|| joined.join("\n\n"))
}

pub fn build_discovery_recovery_followup_user_prompt(
    user_input: &str,
    loop_warning_reason: Option<&str>,
    recovery_reason: &str,
) -> String {
    let mut sections = vec![DISCOVERY_RECOVERY_FOLLOWUP_PROMPT.to_owned()];
    sections.push(format!("Recovery reason:\n{recovery_reason}"));
    if let Some(reason) = loop_warning_reason {
        sections.push(format!(
            "Loop warning:\n{reason}\nAvoid repeating identical unavailable tool calls. Search for the missing capability or change strategy."
        ));
    }
    sections.push(format!("Original request:\n{user_input}"));
    sections.join("\n\n")
}

pub fn join_non_empty_lines(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn append_followup_preface(messages: &mut Vec<Value>, assistant_preface: &str) {
    let preface = assistant_preface.trim();
    if !preface.is_empty() {
        messages.push(serde_json::json!({
            "role": "assistant",
            "content": preface,
        }));
    }
}

pub(crate) fn append_followup_warning(
    messages: &mut Vec<Value>,
    loop_warning_reason: Option<&str>,
) {
    if let Some(reason) = loop_warning_reason {
        messages.push(serde_json::json!({
            "role": "assistant",
            "content": format!("[tool_loop_warning]\n{reason}"),
        }));
    }
}
