use std::collections::BTreeSet;

use loong_contracts::{Capability, ToolCoreRequest};
use serde_json::Value;

use super::routing::route_hidden_discoverable_tool_name;
use super::{
    BASH_EXEC_TOOL_NAME, HIDDEN_AGENT_TOOL_NAME, HIDDEN_CHANNEL_TOOL_NAME, HIDDEN_SKILLS_TOOL_NAME,
    HTTP_REQUEST_TOOL_NAME, ToolExecutionKind, ToolView, config_import, feishu, runtime_tool_view,
    tool_catalog, tool_surface,
};

pub fn canonical_tool_name(raw: &str) -> &str {
    let catalog = tool_catalog();
    if let Some(descriptor) = catalog.resolve(raw) {
        return descriptor.name;
    }
    #[cfg(feature = "feishu-integration")]
    if let Some(canonical) = feishu::canonical_feishu_tool_name(raw) {
        return canonical;
    }
    raw
}

pub(crate) fn required_capabilities_for_request(request: &ToolCoreRequest) -> BTreeSet<Capability> {
    required_capabilities_for_tool_name_and_payload(
        canonical_tool_name(request.tool_name.as_str()),
        &request.payload,
    )
}

pub(crate) fn required_capabilities_for_tool_name_and_payload(
    tool_name: &str,
    payload: &Value,
) -> BTreeSet<Capability> {
    let mut caps = BTreeSet::from([Capability::InvokeTool]);
    if tool_requires_network_egress(tool_name) {
        caps.insert(Capability::NetworkEgress);
    }
    match tool_name {
        "tool.invoke" => {
            let Some((invoked_tool_name, invoked_payload)) =
                invoked_discoverable_tool_request(payload)
            else {
                return caps;
            };
            return required_capabilities_for_tool_name_and_payload(
                invoked_tool_name,
                invoked_payload,
            );
        }
        "read" | "file.read" | "glob.search" | "content.search" => {
            caps.insert(Capability::FilesystemRead);
        }
        "memory" | "memory_search" | "memory_get" => {
            caps.insert(Capability::FilesystemRead);
        }
        "sessions_list"
        | "tasks_list"
        | "sessions_history"
        | "session_status"
        | "session_heads"
        | "session_path"
        | "session_children"
        | "session_artifacts"
        | "session_events"
        | "session_wait"
        | "task_status"
        | "task_wait"
        | "task_history"
        | "task_events"
        | "session_search"
        | "session_tool_policy_status" => {
            caps.insert(Capability::MemoryRead);
        }
        "write" | "file.write" | "file.edit" => {
            caps.insert(Capability::FilesystemWrite);
        }
        "exec" | BASH_EXEC_TOOL_NAME => {
            caps.insert(Capability::FilesystemRead);
            caps.insert(Capability::FilesystemWrite);
            caps.insert(Capability::NetworkEgress);
        }
        config_import::CONFIG_IMPORT_TOOL_NAME => {
            caps.insert(Capability::FilesystemRead);
            let mode_requires_write =
                config_import::config_import_mode_requires_write_value(payload);
            if mode_requires_write {
                caps.insert(Capability::FilesystemWrite);
            }
        }
        _ => {}
    }
    caps
}

pub(crate) fn invoked_discoverable_tool_request(payload: &Value) -> Option<(&str, &Value)> {
    let tool_id = payload
        .get("tool_id")
        .and_then(Value::as_str)
        .map(canonical_tool_name)?;
    if matches!(tool_id, "tool.search" | "tool.invoke") {
        return None;
    }
    let arguments = payload.get("arguments").unwrap_or(payload);
    let routed_hidden_tool_name = route_hidden_discoverable_tool_name(tool_id, arguments).ok();
    let resolved_tool_name = routed_hidden_tool_name.unwrap_or(tool_id);
    let resolved = resolve_tool_execution(resolved_tool_name)?;
    if is_provider_exposed_tool_name(resolved.canonical_name) {
        return None;
    }
    Some((resolved.canonical_name, arguments))
}

fn tool_requires_network_egress(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "web"
            | "browser"
            | HTTP_REQUEST_TOOL_NAME
            | "web.fetch"
            | "web.search"
            | "browser.open"
            | "browser.click"
            | "exec"
            | "external_skills.fetch"
            | "external_skills.source_search"
    )
}

