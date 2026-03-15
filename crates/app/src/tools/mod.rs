use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use loongclaw_contracts::{Capability, ToolCoreOutcome, ToolCoreRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::KernelContext;
use crate::config::ToolConfig;
use crate::memory::runtime_config::MemoryRuntimeConfig;

pub(crate) mod approval;
mod catalog;
mod claw_import;
pub(crate) mod delegate;
mod external_skills;
mod file;
pub mod file_policy_ext;
mod kernel_adapter;
pub(crate) mod messaging;
mod provider_switch;
pub mod runtime_config;
mod session;
mod shell;
pub mod shell_policy_ext;

pub use catalog::{
    ToolApprovalMode, ToolAvailability, ToolCatalog, ToolDescriptor, ToolExecutionKind,
    ToolGovernanceProfile, ToolGovernanceScope, ToolRiskClass, ToolView,
    delegate_child_tool_view_for_config, delegate_child_tool_view_for_config_with_delegate,
    governance_profile_for_descriptor, governance_profile_for_tool_name,
    planned_delegate_child_tool_view, planned_root_tool_view, runtime_tool_view,
    runtime_tool_view_for_config, tool_catalog,
};
pub use kernel_adapter::MvpToolAdapter;

/// Execute a tool request, routing through the kernel for
/// policy enforcement and audit recording.
///
/// All requests are dispatched via `kernel.execute_tool_core` which
/// enforces the derived capability set for the effective tool request, runs
/// policy extensions, and records audit events.
/// `kernel.execute_tool_core` which enforces the derived capability set
/// for the effective tool request and records audit events.
pub async fn execute_tool(
    request: ToolCoreRequest,
    kernel_ctx: &KernelContext,
) -> Result<ToolCoreOutcome, String> {
    let request = prepare_kernel_tool_request(
        request,
        &kernel_ctx.token.allowed_capabilities,
        Some(kernel_ctx.token.token_id.as_str()),
        None,
        None,
    );
    let caps = required_capabilities_for_request(&request);
    kernel_ctx
        .kernel
        .execute_tool_core(
            kernel_ctx.pack_id(),
            &kernel_ctx.token,
            &caps,
            None,
            request,
        )
        .await
        .map_err(|e| format!("{e}"))
}

pub fn execute_tool_core(request: ToolCoreRequest) -> Result<ToolCoreOutcome, String> {
    execute_tool_core_with_config(request, runtime_config::get_tool_runtime_config())
}

pub fn execute_app_tool_with_config(
    request: ToolCoreRequest,
    current_session_id: &str,
    memory_config: &MemoryRuntimeConfig,
    tool_config: &ToolConfig,
) -> Result<ToolCoreOutcome, String> {
    let canonical_name = canonical_tool_name(request.tool_name.as_str());
    let request = ToolCoreRequest {
        tool_name: canonical_name.to_owned(),
        payload: request.payload,
    };

    match canonical_name {
        "approval_requests_list" | "approval_request_status" | "approval_request_resolve" => {
            approval::execute_approval_tool_with_policies(
                request,
                current_session_id,
                memory_config,
                tool_config,
            )
        }
        "sessions_list" | "sessions_history" | "session_status" | "session_events"
        | "session_archive" | "session_cancel" | "session_recover" => {
            session::execute_session_tool_with_policies(
                request,
                current_session_id,
                memory_config,
                tool_config,
            )
        }
        _ => Err(format!(
            "app_tool_not_found: unknown app tool `{}`",
            request.tool_name
        )),
    }
}

pub async fn wait_for_session_with_config(
    payload: Value,
    current_session_id: &str,
    memory_config: &MemoryRuntimeConfig,
    tool_config: &ToolConfig,
) -> Result<ToolCoreOutcome, String> {
    #[cfg(not(feature = "memory-sqlite"))]
    {
        let _ = (payload, current_session_id, memory_config, tool_config);
        return Err(
            "session tools require sqlite memory support (enable feature `memory-sqlite`)"
                .to_owned(),
        );
    }

    #[cfg(feature = "memory-sqlite")]
    {
        if !tool_config.sessions.enabled {
            return Err("app_tool_disabled: session tools are disabled by config".to_owned());
        }
        session::wait_for_session_tool_with_policies(
            payload,
            current_session_id,
            memory_config,
            tool_config,
        )
        .await
    }
}

/// Normalize a path by resolving `.` and `..` components without filesystem access.
///
/// - `Prefix` and `RootDir` are tracked separately so `..` can never "eat" them.
/// - `..` past the filesystem root (or volume root on Windows) is silently dropped.
/// - Relative paths preserve leading `..` components (e.g. `../../foo` stays as-is).
///
/// All three path-handling modules (`file`, `claw_import`, `file_policy_ext`) use
/// this single implementation to avoid divergence.
pub(super) fn normalize_without_fs(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut parts: Vec<OsString> = Vec::new();
    let mut prefix: Option<OsString> = None;
    let mut has_root = false;

    for component in path.components() {
        match component {
            Component::Prefix(value) => prefix = Some(value.as_os_str().to_owned()),
            Component::RootDir => has_root = true,
            Component::CurDir => {}
            Component::ParentDir => {
                if let Some(last) = parts.last() {
                    if last != ".." {
                        let _ = parts.pop();
                    } else if !has_root {
                        parts.push(OsString::from(".."));
                    }
                } else if !has_root {
                    parts.push(OsString::from(".."));
                }
            }
            Component::Normal(value) => parts.push(value.to_owned()),
        }
    }

    let mut normalized = PathBuf::new();
    if let Some(prefix) = prefix {
        normalized.push(prefix);
    }
    if has_root {
        normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR));
    }
    for part in parts {
        normalized.push(part);
    }
    if normalized.as_os_str().is_empty() {
        if has_root {
            PathBuf::from(std::path::MAIN_SEPARATOR_STR)
        } else {
            PathBuf::from(".")
        }
    } else {
        normalized
    }
}

const TOOL_SEARCH_GRANTED_CAPABILITIES_FIELD: &str = "_granted_capabilities";
const TOOL_LEASE_TOKEN_ID_FIELD: &str = "_lease_token_id";
const TOOL_LEASE_SESSION_ID_FIELD: &str = "_lease_session_id";
const TOOL_LEASE_TURN_ID_FIELD: &str = "_lease_turn_id";

pub(crate) fn prepare_kernel_tool_request(
    mut request: ToolCoreRequest,
    granted_capabilities: &BTreeSet<Capability>,
    token_id: Option<&str>,
    session_id: Option<&str>,
    turn_id: Option<&str>,
) -> ToolCoreRequest {
    let canonical_tool_name = canonical_tool_name(request.tool_name.as_str());
    if !matches!(canonical_tool_name, "tool.search" | "tool.invoke") {
        return request;
    }

    if let Value::Object(payload) = &mut request.payload {
        if canonical_tool_name == "tool.search" {
            let granted =
                serde_json::to_value(granted_capabilities.iter().copied().collect::<Vec<_>>())
                    .unwrap_or_else(|_| Value::Array(Vec::new()));
            payload.insert(TOOL_SEARCH_GRANTED_CAPABILITIES_FIELD.to_owned(), granted);
        }
        inject_tool_lease_binding(payload, token_id, session_id, turn_id);
    }

    request
}

fn inject_tool_lease_binding(
    payload: &mut serde_json::Map<String, Value>,
    token_id: Option<&str>,
    session_id: Option<&str>,
    turn_id: Option<&str>,
) {
    if let Some(token_id) = token_id {
        payload.insert(
            TOOL_LEASE_TOKEN_ID_FIELD.to_owned(),
            Value::String(token_id.to_owned()),
        );
    }
    if let Some(session_id) = session_id {
        payload.insert(
            TOOL_LEASE_SESSION_ID_FIELD.to_owned(),
            Value::String(session_id.to_owned()),
        );
    }
    if let Some(turn_id) = turn_id {
        payload.insert(
            TOOL_LEASE_TURN_ID_FIELD.to_owned(),
            Value::String(turn_id.to_owned()),
        );
    }
}

pub(crate) fn required_capabilities_for_request(request: &ToolCoreRequest) -> BTreeSet<Capability> {
    required_capabilities_for_tool_name_and_payload(
        canonical_tool_name(request.tool_name.as_str()),
        &request.payload,
    )
}

fn required_capabilities_for_tool_name_and_payload(
    tool_name: &str,
    payload: &Value,
) -> BTreeSet<Capability> {
    let mut caps = BTreeSet::from([Capability::InvokeTool]);
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
        "file.read" => {
            caps.insert(Capability::FilesystemRead);
        }
        "file.write" => {
            caps.insert(Capability::FilesystemWrite);
        }
        "claw.import" => {
            caps.insert(Capability::FilesystemRead);
            if claw_import_mode_requires_write(payload) {
                caps.insert(Capability::FilesystemWrite);
            }
        }
        _ => {}
    }
    caps
}

fn invoked_discoverable_tool_request(payload: &Value) -> Option<(&str, &Value)> {
    let tool_id = payload
        .get("tool_id")
        .and_then(Value::as_str)
        .map(canonical_tool_name)?;
    if matches!(tool_id, "tool.search" | "tool.invoke") {
        return None;
    }
    let entry = catalog::find_tool_catalog_entry(tool_id)?;
    if !entry.is_discoverable() {
        return None;
    }
    Some((tool_id, payload.get("arguments").unwrap_or(payload)))
}

fn claw_import_mode_requires_write(payload: &Value) -> bool {
    matches!(
        payload
            .get("mode")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("plan"),
        "apply" | "apply_selected" | "rollback_last_apply"
    )
}

pub fn canonical_tool_name(raw: &str) -> &str {
    tool_catalog()
        .resolve(raw)
        .map(|descriptor| descriptor.name)
        .unwrap_or(raw)
}

pub fn is_known_tool_name(raw: &str) -> bool {
    catalog::find_tool_catalog_entry(canonical_tool_name(raw)).is_some()
}

pub fn is_known_tool_name_in_view(raw: &str, view: &ToolView) -> bool {
    let canonical_name = canonical_tool_name(raw);
    is_provider_exposed_tool_name(canonical_name) || view.contains(canonical_name)
}

pub fn is_provider_exposed_tool_name(raw: &str) -> bool {
    catalog::find_tool_catalog_entry(canonical_tool_name(raw))
        .is_some_and(|entry| entry.is_provider_core())
}

