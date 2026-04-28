use super::{SessionContext, ToolIntent, TurnFailure, tool_search_recovery_hint};

pub(super) fn effective_visible_tool_name(
    intent: &ToolIntent,
    descriptor: &crate::tools::ToolDescriptor,
) -> String {
    if descriptor.name != "tool.invoke" {
        return descriptor.name.to_owned();
    }

    crate::tools::invoked_discoverable_tool_request(&intent.args_json)
        .map(|(tool_name, _arguments)| tool_name.to_owned())
        .unwrap_or_else(|| descriptor.name.to_owned())
}

pub(super) fn provider_tool_denial_should_conceal_name(
    intent: &ToolIntent,
    descriptor: &crate::tools::ToolDescriptor,
    tool_is_visible: bool,
) -> bool {
    if !intent.source.starts_with("provider_") {
        return false;
    }

    if !descriptor.is_provider_exposed() {
        return true;
    }

    !tool_is_visible
        && descriptor.name == "tool.invoke"
        && effective_visible_tool_name(intent, descriptor) != descriptor.name
}

pub(super) fn concealed_provider_tool_denial() -> TurnFailure {
    let base_reason = "tool_not_found: requested tool is not available";
    let reason = provider_tool_denial_reason(base_reason, "provider_tool_call");
    TurnFailure::policy_denied_with_discovery_recovery("tool_not_found", reason)
}

pub(super) fn provider_tool_denial_reason(reason: &str, source: &str) -> String {
    if !source.starts_with("provider_") {
        return reason.to_owned();
    }

    let mut message = reason.to_owned();
    message.push_str(tool_search_recovery_hint());
    message
}

pub(super) fn tool_intent_is_visible(
    session_context: &SessionContext,
    intent: &ToolIntent,
    descriptor: &crate::tools::ToolDescriptor,
) -> bool {
    if descriptor.is_provider_exposed() {
        if descriptor.name != "tool.invoke" {
            return true;
        }

        let effective_name = effective_visible_tool_name(intent, descriptor);
        return effective_name == descriptor.name
            || session_context.tool_view.contains(effective_name.as_str());
    }

    if intent.source.starts_with("provider_") {
        return false;
    }

    session_context.tool_view.contains(descriptor.name)
}
