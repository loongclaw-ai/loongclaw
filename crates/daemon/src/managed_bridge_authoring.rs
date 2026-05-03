use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::{CliResult, kernel, mvp};

pub(crate) const DEFAULT_CHANNEL_BRIDGE_ACCOUNT_SCOPE: &str = "multi_account";
pub(crate) const PROCESS_STDIO_MANAGED_BRIDGE_OPERATIONS: &[&str] = &[
    mvp::channel::CHANNEL_PLUGIN_BRIDGE_RUNTIME_SEND_MESSAGE_OPERATION,
    mvp::channel::CHANNEL_PLUGIN_BRIDGE_RUNTIME_RECEIVE_BATCH_OPERATION,
    mvp::channel::CHANNEL_PLUGIN_BRIDGE_RUNTIME_ACK_INBOUND_OPERATION,
    mvp::channel::CHANNEL_PLUGIN_BRIDGE_RUNTIME_COMPLETE_BATCH_OPERATION,
];

pub(crate) const CHANNEL_BRIDGE_JAVASCRIPT_REFERENCE_EXAMPLE_ROOT: &str =
    "examples/plugins-process/channel-bridge-javascript";

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProcessStdioManagedBridgeLanguageProfile {
    pub source_language: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub process_timeout_ms: u64,
    pub smoke_allow_command: &'static str,
    pub scaffold_files: &'static [crate::native_extension_authoring::RuntimeScaffoldTemplateFile],
}

const JAVASCRIPT_MANAGED_BRIDGE_SCAFFOLD_FILES:
    &[crate::native_extension_authoring::RuntimeScaffoldTemplateFile] = &[
    crate::native_extension_authoring::RuntimeScaffoldTemplateFile {
        relative_path: "bridge.js",
        contents: JAVASCRIPT_MANAGED_BRIDGE_STUB,
    },
];

const JAVASCRIPT_MANAGED_BRIDGE_ARGS: &[&str] = &["bridge.js"];

const SUPPORTED_PROCESS_STDIO_MANAGED_BRIDGE_PROFILES:
    &[ProcessStdioManagedBridgeLanguageProfile] = &[ProcessStdioManagedBridgeLanguageProfile {
    source_language: "javascript",
    command: "node",
    args: JAVASCRIPT_MANAGED_BRIDGE_ARGS,
    process_timeout_ms: 15_000,
    smoke_allow_command: "node",
    scaffold_files: JAVASCRIPT_MANAGED_BRIDGE_SCAFFOLD_FILES,
}];

pub(crate) fn process_stdio_managed_bridge_language_profile(
    scaffold_defaults: &kernel::PluginRuntimeScaffoldDefaults,
) -> CliResult<Option<ProcessStdioManagedBridgeLanguageProfile>> {
    if scaffold_defaults.bridge_kind != kernel::PluginBridgeKind::ProcessStdio {
        return Ok(None);
    }

    let Some(source_language) = scaffold_defaults.source_language.as_deref() else {
        return Ok(None);
    };
    if let Some(profile) =
        process_stdio_managed_bridge_language_profile_for_source_language(source_language)
    {
        return Ok(Some(profile));
    }

    Err(format!(
        "plugins init currently scaffolds managed bridge process_stdio entrypoints only for source_language `javascript`; got `{source_language}`"
    ))
}

pub(crate) fn process_stdio_managed_bridge_language_profile_for_source_language(
    source_language: &str,
) -> Option<ProcessStdioManagedBridgeLanguageProfile> {
    SUPPORTED_PROCESS_STDIO_MANAGED_BRIDGE_PROFILES
        .iter()
        .find(|profile| profile.source_language == source_language)
        .copied()
}

pub(crate) fn process_stdio_managed_bridge_scaffold_args(
    profile: ProcessStdioManagedBridgeLanguageProfile,
) -> Vec<String> {
    profile
        .args
        .iter()
        .map(|value| (*value).to_owned())
        .collect()
}

pub(crate) fn channel_bridge_reference_example_package_root(
    source_language: &str,
) -> Option<&'static str> {
    match source_language {
        "javascript" => Some(CHANNEL_BRIDGE_JAVASCRIPT_REFERENCE_EXAMPLE_ROOT),
        _ => None,
    }
}