pub fn execute_tool_core_with_config(
    request: ToolCoreRequest,
    config: &runtime_config::ToolRuntimeConfig,
) -> Result<ToolCoreOutcome, String> {
    let canonical_name = canonical_tool_name(request.tool_name.as_str());
    let request = ToolCoreRequest {
        tool_name: canonical_name.to_owned(),
        payload: request.payload,
    };
    match canonical_name {
        "tool.search" => execute_tool_search_tool_with_config(request, config),
        "tool.invoke" => execute_tool_invoke_tool_with_config(request, config),
        _ => execute_discoverable_tool_core_with_config(request, config),
    }
}

fn execute_discoverable_tool_core_with_config(
    request: ToolCoreRequest,
    config: &runtime_config::ToolRuntimeConfig,
) -> Result<ToolCoreOutcome, String> {
    match request.tool_name.as_str() {
        "claw.import" => claw_import::execute_claw_import_tool_with_config(request, config),
        "external_skills.inspect" => {
            external_skills::execute_external_skills_inspect_tool_with_config(request, config)
        }
        "external_skills.install" => {
            external_skills::execute_external_skills_install_tool_with_config(request, config)
        }
        "external_skills.invoke" => {
            external_skills::execute_external_skills_invoke_tool_with_config(request, config)
        }
        "external_skills.list" => {
            external_skills::execute_external_skills_list_tool_with_config(request, config)
        }
        "external_skills.policy" => {
            external_skills::execute_external_skills_policy_tool_with_config(request, config)
        }
        "external_skills.fetch" => {
            external_skills::execute_external_skills_fetch_tool_with_config(request, config)
        }
        "external_skills.remove" => {
            external_skills::execute_external_skills_remove_tool_with_config(request, config)
        }
        "shell.exec" => shell::execute_shell_tool_with_config(request, config),
        "file.read" => file::execute_file_read_tool_with_config(request, config),
        "file.write" => file::execute_file_write_tool_with_config(request, config),
        "provider.switch" => {
            provider_switch::execute_provider_switch_tool_with_config(request, config)
        }
        _ => Err(format!(
            "tool_not_found: unknown tool `{}`",
            request.tool_name
        )),
    }
}

/// Tool registry entry for capability snapshot disclosure.
#[derive(Debug, Clone)]
pub struct ToolRegistryEntry {
    pub name: &'static str,
    pub description: &'static str,
}

/// Returns a sorted list of all registered tools, gated by feature flags.
pub fn tool_registry() -> Vec<ToolRegistryEntry> {
    let mut entries: Vec<ToolRegistryEntry> = catalog::discoverable_tool_catalog()
        .into_iter()
        .map(|entry| ToolRegistryEntry {
            name: entry.canonical_name,
            description: entry.summary,
        })
        .collect();
    entries.sort_by_key(|entry| entry.name);
    entries
}

/// Produce a deterministic text block listing available tools,
/// suitable for appending to the system prompt.
pub fn capability_snapshot() -> String {
    capability_snapshot_with_config(runtime_config::get_tool_runtime_config())
}

pub fn capability_snapshot_with_config(config: &runtime_config::ToolRuntimeConfig) -> String {
    capability_snapshot_for_view_with_config(&runtime_tool_view(), config)
}

pub fn capability_snapshot_for_view(view: &ToolView) -> String {
    capability_snapshot_for_view_with_config(view, runtime_config::get_tool_runtime_config())
}

pub(crate) fn capability_snapshot_for_view_with_config(
    _view: &ToolView,
    _config: &runtime_config::ToolRuntimeConfig,
) -> String {
    let mut lines = vec!["[tool_discovery_runtime]".to_owned()];
    for entry in catalog::provider_core_tool_catalog() {
        lines.push(format!("- {}: {}", entry.canonical_name, entry.summary));
    }
    lines.push(
        "Non-core tools are intentionally hidden until discovered with tool.search.".to_owned(),
    );
    lines.join("\n")
}

/// Provider request tool schema for function-calling capable models.
///
/// The output shape matches OpenAI-compatible `tools=[{type:function,...}]`.
/// Order is deterministic for stable prompting/tests.
pub fn provider_tool_definitions() -> Vec<Value> {
    provider_tool_definitions_for_view(&runtime_tool_view())
}

pub fn try_provider_tool_definitions_for_view(_view: &ToolView) -> Result<Vec<Value>, String> {
    Ok(provider_tool_definitions_for_view(_view))
}

fn provider_tool_definitions_for_view(_view: &ToolView) -> Vec<Value> {
    let catalog = tool_catalog();
    let mut tools = catalog
        .descriptors()
        .iter()
        .filter(|descriptor| {
            descriptor.is_provider_core() && descriptor.availability == ToolAvailability::Runtime
        })
        .map(ToolDescriptor::provider_definition)
        .collect::<Vec<_>>();
    tools.sort_by(|left, right| tool_function_name(left).cmp(tool_function_name(right)));
    tools
}

pub fn tool_parameter_schema_types() -> BTreeMap<String, BTreeMap<String, &'static str>> {
    let mut tools_by_name = BTreeMap::<String, BTreeMap<String, &'static str>>::new();
    for entry in catalog::all_tool_catalog() {
        let parameters = entry
            .parameter_types
            .iter()
            .map(|(parameter_name, parameter_type)| ((*parameter_name).to_owned(), *parameter_type))
            .collect::<BTreeMap<_, _>>();
        if !parameters.is_empty() {
            tools_by_name.insert(entry.canonical_name.to_owned(), parameters);
        }
    }
    tools_by_name
}

const TOOL_LEASE_TTL_SECONDS: u64 = 300;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolLeaseClaims {
    tool_id: String,
    catalog_digest: String,
    expires_at_unix: u64,
    token_id: Option<String>,
    session_id: Option<String>,
    turn_id: Option<String>,
}

fn execute_tool_search_tool_with_config(
    request: ToolCoreRequest,
    config: &runtime_config::ToolRuntimeConfig,
) -> Result<ToolCoreOutcome, String> {
    let payload = request
        .payload
        .as_object()
        .ok_or_else(|| "tool.search payload must be an object".to_owned())?;
    let query = payload
        .get("query")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "tool.search requires payload.query".to_owned())?;
    let limit = payload
        .get("limit")
        .and_then(Value::as_u64)
        .map(|value| value.clamp(1, 8) as usize)
        .unwrap_or(5);
    let granted_capabilities = payload
        .get(TOOL_SEARCH_GRANTED_CAPABILITIES_FIELD)
        .cloned()
        .and_then(|value| serde_json::from_value::<BTreeSet<Capability>>(value).ok());

    let query_normalized = query.to_ascii_lowercase();
    let tokens = tokenize_search_query(query_normalized.as_str());
    let mut ranked: Vec<(u32, Vec<String>, catalog::ToolCatalogEntry)> =
        catalog::discoverable_tool_catalog()
            .into_iter()
            .filter(|entry| tool_search_entry_is_runtime_usable(*entry, config))
            .filter(|entry| {
                tool_search_entry_is_capability_usable(*entry, granted_capabilities.as_ref())
            })
            .filter_map(|entry| {
                let (score, why) = search_score(entry, query_normalized.as_str(), &tokens);
                if score == 0 {
                    None
                } else {
                    Some((score, why, entry))
                }
            })
            .collect();
    ranked.sort_by(|(left_score, _, left), (right_score, _, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.canonical_name.cmp(right.canonical_name))
    });

    let results: Vec<Value> = ranked
        .into_iter()
        .take(limit)
        .map(|(_score, why, entry)| {
            json!({
                "tool_id": entry.canonical_name,
                "summary": entry.summary,
                "argument_hint": entry.argument_hint,
                "required_fields": entry.required_fields,
                "tags": entry.tags,
                "why": why,
                "lease": issue_tool_lease(entry.canonical_name, payload),
            })
        })
        .collect();

    Ok(ToolCoreOutcome {
        status: "ok".to_owned(),
        payload: json!({
            "adapter": "core-tools",
            "tool_name": request.tool_name,
            "query": query,
            "returned": results.len(),
            "results": results,
        }),
    })
}

fn tool_search_entry_is_runtime_usable(
    entry: catalog::ToolCatalogEntry,
    config: &runtime_config::ToolRuntimeConfig,
) -> bool {
    match entry.canonical_name {
        "shell.exec" => {
            !config.shell_allow.is_empty()
                || matches!(
                    config.shell_default_mode,
                    crate::tools::shell_policy_ext::ShellPolicyDefault::Allow
                )
        }
        "external_skills.fetch"
        | "external_skills.install"
        | "external_skills.inspect"
        | "external_skills.invoke"
        | "external_skills.list"
        | "external_skills.remove" => config.external_skills.enabled,
        _ => true,
    }
}

fn tool_search_entry_is_capability_usable(
    entry: catalog::ToolCatalogEntry,
    granted_capabilities: Option<&BTreeSet<Capability>>,
) -> bool {
    let Some(granted_capabilities) = granted_capabilities else {
        return true;
    };
    let required =
        required_capabilities_for_tool_name_and_payload(entry.canonical_name, &json!({}));
    required
        .iter()
        .all(|capability| granted_capabilities.contains(capability))
}

pub(crate) fn resolve_tool_invoke_request(
    request: &ToolCoreRequest,
) -> Result<(catalog::ToolCatalogEntry, ToolCoreRequest), String> {
    if canonical_tool_name(request.tool_name.as_str()) != "tool.invoke" {
        return Err(format!(
            "tool_invoke_required: expected `tool.invoke`, got `{}`",
            request.tool_name
        ));
    }

    let payload = request
        .payload
        .as_object()
        .ok_or_else(|| "tool.invoke payload must be an object".to_owned())?;
    let tool_id = payload
        .get("tool_id")
        .and_then(Value::as_str)
        .map(canonical_tool_name)
        .ok_or_else(|| "tool.invoke requires payload.tool_id".to_owned())?;
    let lease = payload
        .get("lease")
        .and_then(Value::as_str)
        .ok_or_else(|| "tool.invoke requires payload.lease".to_owned())?;
    let arguments = payload
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !arguments.is_object() {
        return Err("tool.invoke payload.arguments must be an object".to_owned());
    }

    let entry = catalog::find_tool_catalog_entry(tool_id)
        .ok_or_else(|| format!("tool_not_found: unknown tool `{tool_id}`"))?;
    if !entry.is_discoverable() {
        return Err(format!(
            "tool_not_provider_exposed: {tool_id} must be called directly as a core tool"
        ));
    }
    validate_tool_lease(tool_id, lease, payload)?;

    Ok((
        entry,
        ToolCoreRequest {
            tool_name: tool_id.to_owned(),
            payload: arguments,
        },
    ))
}

