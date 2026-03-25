use std::{
    collections::{BTreeMap, BTreeSet},
    env,
};

use serde::{Deserialize, Serialize};

use crate::{
    contracts::Capability,
    plugin::{PluginDescriptor, PluginManifest, PluginScanReport, PluginSetup, PluginSourceKind},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginBridgeKind {
    HttpJson,
    ProcessStdio,
    NativeFfi,
    WasmComponent,
    McpServer,
    AcpBridge,
    AcpRuntime,
    Unknown,
}

impl PluginBridgeKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HttpJson => "http_json",
            Self::ProcessStdio => "process_stdio",
            Self::NativeFfi => "native_ffi",
            Self::WasmComponent => "wasm_component",
            Self::McpServer => "mcp_server",
            Self::AcpBridge => "acp_bridge",
            Self::AcpRuntime => "acp_runtime",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRuntimeProfile {
    pub source_language: String,
    pub bridge_kind: PluginBridgeKind,
    pub adapter_family: String,
    pub entrypoint_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginIR {
    pub plugin_id: String,
    pub provider_id: String,
    pub connector_name: String,
    pub channel_id: Option<String>,
    pub endpoint: Option<String>,
    pub capabilities: BTreeSet<Capability>,
    pub metadata: BTreeMap<String, String>,
    pub source_path: String,
    pub source_kind: PluginSourceKind,
    pub package_root: String,
    pub package_manifest_path: Option<String>,
    pub setup: Option<PluginSetup>,
    pub runtime: PluginRuntimeProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PluginSetupReadinessContext {
    pub available_env_vars: BTreeSet<String>,
    pub configured_keys: BTreeSet<String>,
    pub config_keys_verified: bool,
}

impl PluginSetupReadinessContext {
    #[must_use]
    pub fn from_process_env() -> Self {
        let mut available_env_vars = BTreeSet::new();

        for (raw_name, raw_value) in env::vars_os() {
            let value_is_empty = raw_value.is_empty();
            if value_is_empty {
                continue;
            }

            let env_name = raw_name.to_string_lossy().trim().to_owned();
            let env_name_is_empty = env_name.is_empty();
            if env_name_is_empty {
                continue;
            }

            available_env_vars.insert(env_name);
        }

        Self {
            available_env_vars,
            configured_keys: BTreeSet::new(),
            config_keys_verified: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSetupReadiness {
    pub ready: bool,
    pub reason: String,
    pub missing_required_env_vars: Vec<String>,
    pub missing_required_config_keys: Vec<String>,
}

#[must_use]
pub fn evaluate_plugin_setup_readiness(
    has_declared_setup: bool,
    required_env_vars: &[String],
    required_config_keys: &[String],
    context: &PluginSetupReadinessContext,
) -> PluginSetupReadiness {
    if !has_declared_setup {
        return PluginSetupReadiness {
            ready: true,
            reason: "plugin does not declare setup requirements".to_owned(),
            missing_required_env_vars: Vec::new(),
            missing_required_config_keys: Vec::new(),
        };
    }

    let missing_required_env_vars = collect_missing_required_env_vars(required_env_vars, context);
    let missing_required_config_keys =
        collect_missing_required_config_keys(required_config_keys, context);
    let has_missing_required_env_vars = !missing_required_env_vars.is_empty();
    let has_missing_required_config_keys = !missing_required_config_keys.is_empty();

    if !has_missing_required_env_vars && !has_missing_required_config_keys {
        return PluginSetupReadiness {
            ready: true,
            reason: "declared setup requirements are satisfied".to_owned(),
            missing_required_env_vars,
            missing_required_config_keys,
        };
    }

    let reason = build_plugin_setup_pending_reason(
        &missing_required_env_vars,
        &missing_required_config_keys,
        context.config_keys_verified,
    );

    PluginSetupReadiness {
        ready: false,
        reason,
        missing_required_env_vars,
        missing_required_config_keys,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PluginTranslationReport {
    pub translated_plugins: usize,
    pub bridge_distribution: BTreeMap<String, usize>,
    pub entries: Vec<PluginIR>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginActivationStatus {
    Ready,
    PendingSetup,
    BlockedUnsupportedBridge,
    BlockedUnsupportedAdapterFamily,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginActivationCandidate {
    pub plugin_id: String,
    pub source_path: String,
    pub source_kind: PluginSourceKind,
    pub package_root: String,
    pub package_manifest_path: Option<String>,
    pub bridge_kind: PluginBridgeKind,
    pub adapter_family: String,
    pub status: PluginActivationStatus,
    pub missing_required_env_vars: Vec<String>,
    pub missing_required_config_keys: Vec<String>,
    pub reason: String,
    pub bootstrap_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PluginActivationPlan {
    pub total_plugins: usize,
    pub ready_plugins: usize,
    pub pending_plugins: usize,
    pub blocked_plugins: usize,
    pub candidates: Vec<PluginActivationCandidate>,
}

impl PluginActivationPlan {
    #[must_use]
    pub fn has_blockers(&self) -> bool {
        self.blocked_plugins > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeSupportMatrix {
    pub supported_bridges: BTreeSet<PluginBridgeKind>,
    pub supported_adapter_families: BTreeSet<String>,
}

impl Default for BridgeSupportMatrix {
    fn default() -> Self {
        Self {
            supported_bridges: BTreeSet::from([
                PluginBridgeKind::HttpJson,
                PluginBridgeKind::ProcessStdio,
                PluginBridgeKind::NativeFfi,
                PluginBridgeKind::WasmComponent,
                PluginBridgeKind::McpServer,
                PluginBridgeKind::AcpBridge,
                PluginBridgeKind::AcpRuntime,
            ]),
            supported_adapter_families: BTreeSet::new(),
        }
    }
}

impl BridgeSupportMatrix {
    #[must_use]
    pub fn is_bridge_supported(&self, bridge_kind: PluginBridgeKind) -> bool {
        self.supported_bridges.contains(&bridge_kind)
    }

    #[must_use]
    pub fn is_adapter_family_supported(&self, adapter_family: &str) -> bool {
        self.supported_adapter_families.is_empty()
            || self.supported_adapter_families.contains(adapter_family)
    }
}

#[derive(Debug, Default)]
pub struct PluginTranslator;

impl PluginTranslator {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn translate_scan_report(&self, report: &PluginScanReport) -> PluginTranslationReport {
        let mut translated = PluginTranslationReport::default();

        for descriptor in &report.descriptors {
            let ir = self.translate_descriptor(descriptor);
            let bridge = ir.runtime.bridge_kind.as_str().to_owned();
            *translated.bridge_distribution.entry(bridge).or_insert(0) += 1;
            translated.translated_plugins = translated.translated_plugins.saturating_add(1);
            translated.entries.push(ir);
        }

        translated
    }

    #[must_use]
    pub fn translate_descriptor(&self, descriptor: &PluginDescriptor) -> PluginIR {
        let runtime = infer_runtime_profile(&descriptor.language, &descriptor.manifest);

        PluginIR {
            plugin_id: descriptor.manifest.plugin_id.clone(),
            provider_id: descriptor.manifest.provider_id.clone(),
            connector_name: descriptor.manifest.connector_name.clone(),
            channel_id: descriptor.manifest.channel_id.clone(),
            endpoint: descriptor.manifest.endpoint.clone(),
            capabilities: descriptor.manifest.capabilities.clone(),
            metadata: descriptor.manifest.metadata.clone(),
            source_path: descriptor.path.clone(),
            source_kind: descriptor.source_kind,
            package_root: descriptor.package_root.clone(),
            package_manifest_path: descriptor.package_manifest_path.clone(),
            setup: descriptor.manifest.setup.clone(),
            runtime,
        }
    }

    #[must_use]
    pub fn plan_activation(
        &self,
        translation: &PluginTranslationReport,
        matrix: &BridgeSupportMatrix,
        setup_context: &PluginSetupReadinessContext,
    ) -> PluginActivationPlan {
        let mut plan = PluginActivationPlan::default();

        for ir in &translation.entries {
            plan.total_plugins = plan.total_plugins.saturating_add(1);
            let setup = ir.setup.as_ref();
            let has_declared_setup = setup.is_some();
            let required_env_vars = setup
                .map(|value| value.required_env_vars.clone())
                .unwrap_or_default();
            let required_config_keys = setup
                .map(|value| value.required_config_keys.clone())
                .unwrap_or_default();
            let setup_readiness = evaluate_plugin_setup_readiness(
                has_declared_setup,
                &required_env_vars,
                &required_config_keys,
                setup_context,
            );

            let (status, reason) = if !matrix.is_bridge_supported(ir.runtime.bridge_kind) {
                (
                    PluginActivationStatus::BlockedUnsupportedBridge,
                    format!(
                        "bridge kind {} is not supported by current runtime matrix",
                        ir.runtime.bridge_kind.as_str()
                    ),
                )
            } else if !matrix.is_adapter_family_supported(&ir.runtime.adapter_family) {
                (
                    PluginActivationStatus::BlockedUnsupportedAdapterFamily,
                    format!(
                        "adapter family {} is not supported by current runtime matrix",
                        ir.runtime.adapter_family
                    ),
                )
            } else if !setup_readiness.ready {
                (
                    PluginActivationStatus::PendingSetup,
                    setup_readiness.reason.clone(),
                )
            } else {
                (
                    PluginActivationStatus::Ready,
                    "plugin runtime profile is supported and declared setup requirements are satisfied"
                        .to_owned(),
                )
            };

            match status {
                PluginActivationStatus::Ready => {
                    plan.ready_plugins = plan.ready_plugins.saturating_add(1)
                }
                PluginActivationStatus::PendingSetup => {
                    plan.pending_plugins = plan.pending_plugins.saturating_add(1)
                }
                PluginActivationStatus::BlockedUnsupportedBridge
                | PluginActivationStatus::BlockedUnsupportedAdapterFamily => {
                    plan.blocked_plugins = plan.blocked_plugins.saturating_add(1)
                }
            }

            plan.candidates.push(PluginActivationCandidate {
                plugin_id: ir.plugin_id.clone(),
                source_path: ir.source_path.clone(),
                source_kind: ir.source_kind,
                package_root: ir.package_root.clone(),
                package_manifest_path: ir.package_manifest_path.clone(),
                bridge_kind: ir.runtime.bridge_kind,
                adapter_family: ir.runtime.adapter_family.clone(),
                status,
                missing_required_env_vars: setup_readiness.missing_required_env_vars,
                missing_required_config_keys: setup_readiness.missing_required_config_keys,
                reason,
                bootstrap_hint: bootstrap_hint(ir),
            });
        }

        plan
    }
}

fn collect_missing_required_env_vars(
    required_env_vars: &[String],
    context: &PluginSetupReadinessContext,
) -> Vec<String> {
    let mut missing_required_env_vars = Vec::new();

    for required_env_var in required_env_vars {
        let env_var_is_available = context.available_env_vars.contains(required_env_var);
        if env_var_is_available {
            continue;
        }

        missing_required_env_vars.push(required_env_var.clone());
    }

    missing_required_env_vars
}

fn collect_missing_required_config_keys(
    required_config_keys: &[String],
    context: &PluginSetupReadinessContext,
) -> Vec<String> {
    if !context.config_keys_verified {
        return required_config_keys.to_vec();
    }

    let mut missing_required_config_keys = Vec::new();

    for required_config_key in required_config_keys {
        let config_key_is_available = context.configured_keys.contains(required_config_key);
        if config_key_is_available {
            continue;
        }

        missing_required_config_keys.push(required_config_key.clone());
    }

    missing_required_config_keys
}

fn build_plugin_setup_pending_reason(
    missing_required_env_vars: &[String],
    missing_required_config_keys: &[String],
    config_keys_verified: bool,
) -> String {
    let mut segments = Vec::new();

    let has_missing_required_env_vars = !missing_required_env_vars.is_empty();
    if has_missing_required_env_vars {
        let required_env_vars = missing_required_env_vars.join(", ");
        let env_segment = format!("missing required env var(s): {required_env_vars}");
        segments.push(env_segment);
    }

    let has_missing_required_config_keys = !missing_required_config_keys.is_empty();
    if has_missing_required_config_keys {
        let required_config_keys = missing_required_config_keys.join(", ");
        let config_segment = if config_keys_verified {
            format!("missing required config key(s): {required_config_keys}")
        } else {
            format!("required config key(s) have not been verified: {required_config_keys}")
        };
        segments.push(config_segment);
    }

    let detail = segments.join("; ");
    format!("plugin setup is incomplete ({detail})")
}

fn infer_runtime_profile(language: &str, manifest: &PluginManifest) -> PluginRuntimeProfile {
    let source_language = normalize_language(language);

    let bridge_kind = manifest
        .metadata
        .get("bridge_kind")
        .and_then(|value| parse_bridge_kind(value))
        .or_else(|| {
            manifest
                .metadata
                .get("protocol")
                .filter(|value| value.eq_ignore_ascii_case("mcp"))
                .map(|_| PluginBridgeKind::McpServer)
        })
        .unwrap_or_else(|| default_bridge_kind(&source_language, manifest.endpoint.as_deref()));

    let adapter_family = manifest
        .metadata
        .get("adapter_family")
        .cloned()
        .unwrap_or_else(|| default_adapter_family(&source_language, bridge_kind));

    let entrypoint_hint = manifest
        .metadata
        .get("entrypoint")
        .cloned()
        .or_else(|| default_entrypoint_hint(bridge_kind, manifest.endpoint.as_deref()))
        .unwrap_or_else(|| "invoke".to_owned());

    PluginRuntimeProfile {
        source_language,
        bridge_kind,
        adapter_family,
        entrypoint_hint,
    }
}

fn normalize_language(language: &str) -> String {
    match language.trim().to_ascii_lowercase().as_str() {
        "rs" => "rust".to_owned(),
        "py" => "python".to_owned(),
        "js" => "javascript".to_owned(),
        "ts" => "typescript".to_owned(),
        "go" => "go".to_owned(),
        "wasm" => "wasm".to_owned(),
        "" => "unknown".to_owned(),
        other => other.to_owned(),
    }
}

fn parse_bridge_kind(raw: &str) -> Option<PluginBridgeKind> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "http_json" | "http" => Some(PluginBridgeKind::HttpJson),
        "process_stdio" | "stdio" => Some(PluginBridgeKind::ProcessStdio),
        "native_ffi" | "ffi" => Some(PluginBridgeKind::NativeFfi),
        "wasm_component" | "wasm" => Some(PluginBridgeKind::WasmComponent),
        "mcp_server" | "mcp" => Some(PluginBridgeKind::McpServer),
        "acp_bridge" | "acp" => Some(PluginBridgeKind::AcpBridge),
        "acp_runtime" | "acpx" => Some(PluginBridgeKind::AcpRuntime),
        "unknown" => Some(PluginBridgeKind::Unknown),
        _ => None,
    }
}

fn default_bridge_kind(language: &str, endpoint: Option<&str>) -> PluginBridgeKind {
    match language {
        "rust" | "go" | "c" | "cpp" | "cxx" => PluginBridgeKind::NativeFfi,
        "python" | "javascript" | "typescript" | "java" => PluginBridgeKind::ProcessStdio,
        "wasm" | "wat" => PluginBridgeKind::WasmComponent,
        _ => {
            if let Some(endpoint) = endpoint
                && (endpoint.starts_with("http://") || endpoint.starts_with("https://"))
            {
                return PluginBridgeKind::HttpJson;
            }
            PluginBridgeKind::Unknown
        }
    }
}

fn default_adapter_family(language: &str, bridge_kind: PluginBridgeKind) -> String {
    match bridge_kind {
        PluginBridgeKind::HttpJson => "http-adapter".to_owned(),
        PluginBridgeKind::ProcessStdio => format!("{language}-stdio-adapter"),
        PluginBridgeKind::NativeFfi => format!("{language}-ffi-adapter"),
        PluginBridgeKind::WasmComponent => "wasm-component-adapter".to_owned(),
        PluginBridgeKind::McpServer => "mcp-adapter".to_owned(),
        PluginBridgeKind::AcpBridge => "acp-bridge-adapter".to_owned(),
        PluginBridgeKind::AcpRuntime => "acp-runtime-adapter".to_owned(),
        PluginBridgeKind::Unknown => format!("{language}-unknown-adapter"),
    }
}

fn default_entrypoint_hint(
    bridge_kind: PluginBridgeKind,
    endpoint: Option<&str>,
) -> Option<String> {
    match bridge_kind {
        PluginBridgeKind::HttpJson => {
            Some(endpoint.unwrap_or("https://localhost/invoke").to_owned())
        }
        PluginBridgeKind::ProcessStdio => Some("stdin/stdout::invoke".to_owned()),
        PluginBridgeKind::NativeFfi => Some("lib::invoke".to_owned()),
        PluginBridgeKind::WasmComponent => Some("component::run".to_owned()),
        PluginBridgeKind::McpServer => Some("mcp::stdio".to_owned()),
        PluginBridgeKind::AcpBridge => Some("acp::bridge".to_owned()),
        PluginBridgeKind::AcpRuntime => Some("acp::turn".to_owned()),
        PluginBridgeKind::Unknown => None,
    }
}

fn bootstrap_hint(ir: &PluginIR) -> String {
    match ir.runtime.bridge_kind {
        PluginBridgeKind::HttpJson => format!(
            "register http connector adapter for {} at {}",
            ir.connector_name,
            ir.endpoint.as_deref().unwrap_or("https://localhost/invoke")
        ),
        PluginBridgeKind::ProcessStdio => format!(
            "spawn {} worker and bind stdio bridge {}",
            ir.runtime.source_language, ir.runtime.entrypoint_hint
        ),
        PluginBridgeKind::NativeFfi => format!(
            "load native library adapter {} with symbol {}",
            ir.runtime.adapter_family, ir.runtime.entrypoint_hint
        ),
        PluginBridgeKind::WasmComponent => {
            format!(
                "load wasm component and invoke {}",
                ir.runtime.entrypoint_hint
            )
        }
        PluginBridgeKind::McpServer => {
            "register MCP server bridge and handshake capability schema".to_owned()
        }
        PluginBridgeKind::AcpBridge => {
            "register ACP bridge surface and bind the external gateway/runtime contract".to_owned()
        }
        PluginBridgeKind::AcpRuntime => {
            "register ACP runtime backend and bind a session-aware control plane".to_owned()
        }
        PluginBridgeKind::Unknown => {
            "inspect plugin metadata and define explicit bridge_kind override".to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{PluginManifest, PluginSetup, PluginSetupMode, PluginSourceKind};

    fn descriptor(language: &str, metadata: BTreeMap<String, String>) -> PluginDescriptor {
        let source_kind = if language == "manifest" {
            PluginSourceKind::PackageManifest
        } else {
            PluginSourceKind::EmbeddedSource
        };
        let path = if language == "manifest" {
            "/tmp/loongclaw.plugin.json".to_owned()
        } else {
            format!("/tmp/plugin.{language}")
        };
        let package_manifest_path = if matches!(source_kind, PluginSourceKind::PackageManifest) {
            Some(path.clone())
        } else {
            None
        };

        PluginDescriptor {
            path,
            source_kind,
            package_root: "/tmp".to_owned(),
            package_manifest_path,
            language: language.to_owned(),
            manifest: PluginManifest {
                plugin_id: format!("sample-{language}"),
                provider_id: "sample-provider".to_owned(),
                connector_name: "sample-connector".to_owned(),
                channel_id: Some("primary".to_owned()),
                endpoint: Some("https://example.com/invoke".to_owned()),
                capabilities: BTreeSet::from([Capability::InvokeConnector]),
                metadata,
                summary: None,
                tags: Vec::new(),
                input_examples: Vec::new(),
                output_examples: Vec::new(),
                defer_loading: false,
                setup: Some(PluginSetup {
                    mode: PluginSetupMode::MetadataOnly,
                    surface: Some("web_search".to_owned()),
                    required_env_vars: vec!["TAVILY_API_KEY".to_owned()],
                    recommended_env_vars: vec!["TEAM_TAVILY_KEY".to_owned()],
                    required_config_keys: vec!["tools.web_search.default_provider".to_owned()],
                    default_env_var: Some("TAVILY_API_KEY".to_owned()),
                    docs_urls: vec!["https://docs.example.com/tavily".to_owned()],
                    remediation: Some("set a Tavily credential before enabling search".to_owned()),
                }),
            },
        }
    }

    #[test]
    fn translator_infers_bridge_from_source_language() {
        let scanner_report = PluginScanReport {
            scanned_files: 2,
            matched_plugins: 2,
            descriptors: vec![
                descriptor("rs", BTreeMap::new()),
                descriptor("py", BTreeMap::new()),
            ],
        };

        let translator = PluginTranslator::new();
        let report = translator.translate_scan_report(&scanner_report);

        assert_eq!(report.translated_plugins, 2);
        assert_eq!(
            report.bridge_distribution.get("native_ffi").copied(),
            Some(1)
        );
        assert_eq!(
            report.bridge_distribution.get("process_stdio").copied(),
            Some(1)
        );
    }

    #[test]
    fn translator_honors_metadata_bridge_override() {
        let descriptor = descriptor(
            "js",
            BTreeMap::from([
                ("bridge_kind".to_owned(), "mcp_server".to_owned()),
                ("entrypoint".to_owned(), "custom::run".to_owned()),
            ]),
        );

        let translator = PluginTranslator::new();
        let ir = translator.translate_descriptor(&descriptor);

        assert_eq!(ir.runtime.bridge_kind, PluginBridgeKind::McpServer);
        assert_eq!(ir.runtime.entrypoint_hint, "custom::run");
        assert_eq!(ir.runtime.adapter_family, "mcp-adapter");
    }

    #[test]
    fn translator_defaults_manifest_descriptor_with_endpoint_to_http_json() {
        let descriptor = descriptor("manifest", BTreeMap::new());

        let translator = PluginTranslator::new();
        let ir = translator.translate_descriptor(&descriptor);

        assert_eq!(ir.runtime.source_language, "manifest");
        assert_eq!(ir.runtime.bridge_kind, PluginBridgeKind::HttpJson);
        assert_eq!(ir.runtime.adapter_family, "http-adapter");
        assert_eq!(ir.source_kind, PluginSourceKind::PackageManifest);
        assert_eq!(ir.package_root, "/tmp");
        assert_eq!(
            ir.setup.as_ref().and_then(|setup| setup.surface.as_deref()),
            Some("web_search")
        );
        assert_eq!(
            ir.package_manifest_path,
            Some("/tmp/loongclaw.plugin.json".to_owned())
        );
    }

    #[test]
    fn translator_accepts_acpx_runtime_alias() {
        let descriptor = descriptor(
            "js",
            BTreeMap::from([("bridge_kind".to_owned(), "acpx".to_owned())]),
        );

        let translator = PluginTranslator::new();
        let ir = translator.translate_descriptor(&descriptor);

        assert_eq!(ir.runtime.bridge_kind, PluginBridgeKind::AcpRuntime);
        assert_eq!(ir.runtime.adapter_family, "acp-runtime-adapter");
        assert_eq!(ir.runtime.entrypoint_hint, "acp::turn");
    }

    #[test]
    fn translator_maps_acp_alias_to_bridge_surface() {
        let descriptor = descriptor(
            "js",
            BTreeMap::from([("bridge_kind".to_owned(), "acp".to_owned())]),
        );

        let translator = PluginTranslator::new();
        let ir = translator.translate_descriptor(&descriptor);

        assert_eq!(ir.runtime.bridge_kind, PluginBridgeKind::AcpBridge);
        assert_eq!(ir.runtime.adapter_family, "acp-bridge-adapter");
        assert_eq!(ir.runtime.entrypoint_hint, "acp::bridge");
    }

    #[test]
    fn activation_plan_blocks_unsupported_bridge() {
        let descriptor = descriptor(
            "js",
            BTreeMap::from([("bridge_kind".to_owned(), "mcp_server".to_owned())]),
        );
        let translator = PluginTranslator::new();
        let translation = translator.translate_scan_report(&PluginScanReport {
            scanned_files: 1,
            matched_plugins: 1,
            descriptors: vec![descriptor],
        });

        let matrix = BridgeSupportMatrix {
            supported_bridges: BTreeSet::from([PluginBridgeKind::HttpJson]),
            supported_adapter_families: BTreeSet::new(),
        };
        let setup_context = PluginSetupReadinessContext::default();
        let plan = translator.plan_activation(&translation, &matrix, &setup_context);

        assert_eq!(plan.total_plugins, 1);
        assert_eq!(plan.ready_plugins, 0);
        assert_eq!(plan.pending_plugins, 0);
        assert_eq!(plan.blocked_plugins, 1);
        assert_eq!(
            plan.candidates[0].source_kind,
            PluginSourceKind::EmbeddedSource
        );
        assert_eq!(plan.candidates[0].package_root, "/tmp");
        assert_eq!(plan.candidates[0].package_manifest_path, None);
        assert!(matches!(
            plan.candidates[0].status,
            PluginActivationStatus::BlockedUnsupportedBridge
        ));
    }

    #[test]
    fn activation_plan_blocks_unsupported_adapter_family() {
        let descriptor = descriptor(
            "py",
            BTreeMap::from([(
                "adapter_family".to_owned(),
                "python-stdio-adapter".to_owned(),
            )]),
        );
        let translator = PluginTranslator::new();
        let translation = translator.translate_scan_report(&PluginScanReport {
            scanned_files: 1,
            matched_plugins: 1,
            descriptors: vec![descriptor],
        });

        let matrix = BridgeSupportMatrix {
            supported_bridges: BTreeSet::from([PluginBridgeKind::ProcessStdio]),
            supported_adapter_families: BTreeSet::from(["rust-stdio-adapter".to_owned()]),
        };
        let setup_context = PluginSetupReadinessContext::default();
        let plan = translator.plan_activation(&translation, &matrix, &setup_context);

        assert_eq!(plan.total_plugins, 1);
        assert_eq!(plan.ready_plugins, 0);
        assert_eq!(plan.pending_plugins, 0);
        assert_eq!(plan.blocked_plugins, 1);
        assert!(matches!(
            plan.candidates[0].status,
            PluginActivationStatus::BlockedUnsupportedAdapterFamily
        ));
    }

    #[test]
    fn activation_plan_marks_plugin_pending_when_required_setup_is_missing() {
        let descriptor = descriptor("manifest", BTreeMap::new());
        let translator = PluginTranslator::new();
        let translation = translator.translate_scan_report(&PluginScanReport {
            scanned_files: 1,
            matched_plugins: 1,
            descriptors: vec![descriptor],
        });

        let matrix = BridgeSupportMatrix {
            supported_bridges: BTreeSet::from([PluginBridgeKind::HttpJson]),
            supported_adapter_families: BTreeSet::new(),
        };
        let setup_context = PluginSetupReadinessContext::default();
        let plan = translator.plan_activation(&translation, &matrix, &setup_context);

        assert_eq!(plan.total_plugins, 1);
        assert_eq!(plan.ready_plugins, 0);
        assert_eq!(plan.pending_plugins, 1);
        assert_eq!(plan.blocked_plugins, 0);
        assert!(matches!(
            plan.candidates[0].status,
            PluginActivationStatus::PendingSetup
        ));
        assert_eq!(
            plan.candidates[0].missing_required_env_vars,
            vec!["TAVILY_API_KEY".to_owned()]
        );
        assert_eq!(
            plan.candidates[0].missing_required_config_keys,
            vec!["tools.web_search.default_provider".to_owned()]
        );
    }

    #[test]
    fn activation_plan_marks_plugin_ready_when_required_setup_is_satisfied() {
        let descriptor = descriptor("manifest", BTreeMap::new());
        let translator = PluginTranslator::new();
        let translation = translator.translate_scan_report(&PluginScanReport {
            scanned_files: 1,
            matched_plugins: 1,
            descriptors: vec![descriptor],
        });

        let matrix = BridgeSupportMatrix {
            supported_bridges: BTreeSet::from([PluginBridgeKind::HttpJson]),
            supported_adapter_families: BTreeSet::new(),
        };
        let setup_context = PluginSetupReadinessContext {
            available_env_vars: BTreeSet::from(["TAVILY_API_KEY".to_owned()]),
            configured_keys: BTreeSet::from(["tools.web_search.default_provider".to_owned()]),
            config_keys_verified: true,
        };
        let plan = translator.plan_activation(&translation, &matrix, &setup_context);

        assert_eq!(plan.total_plugins, 1);
        assert_eq!(plan.ready_plugins, 1);
        assert_eq!(plan.pending_plugins, 0);
        assert_eq!(plan.blocked_plugins, 0);
        assert!(matches!(
            plan.candidates[0].status,
            PluginActivationStatus::Ready
        ));
        assert!(plan.candidates[0].missing_required_env_vars.is_empty());
        assert!(plan.candidates[0].missing_required_config_keys.is_empty());
    }
}