pub(crate) fn channel_bridge_required_env_var(channel_id: &str) -> String {
    let normalized = channel_id
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() {
                value.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("{normalized}_BRIDGE_URL")
}

pub(crate) fn default_channel_bridge_setup(channel_id: &str) -> kernel::PluginSetup {
    kernel::PluginSetup {
        surface: Some("channel".to_owned()),
        required_env_vars: vec![channel_bridge_required_env_var(channel_id)],
        ..Default::default()
    }
}

pub(crate) fn render_channel_bridge_operation_specs_json(operations: &[String]) -> Option<String> {
    if operations.is_empty() {
        return None;
    }

    let specs = operations
        .iter()
        .filter_map(|operation| {
            let (label, summary, sample_payload) = channel_bridge_operation_metadata(operation)?;
            Some((
                operation.clone(),
                json!({
                    "label": label,
                    "summary": summary,
                    "sample_payload": sample_payload,
                    "operator_hint": format!(
                        "Probe this operation with `loong plugins invoke-channel-bridge-operation --root \"<package-root>\" --plugin-id \"<plugin-id>\" --operation {operation} --payload '{payload}' --allow-command <allow-command>`.",
                        payload = sample_payload.to_string()
                    ),
                }),
            ))
        })
        .collect::<BTreeMap<_, _>>();

    if specs.is_empty() {
        return None;
    }

    serde_json::to_string(&specs).ok()
}

pub(crate) fn render_authoring_channel_bridge_probe_command(
    package_root: &str,
    plugin_id: &str,
    operation: &str,
    allow_command: &str,
) -> String {
    let payload = default_channel_bridge_probe_payload(operation).to_string();
    format!(
        "loong plugins invoke-channel-bridge-operation --root \"{package_root}\" --plugin-id \"{plugin_id}\" --operation {operation} --payload '{payload}' --allow-command {allow_command}"
    )
}

pub(crate) fn default_channel_bridge_operations() -> Vec<String> {
    PROCESS_STDIO_MANAGED_BRIDGE_OPERATIONS
        .iter()
        .map(|value| (*value).to_owned())
        .collect()
}

fn default_channel_bridge_probe_payload(operation: &str) -> Value {
    channel_bridge_operation_metadata(operation)
        .map(|(_, _, payload)| payload)
        .unwrap_or_else(|| json!({}))
}

fn channel_bridge_operation_metadata(operation: &str) -> Option<(String, String, Value)> {
    match operation {
        mvp::channel::CHANNEL_PLUGIN_BRIDGE_RUNTIME_SEND_MESSAGE_OPERATION => Some((
            "Send Message".to_owned(),
            "Send one outbound bridge message for the selected channel target.".to_owned(),
            json!({
                "target": "weixin:contact:demo",
                "text": "hello",
            }),
        )),
        mvp::channel::CHANNEL_PLUGIN_BRIDGE_RUNTIME_RECEIVE_BATCH_OPERATION => Some((
            "Receive Batch".to_owned(),
            "Poll one inbound bridge batch for channel delivery.".to_owned(),
            json!({
                "limit": 10,
            }),
        )),
        mvp::channel::CHANNEL_PLUGIN_BRIDGE_RUNTIME_ACK_INBOUND_OPERATION => Some((
            "Ack Inbound".to_owned(),
            "Acknowledge one inbound message after processing.".to_owned(),
            json!({
                "message_id": "demo-message",
            }),
        )),
        mvp::channel::CHANNEL_PLUGIN_BRIDGE_RUNTIME_COMPLETE_BATCH_OPERATION => Some((
            "Complete Batch".to_owned(),
            "Mark the current receive batch as fully processed.".to_owned(),
            json!({
                "batch_cursor": "cursor-1",
            }),
        )),
        _ => None,
    }
}

const JAVASCRIPT_MANAGED_BRIDGE_STUB: &str = r#"#!/usr/bin/env node
function buildPayload(operation, payload) {
  switch (operation) {
    case 'send_message':
      return { accepted: true, target: payload.target ?? null };
    case 'receive_batch':
      return { messages: [] };
    case 'ack_inbound':
      return { acknowledged: payload.message_id ?? null };
    case 'complete_batch':
      return { completed: true, batch_cursor: payload.batch_cursor ?? null };
    default:
      return { error: `unsupported operation: ${operation}` };
  }
}
function emitResponse(line) {
  const trimmed = line.trim();
  if (!trimmed) return;
  const request = JSON.parse(trimmed);
  const response = {
    method: request.method ?? '',
    id: request.id ?? null,
    payload: buildPayload(request.payload?.operation ?? '', request.payload?.payload ?? {})
  };
  process.stdout.write(`${JSON.stringify(response)}\n`);
}
process.stdin.setEncoding('utf8');
let buffered = '';
process.stdin.on('data', chunk => {
  buffered += chunk;
  let newlineIndex = buffered.indexOf('\n');
  while (newlineIndex !== -1) {
    const line = buffered.slice(0, newlineIndex);
    buffered = buffered.slice(newlineIndex + 1);
    emitResponse(line);
    newlineIndex = buffered.indexOf('\n');
  }
});
process.stdin.on('end', () => {
  if (buffered.trim()) emitResponse(buffered);
});
process.stdin.resume();
"#;
