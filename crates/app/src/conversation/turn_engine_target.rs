use loong_contracts::ToolCoreRequest;

use super::{ConversationRuntimeBinding, ToolDescriptor, ToolExecutionKind, ToolIntent};
use crate::tools::{ResolvedToolExecution, ToolSchedulingClass};

#[derive(Debug, Clone)]
pub(super) struct EffectiveToolTarget {
    pub(super) execution_kind: ToolExecutionKind,
    pub(super) request: ToolCoreRequest,
    pub(super) intent: ToolIntent,
    pub(super) tool_name: String,
}

#[derive(Debug, Clone)]
pub(super) struct EffectiveToolMetadata {
    pub(super) execution_kind: ToolExecutionKind,
    pub(super) request: ToolCoreRequest,
    pub(super) intent: ToolIntent,
    pub(super) tool_name: String,
    pub(super) descriptor: ToolDescriptor,
    pub(super) capability_action_class: crate::tools::CapabilityActionClass,
    pub(super) scheduling_class: ToolSchedulingClass,
}

#[derive(Debug, Clone)]
pub(super) struct EffectiveToolMetadataError {
    pub(super) effective_target: EffectiveToolTarget,
}

fn resolve_effective_tool_target(
    resolved_tool: ResolvedToolExecution,
    mut request: ToolCoreRequest,
    mut normalized_intent: ToolIntent,
    _original_intent: &ToolIntent,
) -> EffectiveToolTarget {
    let mut execution_kind = resolved_tool.execution_kind;
    let mut tool_name = resolved_tool.canonical_name.to_owned();

    if resolved_tool.canonical_name == "browser"
        && let Ok(routed_tool_name) =
            crate::tools::route_direct_tool_name("browser", &request.payload)
        && matches!(
            routed_tool_name,
            "browser.companion.click" | "browser.companion.type"
        )
        && let Some(routed_tool) = crate::tools::resolve_tool_execution(routed_tool_name)
    {
        execution_kind = routed_tool.execution_kind;
        tool_name = routed_tool.canonical_name.to_owned();
        request.tool_name = tool_name.clone();
        normalized_intent.tool_name = tool_name.clone();
    }

    EffectiveToolTarget {
        execution_kind,
        request,
        intent: normalized_intent,
        tool_name,
    }
}

fn resolve_effective_tool_descriptor(effective_tool_name: &str) -> Option<ToolDescriptor> {
    crate::tools::tool_catalog()
        .resolve(effective_tool_name)
        .copied()
}

pub(super) fn resolve_effective_tool_metadata(
    resolved_tool: ResolvedToolExecution,
    request: ToolCoreRequest,
    normalized_intent: ToolIntent,
    original_intent: &ToolIntent,
) -> Result<EffectiveToolMetadata, Box<EffectiveToolMetadataError>> {
    let effective_target =
        resolve_effective_tool_target(resolved_tool, request, normalized_intent, original_intent);
    let Some(descriptor) = resolve_effective_tool_descriptor(effective_target.tool_name.as_str())
    else {
        return Err(Box::new(EffectiveToolMetadataError { effective_target }));
    };

    Ok(EffectiveToolMetadata {
        execution_kind: effective_target.execution_kind,
        request: effective_target.request,
        intent: effective_target.intent,
        tool_name: effective_target.tool_name,
        capability_action_class: descriptor.capability_action_class(),
        scheduling_class: descriptor.scheduling_class(),
        descriptor,
    })
}

pub(super) fn prepare_conversation_kernel_tool_request(
    request: ToolCoreRequest,
    _binding: ConversationRuntimeBinding<'_>,
    _intent: &ToolIntent,
) -> ToolCoreRequest {
    request
}