pub fn is_known_tool_name(raw: &str) -> bool {
    if tool_catalog().resolve(raw).is_some() {
        return true;
    }
    let canonical_name = canonical_tool_name(raw);
    if matches!(
        canonical_name,
        HIDDEN_AGENT_TOOL_NAME | HIDDEN_SKILLS_TOOL_NAME | HIDDEN_CHANNEL_TOOL_NAME
    ) {
        return true;
    }
    if is_known_tool_name_in_view(raw, &runtime_tool_view()) {
        return true;
    }
    #[cfg(feature = "feishu-integration")]
    {
        feishu::is_known_feishu_tool_name(raw)
    }
    #[cfg(not(feature = "feishu-integration"))]
    {
        false
    }
}

pub fn is_known_tool_name_in_view(raw: &str, view: &ToolView) -> bool {
    let canonical_name = canonical_tool_name(raw);
    is_provider_exposed_tool_name(canonical_name) || view.contains(canonical_name)
}

pub fn is_provider_exposed_tool_name(raw: &str) -> bool {
    super::catalog::find_tool_catalog_entry(canonical_tool_name(raw))
        .is_some_and(|entry| entry.is_provider_exposed())
}

pub(crate) fn hidden_facade_tool_name_for_hidden_tool(raw: &str) -> Option<&'static str> {
    let canonical_name = canonical_tool_name(raw);
    tool_surface::hidden_facade_tool_name_for_hidden_tool(canonical_name)
}

pub(crate) fn direct_tool_name_for_hidden_tool(raw: &str) -> Option<&'static str> {
    let canonical_name = canonical_tool_name(raw);
    tool_surface::direct_tool_name_for_hidden_tool(canonical_name)
}

pub fn user_visible_tool_name(raw: &str) -> String {
    let canonical_name = canonical_tool_name(raw);

    if is_tool_surface_id(canonical_name) {
        return canonical_name.to_owned();
    }

    if let Some(direct_tool_name) = direct_tool_name_for_hidden_tool(canonical_name) {
        return direct_tool_name.to_owned();
    }

    canonical_name.to_owned()
}

pub(crate) fn model_visible_tool_name(raw: &str) -> String {
    let canonical_name = canonical_tool_name(raw);

    if let Some(hidden_facade_tool_name) = hidden_facade_tool_name_for_hidden_tool(canonical_name) {
        return hidden_facade_tool_name.to_owned();
    }

    user_visible_tool_name(canonical_name)
}

pub(crate) fn is_tool_surface_id(surface_id: &str) -> bool {
    tool_surface::is_tool_surface_id(surface_id)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedToolExecution {
    pub canonical_name: &'static str,
    pub execution_kind: ToolExecutionKind,
}

pub(crate) fn resolve_tool_execution(raw: &str) -> Option<ResolvedToolExecution> {
    let catalog = tool_catalog();
    if let Some(descriptor) = catalog.resolve(raw) {
        return Some(ResolvedToolExecution {
            canonical_name: descriptor.name,
            execution_kind: descriptor.execution_kind,
        });
    }

    let canonical_name = canonical_tool_name(raw);
    if canonical_name == HIDDEN_AGENT_TOOL_NAME {
        return Some(ResolvedToolExecution {
            canonical_name: HIDDEN_AGENT_TOOL_NAME,
            execution_kind: ToolExecutionKind::Core,
        });
    }
    if canonical_name == HIDDEN_SKILLS_TOOL_NAME {
        return Some(ResolvedToolExecution {
            canonical_name: HIDDEN_SKILLS_TOOL_NAME,
            execution_kind: ToolExecutionKind::Core,
        });
    }
    if canonical_name == HIDDEN_CHANNEL_TOOL_NAME {
        return Some(ResolvedToolExecution {
            canonical_name: HIDDEN_CHANNEL_TOOL_NAME,
            execution_kind: ToolExecutionKind::Core,
        });
    }

    #[cfg(feature = "feishu-integration")]
    if let Some(canonical_name) = feishu::canonical_feishu_tool_name(raw) {
        return Some(ResolvedToolExecution {
            canonical_name,
            execution_kind: ToolExecutionKind::Core,
        });
    }
    None
}