fn execute_tool_invoke_tool_with_config(
    request: ToolCoreRequest,
    config: &runtime_config::ToolRuntimeConfig,
) -> Result<ToolCoreOutcome, String> {
    let (entry, effective_request) = resolve_tool_invoke_request(&request)?;
    match entry.execution_kind {
        ToolExecutionKind::Core => {
            execute_discoverable_tool_core_with_config(effective_request, config)
        }
        ToolExecutionKind::App => Err(format!(
            "tool_requires_app_dispatcher: {}",
            entry.canonical_name
        )),
    }
}

fn tokenize_search_query(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect()
}

fn search_score(
    entry: catalog::ToolCatalogEntry,
    query: &str,
    tokens: &[String],
) -> (u32, Vec<String>) {
    let name = entry.canonical_name.to_ascii_lowercase();
    let summary = entry.summary.to_ascii_lowercase();
    let argument_hint = entry.argument_hint.to_ascii_lowercase();
    let tags = entry.tags.join(" ").to_ascii_lowercase();

    let mut score = 0u32;
    let mut why = Vec::new();

    if name.contains(query) {
        score += 50;
        why.push("name_match".to_owned());
    }
    if summary.contains(query) {
        score += 30;
        why.push("summary_match".to_owned());
    }
    if argument_hint.contains(query) {
        score += 20;
        why.push("argument_match".to_owned());
    }

    for token in tokens {
        if name.contains(token) {
            score += 16;
            why.push(format!("name:{token}"));
        }
        if summary.contains(token) {
            score += 10;
            why.push(format!("summary:{token}"));
        }
        if argument_hint.contains(token) {
            score += 8;
            why.push(format!("argument:{token}"));
        }
        if tags.contains(token) {
            score += 12;
            why.push(format!("tag:{token}"));
        }
    }

    (score, why)
}

fn issue_tool_lease(tool_id: &str, payload: &serde_json::Map<String, Value>) -> String {
    let binding = extract_tool_lease_binding(payload);
    let claims = ToolLeaseClaims {
        tool_id: tool_id.to_owned(),
        catalog_digest: tool_catalog_digest(),
        expires_at_unix: now_unix_seconds().saturating_add(TOOL_LEASE_TTL_SECONDS),
        token_id: binding.token_id,
        session_id: binding.session_id,
        turn_id: binding.turn_id,
    };
    let claims_bytes = serde_json::to_vec(&claims).unwrap_or_default();
    let encoded_claims = URL_SAFE_NO_PAD.encode(claims_bytes);
    let signature = sign_tool_lease(encoded_claims.as_str());
    format!("{encoded_claims}.{signature}")
}

