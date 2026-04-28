use serde_json::Value;

use super::super::turn_engine::{
    ProviderTurn, ToolBatchExecutionIntentStatus, ToolBatchExecutionTrace, ToolIntent, TurnResult,
};

pub(crate) fn summarize_tool_followup_request(intents: &[ToolIntent]) -> Option<String> {
    match intents {
        [] => None,
        [intent] => summarize_single_tool_followup_request(intent),
        intents => serde_json::to_string(
            &intents
                .iter()
                .map(tool_followup_request_entry)
                .collect::<Vec<_>>(),
        )
        .ok(),
    }
}

pub(crate) fn summarize_single_tool_followup_request(intent: &ToolIntent) -> Option<String> {
    let entry = tool_followup_request_entry(intent);
    serde_json::to_string(&entry).ok()
}

pub(crate) fn summarize_provider_lane_tool_request(
    turn: &ProviderTurn,
    turn_result: &TurnResult,
    trace: Option<&ToolBatchExecutionTrace>,
) -> Option<String> {
    match turn_result {
        TurnResult::FinalText(_) | TurnResult::StreamingText(_) | TurnResult::StreamingDone(_) => {
            summarize_tool_followup_request(&turn.tool_intents)
        }
        TurnResult::NeedsApproval(_)
        | TurnResult::ToolDenied(_)
        | TurnResult::ToolError(_)
        | TurnResult::ProviderError(_) => summarize_failed_provider_lane_tool_request(turn, trace),
    }
}

pub(crate) fn summarize_failed_provider_lane_tool_request(
    turn: &ProviderTurn,
    trace: Option<&ToolBatchExecutionTrace>,
) -> Option<String> {
    let failed_tool_call_id = trace.and_then(first_failed_provider_lane_tool_call_id);
    if let Some(failed_tool_call_id) = failed_tool_call_id {
        let failed_intent = turn
            .tool_intents
            .iter()
            .find(|intent| intent.tool_call_id == failed_tool_call_id)?;
        return summarize_single_tool_followup_request(failed_intent);
    }

    match turn.tool_intents.as_slice() {
        [intent] => summarize_single_tool_followup_request(intent),
        [] => None,
        _ => summarize_tool_followup_request(&turn.tool_intents),
    }
}

fn first_failed_provider_lane_tool_call_id(trace: &ToolBatchExecutionTrace) -> Option<&str> {
    let failed_outcome = trace.intent_outcomes.iter().find(|intent_outcome| {
        !matches!(
            intent_outcome.status,
            ToolBatchExecutionIntentStatus::Completed
        )
    })?;
    Some(failed_outcome.tool_call_id.as_str())
}

fn tool_followup_request_entry(intent: &ToolIntent) -> Value {
    let canonical_tool_name = effective_followup_tool_name(intent);
    let visible_tool_name = effective_followup_visible_tool_name(intent);
    let request = effective_followup_request(intent);
    let request = sanitize_followup_request_summary(canonical_tool_name.as_str(), request);
    serde_json::json!({
        "tool": visible_tool_name,
        "request": request,
    })
}

fn sanitize_followup_request_summary(tool_name: &str, request: Value) -> Value {
    crate::tools::summarize_tool_request_for_display(tool_name, request)
}

pub(crate) fn effective_followup_tool_name(intent: &ToolIntent) -> String {
    let canonical_tool_name = crate::tools::canonical_tool_name(intent.tool_name.as_str());
    if canonical_tool_name != "tool.invoke" {
        return canonical_tool_name.to_owned();
    }

    if let Some((tool_name, _arguments)) =
        crate::tools::invoked_discoverable_tool_request(&intent.args_json)
    {
        return tool_name.to_owned();
    }

    intent
        .args_json
        .get("tool_id")
        .and_then(Value::as_str)
        .map(crate::tools::canonical_tool_name)
        .unwrap_or(canonical_tool_name)
        .to_owned()
}

pub(crate) fn effective_followup_visible_tool_name(intent: &ToolIntent) -> String {
    let canonical_tool_name = effective_followup_tool_name(intent);
    crate::tools::user_visible_tool_name(canonical_tool_name.as_str())
}

pub(crate) fn effective_followup_request(intent: &ToolIntent) -> Value {
    let canonical_tool_name = crate::tools::canonical_tool_name(intent.tool_name.as_str());
    if canonical_tool_name != "tool.invoke" {
        return crate::tools::normalize_shell_payload_for_request(
            canonical_tool_name,
            intent.args_json.clone(),
        );
    }

    let raw_tool_id = intent.args_json.get("tool_id").and_then(Value::as_str);
    let resolved_invoke = crate::tools::invoked_discoverable_tool_request(&intent.args_json);
    let (invoked_tool_name, request_payload) = match resolved_invoke {
        Some((tool_name, arguments)) => {
            let request_payload =
                strip_grouped_hidden_operation_from_request(raw_tool_id, arguments.clone());
            (tool_name, request_payload)
        }
        None => {
            let invoked_tool_name = raw_tool_id
                .map(crate::tools::canonical_tool_name)
                .unwrap_or(canonical_tool_name);
            let request_payload = intent
                .args_json
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| intent.args_json.clone());
            (invoked_tool_name, request_payload)
        }
    };

    crate::tools::normalize_shell_payload_for_request(invoked_tool_name, request_payload)
}

fn strip_grouped_hidden_operation_from_request(raw_tool_id: Option<&str>, request: Value) -> Value {
    let Some(raw_tool_id) = raw_tool_id.map(crate::tools::canonical_tool_name) else {
        return request;
    };
    let is_grouped_hidden_surface = crate::tools::is_tool_surface_id(raw_tool_id)
        && !crate::tools::is_provider_exposed_tool_name(raw_tool_id);
    if !is_grouped_hidden_surface {
        return request;
    }

    let Value::Object(mut request_object) = request else {
        return request;
    };
    request_object.remove("operation");
    Value::Object(request_object)
}