#[allow(dead_code)]
pub(crate) fn bridge_provider_tool_call_with_scope(
    tool_name: &str,
    args_json: Value,
    session_id: Option<&str>,
    turn_id: Option<&str>,
) -> (String, Value) {
    let canonical_name = canonical_tool_name(tool_name).to_owned();
    let Some(entry) = catalog::find_tool_catalog_entry(canonical_name.as_str()) else {
        return (canonical_name, args_json);
    };
    if !entry.is_discoverable() {
        return (canonical_name, args_json);
    }
    let mut lease_payload = serde_json::Map::new();
    inject_tool_lease_binding(&mut lease_payload, None, session_id, turn_id);
    (
        "tool.invoke".to_owned(),
        json!({
            "tool_id": entry.canonical_name,
            "lease": issue_tool_lease(entry.canonical_name, &lease_payload),
            "arguments": args_json,
        }),
    )
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn synthesize_test_provider_tool_call(
    tool_name: &str,
    args_json: Value,
) -> (String, Value) {
    bridge_provider_tool_call_with_scope(tool_name, args_json, None, None)
}

#[cfg(test)]
pub(crate) fn synthesize_test_provider_tool_call_with_scope(
    tool_name: &str,
    args_json: Value,
    session_id: Option<&str>,
    turn_id: Option<&str>,
) -> (String, Value) {
    bridge_provider_tool_call_with_scope(tool_name, args_json, session_id, turn_id)
}

fn validate_tool_lease(
    expected_tool_id: &str,
    lease: &str,
    payload: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    let Some((encoded_claims, signature)) = lease.split_once('.') else {
        return Err("invalid_tool_lease: malformed lease".to_owned());
    };
    let expected_signature = sign_tool_lease(encoded_claims);
    if expected_signature != signature {
        return Err("invalid_tool_lease: signature mismatch".to_owned());
    }
    let claims_bytes = URL_SAFE_NO_PAD
        .decode(encoded_claims)
        .map_err(|error| format!("invalid_tool_lease: claims decode failed: {error}"))?;
    let claims: ToolLeaseClaims = serde_json::from_slice(&claims_bytes)
        .map_err(|error| format!("invalid_tool_lease: claims parse failed: {error}"))?;
    if claims.tool_id != expected_tool_id {
        return Err("invalid_tool_lease: tool mismatch".to_owned());
    }
    if claims.catalog_digest != tool_catalog_digest() {
        return Err("invalid_tool_lease: catalog mismatch".to_owned());
    }
    if claims.expires_at_unix <= now_unix_seconds() {
        return Err("invalid_tool_lease: expired lease".to_owned());
    }
    let binding = extract_tool_lease_binding(payload);
    if claims.token_id.is_some() && claims.token_id != binding.token_id {
        return Err("invalid_tool_lease: token mismatch".to_owned());
    }
    if claims.session_id.is_some() && claims.session_id != binding.session_id {
        return Err("invalid_tool_lease: session mismatch".to_owned());
    }
    if claims.turn_id.is_some() && claims.turn_id != binding.turn_id {
        return Err("invalid_tool_lease: turn mismatch".to_owned());
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
struct ToolLeaseBinding {
    token_id: Option<String>,
    session_id: Option<String>,
    turn_id: Option<String>,
}

fn extract_tool_lease_binding(payload: &serde_json::Map<String, Value>) -> ToolLeaseBinding {
    ToolLeaseBinding {
        token_id: payload
            .get(TOOL_LEASE_TOKEN_ID_FIELD)
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        session_id: payload
            .get(TOOL_LEASE_SESSION_ID_FIELD)
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        turn_id: payload
            .get(TOOL_LEASE_TURN_ID_FIELD)
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    }
}

fn sign_tool_lease(encoded_claims: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(tool_lease_secret().as_bytes());
    hasher.update(b":");
    hasher.update(encoded_claims.as_bytes());
    let digest = hasher.finalize();
    format!("{digest:x}")
}

fn tool_catalog_digest() -> String {
    let payload = serde_json::to_vec(&catalog::all_tool_catalog()).unwrap_or_default();
    let digest = Sha256::digest(payload);
    format!("{digest:x}")
}

fn tool_lease_secret() -> &'static str {
    static SECRET: OnceLock<String> = OnceLock::new();
    SECRET.get_or_init(|| {
        let seed = format!("tool-lease:{}:{}", std::process::id(), now_unix_seconds());
        let digest = Sha256::digest(seed.as_bytes());
        format!("{digest:x}")
    })
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn tool_function_name(tool: &Value) -> &str {
    tool.get("function")
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
}

#[allow(dead_code)]
fn _shape_examples() -> BTreeMap<&'static str, Value> {
    BTreeMap::from([
        (
            "claw.import",
            json!({
                "input_path": "/tmp/nanobot-workspace",
                "mode": "plan",
                "source": "auto"
            }),
        ),
        (
            "shell.exec",
            json!({
                "command": "echo",
                "args": ["hello"]
            }),
        ),
        (
            "external_skills.policy",
            json!({
                "action": "set",
                "policy_update_approved": true,
                "enabled": true,
                "require_download_approval": true,
                "allowed_domains": ["skills.sh"],
                "blocked_domains": ["*.evil.example"]
            }),
        ),
        (
            "external_skills.fetch",
            json!({
                "url": "https://skills.sh/packages/demo-skill.tar.gz",
                "approval_granted": true
            }),
        ),
        (
            "file.read",
            json!({
                "path": "README.md",
                "max_bytes": 4096
            }),
        ),
        (
            "file.write",
            json!({
                "path": "notes.txt",
                "content": "hello",
                "create_dirs": true
            }),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_tool_runtime_config(root: PathBuf) -> runtime_config::ToolRuntimeConfig {
        runtime_config::ToolRuntimeConfig {
            shell_allow: BTreeSet::from(["echo".to_owned(), "cat".to_owned(), "ls".to_owned()]),
            shell_deny: BTreeSet::new(),
            shell_default_mode: crate::tools::shell_policy_ext::ShellPolicyDefault::Deny,
            file_root: Some(root),
            config_path: None,
            external_skills: runtime_config::ExternalSkillsRuntimePolicy {
                enabled: true,
                require_download_approval: true,
                allowed_domains: BTreeSet::new(),
                blocked_domains: BTreeSet::new(),
                install_root: None,
                auto_expose_installed: false,
            },
        }
    }

    #[test]
    fn capability_snapshot_is_deterministic() {
        let snapshot = capability_snapshot();
        assert!(snapshot.starts_with("[tool_discovery_runtime]"));
        assert!(snapshot.contains("- tool.search: Discover non-core tools"));
        assert!(snapshot.contains("- tool.invoke: Invoke a discovered non-core tool"));
        assert!(!snapshot.contains("shell.exec"));
        assert!(!snapshot.contains("file.read"));

        // Verify determinism: two calls produce identical output.
        let snapshot2 = capability_snapshot();
        assert_eq!(snapshot, snapshot2);
    }

    #[test]
    fn capability_snapshot_stays_compact_when_external_skills_are_installed() {
        use std::{
            fs,
            path::{Path, PathBuf},
            time::{SystemTime, UNIX_EPOCH},
        };

        fn unique_temp_dir(prefix: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("{prefix}-{nanos}"))
        }

        fn write_file(root: &Path, relative: &str, content: &str) {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(path, content).expect("write fixture");
        }

        let root = unique_temp_dir("loongclaw-tool-capability-snapshot-skills");
        fs::create_dir_all(&root).expect("create fixture root");
        write_file(
            &root,
            "skills/demo-skill/SKILL.md",
            "# Demo Skill\n\nUse this skill for explicit verification.\n",
        );

        let config = test_tool_runtime_config(root.clone());
        execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "external_skills.install".to_owned(),
                payload: json!({
                    "path": "skills/demo-skill"
                }),
            },
            &config,
        )
        .expect("install should succeed");

        let snapshot = capability_snapshot_with_config(&config);
        assert!(snapshot.starts_with("[tool_discovery_runtime]"));
        assert!(!snapshot.contains("[available_external_skills]"));
        assert!(!snapshot.contains("demo-skill"));
        assert!(!snapshot.contains("external_skills.invoke"));

        fs::remove_dir_all(&root).ok();
    }

    #[cfg(all(
        feature = "tool-file",
        feature = "tool-shell",
        feature = "memory-sqlite"
    ))]
    #[test]
    fn capability_snapshot_only_lists_core_discovery_tools() {
        let snapshot = capability_snapshot();
        assert!(snapshot.contains("- tool.search: Discover non-core tools"));
        assert!(snapshot.contains("- tool.invoke: Invoke a discovered non-core tool"));
        assert!(snapshot.contains("Non-core tools are intentionally hidden"));
        assert!(!snapshot.contains("claw.import"));
        assert!(!snapshot.contains("external_skills.fetch"));
        assert!(!snapshot.contains("file.read"));
        assert!(!snapshot.contains("shell.exec"));

        let lines: Vec<&str> = snapshot.lines().skip(1).collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("- tool.invoke"));
        assert!(lines[1].starts_with("- tool.search"));
    }

    #[cfg(all(
        feature = "tool-file",
        feature = "tool-shell",
        feature = "memory-sqlite"
    ))]
    #[test]
    fn tool_registry_returns_all_known_tools() {
        let entries = tool_registry();
        assert_eq!(entries.len(), 25);
        let names: Vec<&str> = entries.iter().map(|e| e.name).collect();
        assert!(names.contains(&"approval_request_resolve"));
        assert!(names.contains(&"approval_request_status"));
        assert!(names.contains(&"approval_requests_list"));
        assert!(names.contains(&"claw.import"));
        assert!(names.contains(&"delegate"));
        assert!(names.contains(&"delegate_async"));
        assert!(names.contains(&"external_skills.fetch"));
        assert!(names.contains(&"external_skills.install"));
        assert!(names.contains(&"external_skills.inspect"));
        assert!(names.contains(&"external_skills.invoke"));
        assert!(names.contains(&"external_skills.list"));
        assert!(names.contains(&"external_skills.policy"));
        assert!(names.contains(&"external_skills.remove"));
        assert!(names.contains(&"shell.exec"));
        assert!(names.contains(&"file.read"));
        assert!(names.contains(&"file.write"));
        assert!(names.contains(&"provider.switch"));
        assert!(names.contains(&"session_archive"));
        assert!(names.contains(&"session_cancel"));
        assert!(names.contains(&"session_events"));
        assert!(names.contains(&"session_recover"));
        assert!(names.contains(&"session_status"));
        assert!(names.contains(&"session_wait"));
        assert!(names.contains(&"sessions_history"));
        assert!(names.contains(&"sessions_list"));
        assert!(names.contains(&"sessions_send"));
    }

    #[cfg(all(feature = "tool-file", feature = "tool-shell"))]
    #[test]
    fn capability_snapshot_for_view_stays_core_only_under_restricted_view() {
        let view = ToolView::from_tool_names(["claw.import", "shell.exec"]);
        let snapshot = capability_snapshot_for_view(&view);

        assert!(snapshot.contains("- tool.search: Discover non-core tools"));
        assert!(snapshot.contains("- tool.invoke: Invoke a discovered non-core tool"));
        assert!(!snapshot.contains("- claw.import:"));
        assert!(!snapshot.contains("- shell.exec:"));
    }

    #[cfg(all(feature = "tool-file", feature = "tool-shell"))]
    #[test]
    fn try_provider_tool_definitions_for_view_returns_core_only_subset() {
        let view = ToolView::from_tool_names(["shell.exec", "claw.import"]);
        let defs = try_provider_tool_definitions_for_view(&view)
            .expect("restricted runtime view should still expose provider-core schemas");
        let names: Vec<&str> = defs
            .iter()
            .filter_map(|item| item.get("function"))
            .filter_map(|function| function.get("name"))
            .filter_map(Value::as_str)
            .collect();

        assert_eq!(names, vec!["tool_invoke", "tool_search"]);
    }

    #[cfg(feature = "memory-sqlite")]
    #[test]
    fn runtime_tool_view_includes_runtime_session_tools_but_hides_planned_ones() {
        let view = runtime_tool_view_for_config(&crate::config::ToolConfig::default());

        for tool_name in [
            "approval_request_resolve",
            "approval_request_status",
            "approval_requests_list",
            "delegate",
            "delegate_async",
            "session_archive",
            "session_cancel",
            "session_events",
            "session_recover",
            "session_status",
            "session_wait",
            "sessions_history",
            "sessions_list",
        ] {
            assert!(
                view.contains(tool_name),
                "expected runtime view to include `{tool_name}`"
            );
        }

        let tool_name = "sessions_send";
        assert!(
            !view.contains(tool_name),
            "expected runtime view to keep `{tool_name}` hidden"
        );
    }

    #[test]
    fn runtime_tool_view_exposes_delegate_tools_with_depth_budget_only() {
        let config = crate::config::ToolConfig::default();

        let root_view = runtime_tool_view_for_config(&config);
        assert!(root_view.contains("delegate"));
        assert!(root_view.contains("delegate_async"));

        let child_view = delegate_child_tool_view_for_config(&config);
        assert!(!child_view.contains("delegate"));
        assert!(!child_view.contains("delegate_async"));

        let depth_budgeted_child = delegate_child_tool_view_for_config_with_delegate(&config, true);
        assert!(depth_budgeted_child.contains("delegate"));
        assert!(depth_budgeted_child.contains("delegate_async"));
    }

    #[test]
    fn runtime_tool_view_exposes_sessions_send_only_when_messages_enabled() {
        let default_root_view = runtime_tool_view_for_config(&crate::config::ToolConfig::default());
        assert!(!default_root_view.contains("sessions_send"));

        let mut config = crate::config::ToolConfig::default();
        config.messages.enabled = true;

        let root_view = runtime_tool_view_for_config(&config);
        assert!(root_view.contains("sessions_send"));

        let child_view = delegate_child_tool_view_for_config(&config);
        assert!(!child_view.contains("sessions_send"));
    }

    #[cfg(all(feature = "tool-file", feature = "tool-shell"))]
    #[test]
    fn delegate_child_tool_view_hides_shell_by_default() {
        let view = delegate_child_tool_view_for_config(&crate::config::ToolConfig::default());

        assert!(view.contains("file.read"));
        assert!(view.contains("file.write"));
        assert!(!view.contains("shell.exec"));
    }

    #[cfg(all(feature = "tool-file", feature = "tool-shell"))]
    #[test]
    fn delegate_child_tool_view_can_allow_shell_when_enabled() {
        let mut config = crate::config::ToolConfig::default();
        config.delegate.allow_shell_in_child = true;

        let view = delegate_child_tool_view_for_config(&config);

        assert!(view.contains("file.read"));
        assert!(view.contains("file.write"));
        assert!(view.contains("shell.exec"));
    }

    #[cfg(all(
        feature = "tool-file",
        feature = "tool-shell",
        feature = "memory-sqlite"
    ))]
    #[test]
    fn provider_tool_definitions_are_stable_and_core_only() {
        let defs = provider_tool_definitions();
        assert_eq!(defs.len(), 2);

        let names: Vec<&str> = defs
            .iter()
            .filter_map(|item| item.get("function"))
            .filter_map(|function| function.get("name"))
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(names, vec!["tool_invoke", "tool_search"]);

        for item in &defs {
            assert_eq!(item["type"], "function");
            assert_eq!(item["function"]["parameters"]["type"], "object");
        }

        let tool_search = defs
            .iter()
            .find(|item| {
                item.get("function")
                    .and_then(|function| function.get("name"))
                    .and_then(Value::as_str)
                    == Some("tool_search")
            })
            .expect("tool_search definition should exist");
        assert!(
            tool_search["function"]["parameters"]["required"]
                .as_array()
                .expect("required should be an array")
                .contains(&Value::String("query".to_owned()))
        );
    }

    #[test]
    fn provider_exposed_tool_gate_is_core_only() {
        assert!(is_provider_exposed_tool_name("tool.search"));
        assert!(is_provider_exposed_tool_name("tool.invoke"));
        assert!(!is_provider_exposed_tool_name("file.read"));
        assert!(!is_provider_exposed_tool_name("shell.exec"));
    }

    #[test]
    fn canonical_tool_name_maps_known_aliases() {
        assert_eq!(canonical_tool_name("tool_search"), "tool.search");
        assert_eq!(canonical_tool_name("tool_invoke"), "tool.invoke");
        assert_eq!(canonical_tool_name("claw_import"), "claw.import");
        assert_eq!(
            canonical_tool_name("external_skills_policy"),
            "external_skills.policy"
        );
        assert_eq!(
            canonical_tool_name("external_skills_fetch"),
            "external_skills.fetch"
        );
        assert_eq!(
            canonical_tool_name("external_skills_install"),
            "external_skills.install"
        );
        assert_eq!(
            canonical_tool_name("external_skills_list"),
            "external_skills.list"
        );
        assert_eq!(
            canonical_tool_name("external_skills_inspect"),
            "external_skills.inspect"
        );
        assert_eq!(
            canonical_tool_name("external_skills_invoke"),
            "external_skills.invoke"
        );
        assert_eq!(
            canonical_tool_name("external_skills_remove"),
            "external_skills.remove"
        );
        assert_eq!(canonical_tool_name("file_read"), "file.read");
        assert_eq!(canonical_tool_name("file_write"), "file.write");
        assert_eq!(canonical_tool_name("provider_switch"), "provider.switch");
        assert_eq!(canonical_tool_name("shell_exec"), "shell.exec");
        assert_eq!(canonical_tool_name("shell"), "shell.exec");
        assert_eq!(canonical_tool_name("file.read"), "file.read");
    }

    #[test]
    fn required_capabilities_follow_effective_tool_request() {
        let direct_file_read = ToolCoreRequest {
            tool_name: "file.read".to_owned(),
            payload: json!({"path": "README.md"}),
        };
        assert_eq!(
            required_capabilities_for_request(&direct_file_read),
            BTreeSet::from([Capability::InvokeTool, Capability::FilesystemRead])
        );

        let direct_file_write = ToolCoreRequest {
            tool_name: "file.write".to_owned(),
            payload: json!({"path": "notes.txt", "content": "hello"}),
        };
        assert_eq!(
            required_capabilities_for_request(&direct_file_write),
            BTreeSet::from([Capability::InvokeTool, Capability::FilesystemWrite])
        );

        let invoked_file_read = ToolCoreRequest {
            tool_name: "tool.invoke".to_owned(),
            payload: json!({
                "tool_id": "file.read",
                "lease": "unused",
                "arguments": {"path": "README.md"}
            }),
        };
        assert_eq!(
            required_capabilities_for_request(&invoked_file_read),
            BTreeSet::from([Capability::InvokeTool, Capability::FilesystemRead])
        );

        let invoked_claw_plan = ToolCoreRequest {
            tool_name: "tool.invoke".to_owned(),
            payload: json!({
                "tool_id": "claw.import",
                "lease": "unused",
                "arguments": {"mode": "plan", "input_path": "imports/nanobot"}
            }),
        };
        assert_eq!(
            required_capabilities_for_request(&invoked_claw_plan),
            BTreeSet::from([Capability::InvokeTool, Capability::FilesystemRead])
        );

        let invoked_claw_apply = ToolCoreRequest {
            tool_name: "tool.invoke".to_owned(),
            payload: json!({
                "tool_id": "claw.import",
                "lease": "unused",
                "arguments": {
                    "mode": "apply",
                    "input_path": "imports/nanobot",
                    "output_path": "loongclaw.toml"
                }
            }),
        };
        assert_eq!(
            required_capabilities_for_request(&invoked_claw_apply),
            BTreeSet::from([
                Capability::InvokeTool,
                Capability::FilesystemRead,
                Capability::FilesystemWrite,
            ])
        );

        let malformed_invoke = ToolCoreRequest {
            tool_name: "tool.invoke".to_owned(),
            payload: json!({"lease": "unused"}),
        };
        assert_eq!(
            required_capabilities_for_request(&malformed_invoke),
            BTreeSet::from([Capability::InvokeTool])
        );
    }

    #[cfg(all(feature = "tool-file", feature = "tool-shell"))]
    #[test]
    fn tool_search_returns_discoverable_tools_with_leases() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("loongclaw-tool-search-{nanos}"));
        fs::create_dir_all(&root).expect("create fixture root");
        fs::write(root.join("README.md"), "hello tool search").expect("write fixture");

        let config = test_tool_runtime_config(root.clone());
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "tool.search".to_owned(),
                payload: json!({
                    "query": "read repo file",
                    "limit": 3
                }),
            },
            &config,
        )
        .expect("tool search should succeed");

        assert_eq!(outcome.status, "ok");
        let results = outcome.payload["results"].as_array().expect("results");
        assert!(!results.is_empty());
        assert!(
            results
                .iter()
                .all(|entry| entry["tool_id"] != "tool.search")
        );
        assert!(
            results
                .iter()
                .any(|entry| entry["tool_id"] == "file.read" && entry["lease"].as_str().is_some())
        );

        fs::remove_dir_all(&root).ok();
    }

    #[cfg(all(feature = "tool-file", feature = "tool-shell"))]
    #[test]
    fn tool_search_hides_filesystem_tools_without_filesystem_capabilities() {
        let root = std::env::temp_dir().join(format!(
            "loongclaw-tool-search-cap-filter-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create fixture root");

        let config = test_tool_runtime_config(root.clone());
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "tool.search".to_owned(),
                payload: json!({
                    "query": "read file import config",
                    TOOL_SEARCH_GRANTED_CAPABILITIES_FIELD: serde_json::to_value(BTreeSet::from([Capability::InvokeTool]))
                        .expect("serialize capabilities")
                }),
            },
            &config,
        )
        .expect("tool search should succeed");

        let results = outcome.payload["results"].as_array().expect("results");
        assert!(results.iter().all(|entry| entry["tool_id"] != "file.read"));
        assert!(results.iter().all(|entry| entry["tool_id"] != "file.write"));
        assert!(
            results
                .iter()
                .all(|entry| entry["tool_id"] != "claw.import")
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(feature = "tool-shell")]
    #[test]
    fn tool_search_hides_shell_exec_when_runtime_allowlist_is_empty() {
        let root = std::env::temp_dir().join(format!(
            "loongclaw-tool-search-shell-filter-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create fixture root");

        let config = runtime_config::ToolRuntimeConfig {
            shell_allow: BTreeSet::new(),
            shell_deny: BTreeSet::new(),
            shell_default_mode: crate::tools::shell_policy_ext::ShellPolicyDefault::Deny,
            file_root: Some(root.clone()),
            config_path: None,
            external_skills: runtime_config::ExternalSkillsRuntimePolicy::default(),
        };
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "tool.search".to_owned(),
                payload: json!({"query": "shell command"}),
            },
            &config,
        )
        .expect("tool search should succeed");

        let results = outcome.payload["results"].as_array().expect("results");
        assert!(results.iter().all(|entry| entry["tool_id"] != "shell.exec"));

        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(all(feature = "tool-file", feature = "tool-shell"))]
    #[test]
    fn tool_search_result_includes_compact_argument_hints() {
        let root = std::env::temp_dir().join(format!(
            "loongclaw-tool-search-hints-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create fixture root");

        let config = test_tool_runtime_config(root.clone());
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "tool.search".to_owned(),
                payload: json!({"query": "shell command"}),
            },
            &config,
        )
        .expect("tool search should succeed");

        let results = outcome.payload["results"].as_array().expect("results");
        assert!(results.iter().any(|entry| {
            entry["tool_id"] == "shell.exec"
                && entry["argument_hint"].as_str() == Some("command:string,args?:string[]")
        }));

        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(feature = "tool-file")]
    #[test]
    fn tool_invoke_dispatches_a_discovered_tool_with_a_valid_lease() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("loongclaw-tool-invoke-{nanos}"));
        fs::create_dir_all(&root).expect("create fixture root");
        fs::write(root.join("README.md"), "tool invoke fixture").expect("write fixture");

        let config = test_tool_runtime_config(root.clone());
        let search = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "tool.search".to_owned(),
                payload: json!({"query": "read file"}),
            },
            &config,
        )
        .expect("tool search should succeed");

        let result = search.payload["results"]
            .as_array()
            .expect("results")
            .iter()
            .find(|entry| entry["tool_id"] == "file.read")
            .expect("file.read search result");

        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "tool.invoke".to_owned(),
                payload: json!({
                    "tool_id": "file.read",
                    "lease": result["lease"].clone(),
                    "arguments": {
                        "path": "README.md",
                        "max_bytes": 64
                    }
                }),
            },
            &config,
        )
        .expect("tool invoke should succeed");

        assert_eq!(outcome.status, "ok");
        assert!(
            outcome.payload["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("README.md"))
        );
        assert_eq!(outcome.payload["content"], "tool invoke fixture");

        fs::remove_dir_all(&root).ok();
    }

    #[cfg(feature = "tool-file")]
    #[test]
    fn tool_invoke_rejects_tampered_or_missing_leases() {
        let root = std::env::temp_dir().join(format!(
            "loongclaw-tool-invoke-invalid-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create fixture root");

        let config = test_tool_runtime_config(root.clone());
        let error = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "tool.invoke".to_owned(),
                payload: json!({
                    "tool_id": "file.read",
                    "lease": "tampered",
                    "arguments": {
                        "path": "README.md"
                    }
                }),
            },
            &config,
        )
        .expect_err("tampered lease should fail");

        assert!(error.contains("invalid_tool_lease"), "error: {error}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(feature = "tool-file")]
    #[test]
    fn tool_invoke_rejects_leases_replayed_in_another_turn() {
        let root = std::env::temp_dir().join(format!(
            "loongclaw-tool-invoke-replay-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create fixture root");

        let config = test_tool_runtime_config(root.clone());
        let search = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "tool.search".to_owned(),
                payload: json!({
                    "query": "read file",
                    TOOL_LEASE_SESSION_ID_FIELD: "session-a",
                    TOOL_LEASE_TURN_ID_FIELD: "turn-a"
                }),
            },
            &config,
        )
        .expect("tool search should succeed");

        let result = search.payload["results"]
            .as_array()
            .expect("results")
            .iter()
            .find(|entry| entry["tool_id"] == "file.read")
            .expect("file.read search result");

        let error = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "tool.invoke".to_owned(),
                payload: json!({
                    "tool_id": "file.read",
                    "lease": result["lease"].clone(),
                    "arguments": {
                        "path": "README.md"
                    },
                    TOOL_LEASE_SESSION_ID_FIELD: "session-a",
                    TOOL_LEASE_TURN_ID_FIELD: "turn-b"
                }),
            },
            &config,
        )
        .expect_err("replayed turn lease should fail");

        assert!(error.contains("turn mismatch"), "error: {error}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn is_known_tool_name_accepts_canonical_and_alias_forms() {
        assert!(is_known_tool_name("claw.import"));
        assert!(is_known_tool_name("claw_import"));
        assert!(is_known_tool_name("external_skills.policy"));
        assert!(is_known_tool_name("external_skills_policy"));
        assert!(is_known_tool_name("external_skills.fetch"));
        assert!(is_known_tool_name("external_skills_fetch"));
        assert!(is_known_tool_name("external_skills.install"));
        assert!(is_known_tool_name("external_skills_install"));
        assert!(is_known_tool_name("external_skills.list"));
        assert!(is_known_tool_name("external_skills_list"));
        assert!(is_known_tool_name("external_skills.inspect"));
        assert!(is_known_tool_name("external_skills_inspect"));
        assert!(is_known_tool_name("external_skills.invoke"));
        assert!(is_known_tool_name("external_skills_invoke"));
        assert!(is_known_tool_name("external_skills.remove"));
        assert!(is_known_tool_name("external_skills_remove"));
        assert!(is_known_tool_name("file.read"));
        assert!(is_known_tool_name("file_read"));
        assert!(is_known_tool_name("file.write"));
        assert!(is_known_tool_name("file_write"));
        assert!(is_known_tool_name("provider.switch"));
        assert!(is_known_tool_name("provider_switch"));
        assert!(is_known_tool_name("shell.exec"));
        assert!(is_known_tool_name("shell_exec"));
        assert!(is_known_tool_name("shell"));
        assert!(!is_known_tool_name("nonexistent.tool"));
    }

    #[test]
    fn provider_switch_tool_updates_target_config_and_reports_active_profile() {
        use std::{
            fs,
            path::PathBuf,
            time::{SystemTime, UNIX_EPOCH},
        };

        fn unique_temp_dir(prefix: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("{prefix}-{nanos}"))
        }

        let root = unique_temp_dir("loongclaw-tool-provider-switch");
        fs::create_dir_all(&root).expect("create fixture root");
        let config_path = root.join("loongclaw.toml");

        let mut config = crate::config::LoongClawConfig::default();
        let mut openai =
            crate::config::ProviderConfig::fresh_for_kind(crate::config::ProviderKind::Openai);
        openai.model = "gpt-5".to_owned();
        config.set_active_provider_profile(
            "openai-gpt-5",
            crate::config::ProviderProfileConfig {
                default_for_kind: true,
                provider: openai.clone(),
            },
        );
        let mut deepseek =
            crate::config::ProviderConfig::fresh_for_kind(crate::config::ProviderKind::Deepseek);
        deepseek.model = "deepseek-chat".to_owned();
        config.providers.insert(
            "deepseek-chat".to_owned(),
            crate::config::ProviderProfileConfig {
                default_for_kind: true,
                provider: deepseek,
            },
        );
        config.provider = openai;
        config.active_provider = Some("openai-gpt-5".to_owned());
        fs::write(
            &config_path,
            crate::config::render(&config).expect("render provider config"),
        )
        .expect("write provider config");

        let runtime_config = runtime_config::ToolRuntimeConfig {
            shell_allow: BTreeSet::new(),
            file_root: Some(root.clone()),
            config_path: Some(config_path.clone()),
            external_skills: Default::default(),
            ..runtime_config::ToolRuntimeConfig::default()
        };
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "provider.switch".to_owned(),
                payload: json!({
                    "selector": "deepseek",
                    "config_path": "loongclaw.toml"
                }),
            },
            &runtime_config,
        )
        .expect("provider switch should succeed");

        assert_eq!(outcome.status, "ok");
        assert_eq!(outcome.payload["tool_name"], "provider.switch");
        assert_eq!(outcome.payload["changed"], true);
        assert_eq!(outcome.payload["previous_active_provider"], "openai-gpt-5");
        assert_eq!(outcome.payload["active_provider"], "deepseek-chat");

        let (_, reloaded) =
            crate::config::load(Some(config_path.to_str().expect("utf8 config path")))
                .expect("load");
        assert_eq!(reloaded.active_provider_id(), Some("deepseek-chat"));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn provider_switch_tool_accepts_unique_model_selector() {
        use std::{
            fs,
            path::PathBuf,
            time::{SystemTime, UNIX_EPOCH},
        };

        fn unique_temp_dir(prefix: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("{prefix}-{nanos}"))
        }

        let root = unique_temp_dir("loongclaw-tool-provider-switch-model");
        fs::create_dir_all(&root).expect("create fixture root");
        let config_path = root.join("loongclaw.toml");

        let mut config = crate::config::LoongClawConfig::default();
        let mut openai =
            crate::config::ProviderConfig::fresh_for_kind(crate::config::ProviderKind::Openai);
        openai.model = "gpt-5".to_owned();
        config.set_active_provider_profile(
            "openai-main",
            crate::config::ProviderProfileConfig {
                default_for_kind: true,
                provider: openai.clone(),
            },
        );
        let mut deepseek =
            crate::config::ProviderConfig::fresh_for_kind(crate::config::ProviderKind::Deepseek);
        deepseek.model = "deepseek-chat".to_owned();
        config.providers.insert(
            "deepseek-cn".to_owned(),
            crate::config::ProviderProfileConfig {
                default_for_kind: true,
                provider: deepseek,
            },
        );
        config.provider = openai;
        config.active_provider = Some("openai-main".to_owned());
        fs::write(
            &config_path,
            crate::config::render(&config).expect("render provider config"),
        )
        .expect("write provider config");

        let runtime_config = runtime_config::ToolRuntimeConfig {
            shell_allow: BTreeSet::new(),
            file_root: Some(root.clone()),
            config_path: Some(config_path.clone()),
            external_skills: Default::default(),
            ..runtime_config::ToolRuntimeConfig::default()
        };
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "provider.switch".to_owned(),
                payload: json!({
                    "selector": "deepseek-chat"
                }),
            },
            &runtime_config,
        )
        .expect("provider switch by model should succeed");

        assert_eq!(outcome.status, "ok");
        assert_eq!(outcome.payload["changed"], true);
        assert_eq!(outcome.payload["previous_active_provider"], "openai-main");
        assert_eq!(outcome.payload["active_provider"], "deepseek-cn");

        let (_, reloaded) =
            crate::config::load(Some(config_path.to_str().expect("utf8 config path")))
                .expect("load");
        assert_eq!(reloaded.active_provider_id(), Some("deepseek-cn"));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn provider_switch_without_selector_reports_current_provider_state() {
        use std::{
            fs,
            path::PathBuf,
            time::{SystemTime, UNIX_EPOCH},
        };

        fn unique_temp_dir(prefix: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("{prefix}-{nanos}"))
        }

        let root = unique_temp_dir("loongclaw-tool-provider-switch-inspect");
        fs::create_dir_all(&root).expect("create fixture root");
        let config_path = root.join("loongclaw.toml");

        let mut config = crate::config::LoongClawConfig::default();
        let mut openai =
            crate::config::ProviderConfig::fresh_for_kind(crate::config::ProviderKind::Openai);
        openai.model = "gpt-5".to_owned();
        config.set_active_provider_profile(
            "openai-gpt-5",
            crate::config::ProviderProfileConfig {
                default_for_kind: true,
                provider: openai,
            },
        );
        fs::write(
            &config_path,
            crate::config::render(&config).expect("render provider config"),
        )
        .expect("write provider config");

        let runtime_config = runtime_config::ToolRuntimeConfig {
            shell_allow: BTreeSet::new(),
            file_root: Some(root.clone()),
            config_path: Some(config_path.clone()),
            external_skills: Default::default(),
            ..runtime_config::ToolRuntimeConfig::default()
        };
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "provider.switch".to_owned(),
                payload: json!({}),
            },
            &runtime_config,
        )
        .expect("provider switch inspect should succeed");

        assert_eq!(outcome.status, "ok");
        assert_eq!(outcome.payload["changed"], false);
        assert_eq!(outcome.payload["active_provider"], "openai-gpt-5");
        assert_eq!(outcome.payload["selector"], Value::Null);
        assert_eq!(outcome.payload["profiles"][0]["profile_id"], "openai-gpt-5");
        assert_eq!(
            outcome.payload["profiles"][0]["accepted_selectors"],
            json!(["openai-gpt-5", "gpt-5", "openai"])
        );

        let (_, reloaded) =
            crate::config::load(Some(config_path.to_str().expect("utf8 config path")))
                .expect("load");
        assert_eq!(reloaded.active_provider_id(), Some("openai-gpt-5"));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn unknown_tool_returns_hard_error_code() {
        let err = execute_tool_core(ToolCoreRequest {
            tool_name: "unknown".to_owned(),
            payload: json!({"hello":"world"}),
        })
        .expect_err("unknown tool should return an error");
        assert!(
            err.contains("tool_not_found"),
            "error should contain tool_not_found, got: {err}"
        );
    }

    #[test]
    fn claw_import_plan_mode_returns_nativeized_preview() {
        use std::{
            fs,
            path::{Path, PathBuf},
            time::{SystemTime, UNIX_EPOCH},
        };

        fn unique_temp_dir(prefix: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("{prefix}-{nanos}"))
        }

        fn write_file(root: &Path, relative: &str, content: &str) {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(path, content).expect("write fixture");
        }

        let root = unique_temp_dir("loongclaw-tool-import-plan");
        fs::create_dir_all(&root).expect("create fixture root");
        write_file(
            &root,
            "SOUL.md",
            "# Soul\n\nAlways prefer concise shell output. updated by nanobot.\n",
        );
        write_file(
            &root,
            "IDENTITY.md",
            "# Identity\n\n- Motto: your nanobot agent for deploys\n",
        );

        let config = runtime_config::ToolRuntimeConfig {
            file_root: Some(root.clone()),
            ..runtime_config::ToolRuntimeConfig::default()
        };
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "claw.import".to_owned(),
                payload: json!({
                    "mode": "plan",
                    "source": "nanobot",
                    "input_path": "."
                }),
            },
            &config,
        )
        .expect("claw import plan should succeed");

        assert_eq!(outcome.status, "ok");
        assert_eq!(outcome.payload["tool_name"], "claw.import");
        assert_eq!(outcome.payload["mode"], "plan");
        assert_eq!(outcome.payload["source"], "nanobot");
        assert_eq!(
            outcome.payload["config_preview"]["prompt_pack_id"],
            "loongclaw-core-v1"
        );
        assert_eq!(
            outcome.payload["config_preview"]["memory_profile"],
            "profile_plus_window"
        );
        assert!(
            outcome.payload["config_preview"]["system_prompt_addendum"]
                .as_str()
                .expect("prompt addendum should exist")
                .contains("LoongClaw")
        );
        assert!(
            outcome.payload["config_preview"]["profile_note"]
                .as_str()
                .expect("profile note should exist")
                .contains("LoongClaw")
        );
        assert_eq!(outcome.payload["config_written"], false);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn claw_import_apply_mode_writes_target_config() {
        use std::{
            fs,
            path::{Path, PathBuf},
            time::{SystemTime, UNIX_EPOCH},
        };

        fn unique_temp_dir(prefix: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("{prefix}-{nanos}"))
        }

        fn write_file(root: &Path, relative: &str, content: &str) {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(path, content).expect("write fixture");
        }

        let root = unique_temp_dir("loongclaw-tool-import-apply");
        fs::create_dir_all(&root).expect("create fixture root");
        write_file(
            &root,
            "SOUL.md",
            "# Soul\n\nAlways prefer concise shell output. updated by nanobot.\n",
        );
        write_file(
            &root,
            "IDENTITY.md",
            "# Identity\n\n- Motto: your nanobot agent for deploys\n",
        );

        let output_path = root.join("generated").join("loongclaw.toml");
        let config = runtime_config::ToolRuntimeConfig {
            file_root: Some(root.clone()),
            ..runtime_config::ToolRuntimeConfig::default()
        };
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "claw_import".to_owned(),
                payload: json!({
                    "mode": "apply",
                    "source": "nanobot",
                    "input_path": ".",
                    "output_path": "generated/loongclaw.toml",
                    "force": true
                }),
            },
            &config,
        )
        .expect("claw import apply should succeed");

        assert_eq!(outcome.status, "ok");
        assert_eq!(outcome.payload["mode"], "apply");
        assert_eq!(outcome.payload["config_written"], true);
        assert_eq!(
            outcome.payload["next_step"]
                .as_str()
                .expect("next_step should be present")
                .split_whitespace()
                .next(),
            Some("loongclaw")
        );
        assert_eq!(
            outcome.payload["output_path"]
                .as_str()
                .expect("output path should exist"),
            fs::canonicalize(&output_path)
                .expect("output path should canonicalize")
                .display()
                .to_string()
        );

        let raw = fs::read_to_string(&output_path).expect("output config should exist");
        assert!(raw.contains("prompt_pack_id = \"loongclaw-core-v1\""));
        assert!(raw.contains("profile = \"profile_plus_window\""));
        assert!(raw.contains("LoongClaw"));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn claw_import_discover_mode_returns_detected_sources() {
        use std::{
            fs,
            path::{Path, PathBuf},
            time::{SystemTime, UNIX_EPOCH},
        };

        fn unique_temp_dir(prefix: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("{prefix}-{nanos}"))
        }

        fn write_file(root: &Path, relative: &str, content: &str) {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(path, content).expect("write fixture");
        }

        let root = unique_temp_dir("loongclaw-tool-import-discover");
        fs::create_dir_all(&root).expect("create fixture root");

        let openclaw_root = root.join("openclaw-workspace");
        fs::create_dir_all(&openclaw_root).expect("create openclaw root");
        write_file(
            &openclaw_root,
            "SOUL.md",
            "# Soul\n\nPrefer direct answers and keep OpenClaw style concise.\n",
        );
        write_file(
            &openclaw_root,
            "IDENTITY.md",
            "# Identity\n\n- Role: Release copilot\n- Priority: stability first\n",
        );

        let config = runtime_config::ToolRuntimeConfig {
            file_root: Some(root.clone()),
            ..runtime_config::ToolRuntimeConfig::default()
        };
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "claw.import".to_owned(),
                payload: json!({
                    "mode": "discover",
                    "input_path": "."
                }),
            },
            &config,
        )
        .expect("claw import discover should succeed");

        assert_eq!(outcome.status, "ok");
        assert_eq!(outcome.payload["mode"], "discover");
        assert_eq!(outcome.payload["sources"][0]["source_id"], "openclaw");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn claw_import_plan_many_mode_returns_source_summaries_and_recommendation() {
        use std::{
            fs,
            path::{Path, PathBuf},
            time::{SystemTime, UNIX_EPOCH},
        };

        fn unique_temp_dir(prefix: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("{prefix}-{nanos}"))
        }

        fn write_file(root: &Path, relative: &str, content: &str) {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(path, content).expect("write fixture");
        }

        let root = unique_temp_dir("loongclaw-tool-import-plan-many");
        fs::create_dir_all(&root).expect("create fixture root");

        let openclaw_root = root.join("openclaw-workspace");
        fs::create_dir_all(&openclaw_root).expect("create openclaw root");
        write_file(
            &openclaw_root,
            "SOUL.md",
            "# Soul\n\nPrefer direct answers and keep OpenClaw style concise.\n",
        );
        write_file(
            &openclaw_root,
            "IDENTITY.md",
            "# Identity\n\n- Role: Release copilot\n- Priority: stability first\n",
        );

        let nanobot_root = root.join("nanobot");
        fs::create_dir_all(&nanobot_root).expect("create nanobot root");
        write_file(
            &nanobot_root,
            "IDENTITY.md",
            "# Identity\n\n- Motto: your nanobot agent for deploys\n",
        );

        let config = runtime_config::ToolRuntimeConfig {
            file_root: Some(root.clone()),
            ..runtime_config::ToolRuntimeConfig::default()
        };
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "claw.import".to_owned(),
                payload: json!({
                    "mode": "plan_many",
                    "input_path": "."
                }),
            },
            &config,
        )
        .expect("claw import plan_many should succeed");

        assert_eq!(outcome.status, "ok");
        assert_eq!(outcome.payload["mode"], "plan_many");
        assert_eq!(outcome.payload["plans"][0]["source_id"], "openclaw");
        assert_eq!(outcome.payload["recommendation"]["source_id"], "openclaw");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn claw_import_merge_profiles_mode_preserves_prompt_owner() {
        use std::{
            fs,
            path::{Path, PathBuf},
            time::{SystemTime, UNIX_EPOCH},
        };

        fn unique_temp_dir(prefix: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("{prefix}-{nanos}"))
        }

        fn write_file(root: &Path, relative: &str, content: &str) {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(path, content).expect("write fixture");
        }

        let root = unique_temp_dir("loongclaw-tool-import-merge-profiles");
        fs::create_dir_all(&root).expect("create fixture root");

        let openclaw_root = root.join("openclaw-workspace");
        fs::create_dir_all(&openclaw_root).expect("create openclaw root");
        write_file(
            &openclaw_root,
            "SOUL.md",
            "# Soul\n\nPrefer direct answers and keep OpenClaw style concise.\n",
        );
        write_file(
            &openclaw_root,
            "IDENTITY.md",
            "# Identity\n\n- role: release copilot\n- tone: steady\n",
        );

        let nanobot_root = root.join("nanobot");
        fs::create_dir_all(&nanobot_root).expect("create nanobot root");
        write_file(
            &nanobot_root,
            "IDENTITY.md",
            "# Identity\n\n- role: release copilot\n- region: apac\n",
        );

        let config = runtime_config::ToolRuntimeConfig {
            file_root: Some(root.clone()),
            ..runtime_config::ToolRuntimeConfig::default()
        };
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "claw.import".to_owned(),
                payload: json!({
                    "mode": "merge_profiles",
                    "input_path": "."
                }),
            },
            &config,
        )
        .expect("claw import merge_profiles should succeed");

        assert_eq!(outcome.status, "ok");
        assert_eq!(outcome.payload["mode"], "merge_profiles");
        assert_eq!(
            outcome.payload["result"]["prompt_owner_source_id"],
            "openclaw"
        );
        assert!(
            outcome.payload["result"]["merged_profile_note"]
                .as_str()
                .expect("merged profile note should be present")
                .contains("region: apac")
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn claw_import_map_external_skills_mode_returns_mapping_plan() {
        use std::{
            fs,
            path::{Path, PathBuf},
            time::{SystemTime, UNIX_EPOCH},
        };

        fn unique_temp_dir(prefix: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("{prefix}-{nanos}"))
        }

        fn write_file(root: &Path, relative: &str, content: &str) {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(path, content).expect("write fixture");
        }

        let root = unique_temp_dir("loongclaw-tool-import-map-external-skills");
        fs::create_dir_all(&root).expect("create fixture root");
        write_file(&root, "SKILLS.md", "# Skills\n\n- custom/skill-a\n");
        fs::create_dir_all(root.join(".codex/skills")).expect("create codex skills dir");

        let config = runtime_config::ToolRuntimeConfig {
            file_root: Some(root.clone()),
            ..runtime_config::ToolRuntimeConfig::default()
        };
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "claw.import".to_owned(),
                payload: json!({
                    "mode": "map_external_skills",
                    "input_path": "."
                }),
            },
            &config,
        )
        .expect("claw import map_external_skills should succeed");

        assert_eq!(outcome.status, "ok");
        assert_eq!(outcome.payload["mode"], "map_external_skills");
        assert_eq!(outcome.payload["result"]["artifact_count"], 2);
        assert_eq!(
            outcome.payload["result"]["declared_skills"][0],
            "custom/skill-a"
        );
        assert_eq!(
            outcome.payload["result"]["resolved_skills"][0],
            "custom/skill-a"
        );
        assert!(
            outcome.payload["result"]["profile_note_addendum"]
                .as_str()
                .expect("profile note addendum should exist")
                .contains("Imported External Skills Artifacts")
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn claw_import_apply_selected_mode_writes_manifest_and_backup() {
        use std::{
            fs,
            path::{Path, PathBuf},
            time::{SystemTime, UNIX_EPOCH},
        };

        fn unique_temp_dir(prefix: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("{prefix}-{nanos}"))
        }

        fn write_file(root: &Path, relative: &str, content: &str) {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(path, content).expect("write fixture");
        }

        let root = unique_temp_dir("loongclaw-tool-import-apply-selected");
        fs::create_dir_all(&root).expect("create fixture root");

        let openclaw_root = root.join("openclaw-workspace");
        fs::create_dir_all(&openclaw_root).expect("create openclaw root");
        write_file(
            &openclaw_root,
            "SOUL.md",
            "# Soul\n\nPrefer direct answers and keep OpenClaw style concise.\n",
        );
        write_file(
            &openclaw_root,
            "IDENTITY.md",
            "# Identity\n\n- role: release copilot\n- tone: steady\n",
        );

        let output_path = root.join("loongclaw.toml");
        let original_body = crate::config::render(&crate::config::LoongClawConfig::default())
            .expect("render default config");
        fs::write(&output_path, &original_body).expect("write original config");

        let config = runtime_config::ToolRuntimeConfig {
            file_root: Some(root.clone()),
            ..runtime_config::ToolRuntimeConfig::default()
        };
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "claw.import".to_owned(),
                payload: json!({
                    "mode": "apply_selected",
                    "input_path": ".",
                    "output_path": "loongclaw.toml",
                    "source_id": "openclaw"
                }),
            },
            &config,
        )
        .expect("claw import apply_selected should succeed");

        assert_eq!(outcome.status, "ok");
        assert_eq!(outcome.payload["mode"], "apply_selected");
        assert!(
            Path::new(
                outcome.payload["result"]["backup_path"]
                    .as_str()
                    .expect("backup path should be present")
            )
            .exists()
        );
        assert!(
            Path::new(
                outcome.payload["result"]["manifest_path"]
                    .as_str()
                    .expect("manifest path should be present")
            )
            .exists()
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn claw_import_apply_selected_mode_can_apply_external_skills_plan() {
        use std::{
            fs,
            path::{Path, PathBuf},
            time::{SystemTime, UNIX_EPOCH},
        };

        fn unique_temp_dir(prefix: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("{prefix}-{nanos}"))
        }

        fn write_file(root: &Path, relative: &str, content: &str) {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(path, content).expect("write fixture");
        }

        let root = unique_temp_dir("loongclaw-tool-import-apply-selected-external");
        fs::create_dir_all(&root).expect("create fixture root");

        let openclaw_root = root.join("openclaw-workspace");
        fs::create_dir_all(&openclaw_root).expect("create openclaw root");
        write_file(
            &openclaw_root,
            "SOUL.md",
            "# Soul\n\nPrefer direct answers and keep OpenClaw style concise.\n",
        );
        write_file(
            &openclaw_root,
            "IDENTITY.md",
            "# Identity\n\n- role: release copilot\n- tone: steady\n",
        );
        write_file(&root, "SKILLS.md", "# Skills\n\n- custom/skill-a\n");

        let output_path = root.join("loongclaw.toml");

        let config = runtime_config::ToolRuntimeConfig {
            file_root: Some(root.clone()),
            ..runtime_config::ToolRuntimeConfig::default()
        };
        let outcome = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "claw.import".to_owned(),
                payload: json!({
                    "mode": "apply_selected",
                    "input_path": ".",
                    "output_path": "loongclaw.toml",
                    "source_id": "openclaw",
                    "apply_external_skills_plan": true
                }),
            },
            &config,
        )
        .expect("claw import apply_selected with external skills should succeed");

        assert_eq!(outcome.status, "ok");
        assert_eq!(
            outcome.payload["result"]["external_skill_artifact_count"],
            1
        );
        assert_eq!(
            outcome.payload["result"]["external_skill_entries_applied"],
            3
        );
        assert!(
            outcome.payload["result"]["external_skills_manifest_path"]
                .as_str()
                .is_some(),
            "external skills manifest path should exist"
        );
        let raw = fs::read_to_string(&output_path).expect("read output config");
        assert!(raw.contains("Imported External Skills Artifacts"));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn claw_import_rollback_last_apply_restores_original_config() {
        use std::{
            fs,
            path::{Path, PathBuf},
            time::{SystemTime, UNIX_EPOCH},
        };

        fn unique_temp_dir(prefix: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            std::env::temp_dir().join(format!("{prefix}-{nanos}"))
        }

        fn write_file(root: &Path, relative: &str, content: &str) {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(path, content).expect("write fixture");
        }

        let root = unique_temp_dir("loongclaw-tool-import-rollback-selected");
        fs::create_dir_all(&root).expect("create fixture root");

        let openclaw_root = root.join("openclaw-workspace");
        fs::create_dir_all(&openclaw_root).expect("create openclaw root");
        write_file(
            &openclaw_root,
            "SOUL.md",
            "# Soul\n\nPrefer direct answers and keep OpenClaw style concise.\n",
        );
        write_file(
            &openclaw_root,
            "IDENTITY.md",
            "# Identity\n\n- role: release copilot\n- tone: steady\n",
        );

        let output_path = root.join("loongclaw.toml");
        let original_body = crate::config::render(&crate::config::LoongClawConfig::default())
            .expect("render default config");
        fs::write(&output_path, &original_body).expect("write original config");

        let config = runtime_config::ToolRuntimeConfig {
            file_root: Some(root.clone()),
            ..runtime_config::ToolRuntimeConfig::default()
        };
        execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "claw.import".to_owned(),
                payload: json!({
                    "mode": "apply_selected",
                    "input_path": ".",
                    "output_path": "loongclaw.toml",
                    "source_id": "openclaw"
                }),
            },
            &config,
        )
        .expect("claw import apply_selected should succeed");

        let rollback = execute_tool_core_with_config(
            ToolCoreRequest {
                tool_name: "claw.import".to_owned(),
                payload: json!({
                    "mode": "rollback_last_apply",
                    "output_path": "loongclaw.toml"
                }),
            },
            &config,
        )
        .expect("claw import rollback_last_apply should succeed");

        assert_eq!(rollback.status, "ok");
        assert!(
            rollback.payload["rolled_back"]
                .as_bool()
                .expect("rolled_back flag should exist")
        );
        assert_eq!(
            fs::read_to_string(&output_path).expect("read restored config"),
            original_body
        );

        fs::remove_dir_all(&root).ok();
    }

    // --- Kernel-routed tool tests ---

    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use loongclaw_contracts::{ExecutionRoute, HarnessKind, ToolPlaneError};
    use loongclaw_kernel::{
        CoreToolAdapter, FixedClock, InMemoryAuditSink, LoongClawKernel, StaticPolicyEngine,
        VerticalPackManifest,
    };

    struct SharedTestToolAdapter {
        invocations: Arc<Mutex<Vec<ToolCoreRequest>>>,
    }

    #[async_trait]
    impl CoreToolAdapter for SharedTestToolAdapter {
        fn name(&self) -> &str {
            "test-tool-shared"
        }

        async fn execute_core_tool(
            &self,
            request: ToolCoreRequest,
        ) -> Result<ToolCoreOutcome, ToolPlaneError> {
            self.invocations
                .lock()
                .expect("invocations lock")
                .push(request);
            Ok(ToolCoreOutcome {
                status: "ok".to_owned(),
                payload: json!({}),
            })
        }
    }

    fn build_tool_kernel_context(
        audit: Arc<InMemoryAuditSink>,
        capabilities: BTreeSet<Capability>,
    ) -> (KernelContext, Arc<Mutex<Vec<ToolCoreRequest>>>) {
        let clock = Arc::new(FixedClock::new(1_700_000_000));
        let mut kernel = LoongClawKernel::with_runtime(StaticPolicyEngine::default(), clock, audit);

        let pack = VerticalPackManifest {
            pack_id: "test-pack".to_owned(),
            domain: "testing".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: None,
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: capabilities,
            metadata: BTreeMap::new(),
        };
        kernel.register_pack(pack).expect("register pack");

        let invocations = Arc::new(Mutex::new(Vec::new()));
        let adapter = SharedTestToolAdapter {
            invocations: invocations.clone(),
        };
        kernel.register_core_tool_adapter(adapter);
        kernel
            .set_default_core_tool_adapter("test-tool-shared")
            .expect("set default tool adapter");

        let token = kernel
            .issue_token("test-pack", "test-agent", 3600)
            .expect("issue token");

        let ctx = KernelContext {
            kernel: Arc::new(kernel),
            token,
        };

        (ctx, invocations)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tool_call_through_kernel_records_audit() {
        let audit = Arc::new(InMemoryAuditSink::default());
        let (ctx, invocations) =
            build_tool_kernel_context(audit.clone(), BTreeSet::from([Capability::InvokeTool]));

        let request = ToolCoreRequest {
            tool_name: "echo".to_owned(),
            payload: json!({"msg": "hello"}),
        };
        let outcome = execute_tool(request, &ctx)
            .await
            .expect("tool call via kernel should succeed");
        assert_eq!(outcome.status, "ok");

        // Verify the tool adapter received the request.
        let captured = invocations.lock().expect("invocations lock");
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].tool_name, "echo");

        // Verify audit events contain a tool plane invocation.
        let events = audit.snapshot();
        let has_tool_plane = events.iter().any(|event| {
            matches!(
                &event.kind,
                loongclaw_kernel::AuditEventKind::PlaneInvoked {
                    plane: loongclaw_contracts::ExecutionPlane::Tool,
                    ..
                }
            )
        });
        assert!(has_tool_plane, "audit should contain tool plane invocation");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mvp_tool_adapter_routes_through_kernel() {
        use kernel_adapter::MvpToolAdapter;

        let audit = Arc::new(InMemoryAuditSink::default());
        let clock = Arc::new(FixedClock::new(1_700_000_000));
        let mut kernel =
            LoongClawKernel::with_runtime(StaticPolicyEngine::default(), clock, audit.clone());

        let pack = VerticalPackManifest {
            pack_id: "test-pack".to_owned(),
            domain: "testing".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: None,
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: BTreeSet::from([Capability::InvokeTool]),
            metadata: BTreeMap::new(),
        };
        kernel.register_pack(pack).expect("register pack");
        kernel.register_core_tool_adapter(MvpToolAdapter::new());
        kernel
            .set_default_core_tool_adapter("mvp-tools")
            .expect("set default");

        let token = kernel
            .issue_token("test-pack", "test-agent", 3600)
            .expect("issue token");

        let caps = BTreeSet::from([Capability::InvokeTool]);
        // Use an unknown tool name — it should propagate as an error through the adapter
        let request = ToolCoreRequest {
            tool_name: "noop".to_owned(),
            payload: json!({"key": "value"}),
        };
        let err = kernel
            .execute_tool_core("test-pack", &token, &caps, None, request)
            .await
            .expect_err("unknown tool via MvpToolAdapter should fail");
        assert!(
            format!("{err}").contains("tool_not_found"),
            "error should contain tool_not_found, got: {err}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tool_call_through_kernel_denied_without_capability() {
        let audit = Arc::new(InMemoryAuditSink::default());
        // Grant MemoryRead only — InvokeTool is missing.
        let (ctx, _invocations) =
            build_tool_kernel_context(audit, BTreeSet::from([Capability::MemoryRead]));

        let request = ToolCoreRequest {
            tool_name: "echo".to_owned(),
            payload: json!({"msg": "hello"}),
        };
        let err = execute_tool(request, &ctx)
            .await
            .expect_err("should be denied without InvokeTool capability");

        // The error message should indicate a policy/capability denial.
        assert!(
            err.contains("denied") || err.contains("capability") || err.contains("Capability"),
            "error should mention denial or capability, got: {err}"
        );
    }
}
