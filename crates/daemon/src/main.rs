use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use clap::{Parser, Subcommand};
use ed25519_dalek::{Signature as Ed25519Signature, Verifier, VerifyingKey};
use futures_util::stream::{FuturesUnordered, StreamExt};
use kernel::{
    ArchitectureBoundaryPolicy, ArchitectureGuardReport, AuditEvent, AuditEventKind, AuditSink,
    AutoProvisionAgent, AutoProvisionRequest, BootstrapPolicy, BootstrapReport,
    BootstrapTaskStatus, BridgeSupportMatrix, Capability, Clock, CodebaseAwarenessConfig,
    CodebaseAwarenessEngine, CodebaseAwarenessSnapshot, ConnectorAdapter, ConnectorCommand,
    ConnectorError, ConnectorOutcome, CoreConnectorAdapter, CoreMemoryAdapter, CoreRuntimeAdapter,
    CoreToolAdapter, ExecutionRoute, FixedClock, HarnessAdapter, HarnessError, HarnessKind,
    HarnessOutcome, HarnessRequest, InMemoryAuditSink, IntegrationCatalog, IntegrationHotfix,
    LoongClawKernel, MemoryCoreOutcome, MemoryCoreRequest, MemoryExtensionAdapter,
    MemoryExtensionOutcome, MemoryExtensionRequest, PluginAbsorbReport, PluginActivationPlan,
    PluginActivationStatus, PluginBootstrapExecutor, PluginBridgeKind, PluginDescriptor,
    PluginScanReport, PluginScanner, PluginTranslationReport, PluginTranslator, ProvisionPlan,
    RuntimeCoreOutcome, RuntimeCoreRequest, RuntimeExtensionAdapter, RuntimeExtensionOutcome,
    RuntimeExtensionRequest, StaticPolicyEngine, SystemClock, TaskIntent, ToolCoreOutcome,
    ToolCoreRequest, ToolExtensionAdapter, ToolExtensionOutcome, ToolExtensionRequest,
    VerticalPackManifest,
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::time::{sleep, Instant as TokioInstant};
use wasmparser::{Parser as WasmParser, Payload as WasmPayload};
use wasmtime::{
    Config as WasmtimeConfig, Engine as WasmtimeEngine, Linker as WasmtimeLinker,
    Module as WasmtimeModule, Store as WasmtimeStore,
};

const DEFAULT_PACK_ID: &str = "dev-automation";
const DEFAULT_AGENT_ID: &str = "agent-dev-01";
static BUNDLED_APPROVAL_RISK_PROFILE: OnceLock<ApprovalRiskProfile> = OnceLock::new();
static BUNDLED_SECURITY_SCAN_PROFILE: OnceLock<SecurityScanProfile> = OnceLock::new();
static WEBHOOK_TEST_RETRY_STATE: OnceLock<Mutex<BTreeMap<String, usize>>> = OnceLock::new();
type CliResult<T> = Result<T, String>;

mod mvp;
mod pressure_benchmark;
mod programmatic;
use pressure_benchmark::run_programmatic_pressure_benchmark_cli;
use programmatic::{
    acquire_programmatic_circuit_slot, execute_programmatic_tool_call,
    record_programmatic_circuit_outcome,
};
#[cfg(test)]
mod tests;

#[derive(Parser, Debug)]
#[command(name = "loongclawd", about = "LoongClaw low-level runtime daemon")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the original end-to-end bootstrap demo
    Demo,
    /// Execute one task through the kernel+harness path
    RunTask {
        #[arg(long)]
        objective: String,
        #[arg(long, default_value = "{}")]
        payload: String,
    },
    /// Invoke one connector operation through kernel policy gate
    InvokeConnector {
        #[arg(long)]
        operation: String,
        #[arg(long, default_value = "{}")]
        payload: String,
    },
    /// Demonstrate audit lifecycle with fixed clock and token revocation
    AuditDemo,
    /// Generate a runnable JSON spec template for quick vertical customization
    InitSpec {
        #[arg(long, default_value = "loongclaw.spec.json")]
        output: String,
    },
    /// Run a full workflow from a JSON spec (task/connector/runtime/tool/memory)
    RunSpec {
        #[arg(long)]
        spec: String,
        #[arg(long, default_value_t = false)]
        print_audit: bool,
    },
    /// Run pressure benchmarks for programmatic orchestration and optional regression gate checks
    BenchmarkProgrammaticPressure {
        #[arg(
            long,
            default_value = "examples/benchmarks/programmatic-pressure-matrix.json"
        )]
        matrix: String,
        #[arg(long)]
        baseline: Option<String>,
        #[arg(
            long,
            default_value = "target/benchmarks/programmatic-pressure-report.json"
        )]
        output: String,
        #[arg(long, default_value_t = false)]
        enforce_gate: bool,
    },
    /// Generate a beginner-friendly TOML config and bootstrap local state
    Setup {
        #[arg(long)]
        output: Option<String>,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Start interactive CLI chat channel with sliding-window memory
    Chat {
        #[arg(long)]
        config: Option<String>,
        #[arg(long)]
        session: Option<String>,
    },
    /// Run Telegram channel polling/response loop
    TelegramServe {
        #[arg(long)]
        config: Option<String>,
        #[arg(long, default_value_t = false)]
        once: bool,
    },
    /// Send one Feishu message or card
    FeishuSend {
        #[arg(long)]
        config: Option<String>,
        #[arg(long)]
        receive_id: String,
        #[arg(long)]
        text: String,
        #[arg(long, default_value_t = false)]
        card: bool,
    },
    /// Run Feishu event callback server and auto-reply via provider
    FeishuServe {
        #[arg(long)]
        config: Option<String>,
        #[arg(long)]
        bind: Option<String>,
        #[arg(long)]
        path: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DefaultCoreSelection {
    connector: Option<String>,
    runtime: Option<String>,
    tool: Option<String>,
    memory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum OperationSpec {
    Task {
        task_id: String,
        objective: String,
        required_capabilities: BTreeSet<Capability>,
        payload: Value,
    },
    ConnectorLegacy {
        connector_name: String,
        operation: String,
        required_capabilities: BTreeSet<Capability>,
        payload: Value,
    },
    ConnectorCore {
        connector_name: String,
        operation: String,
        required_capabilities: BTreeSet<Capability>,
        payload: Value,
        core: Option<String>,
    },
    ConnectorExtension {
        connector_name: String,
        operation: String,
        required_capabilities: BTreeSet<Capability>,
        payload: Value,
        extension: String,
        core: Option<String>,
    },
    RuntimeCore {
        action: String,
        required_capabilities: BTreeSet<Capability>,
        payload: Value,
        core: Option<String>,
    },
    RuntimeExtension {
        action: String,
        required_capabilities: BTreeSet<Capability>,
        payload: Value,
        extension: String,
        core: Option<String>,
    },
    ToolCore {
        tool_name: String,
        required_capabilities: BTreeSet<Capability>,
        payload: Value,
        core: Option<String>,
    },
    ToolExtension {
        extension_action: String,
        required_capabilities: BTreeSet<Capability>,
        payload: Value,
        extension: String,
        core: Option<String>,
    },
    MemoryCore {
        operation: String,
        required_capabilities: BTreeSet<Capability>,
        payload: Value,
        core: Option<String>,
    },
    MemoryExtension {
        operation: String,
        required_capabilities: BTreeSet<Capability>,
        payload: Value,
        extension: String,
        core: Option<String>,
    },
    ToolSearch {
        query: String,
        #[serde(default = "default_tool_search_limit")]
        limit: usize,
        #[serde(default)]
        include_deferred: bool,
        #[serde(default)]
        include_examples: bool,
    },
    ProgrammaticToolCall {
        caller: String,
        #[serde(default = "default_programmatic_max_calls")]
        max_calls: usize,
        #[serde(default)]
        include_intermediate: bool,
        #[serde(default)]
        allowed_connectors: BTreeSet<String>,
        #[serde(default)]
        connector_rate_limits: BTreeMap<String, ProgrammaticConnectorRateLimit>,
        #[serde(default)]
        connector_circuit_breakers: BTreeMap<String, ProgrammaticCircuitBreakerPolicy>,
        #[serde(default = "default_programmatic_concurrency_policy")]
        concurrency: ProgrammaticConcurrencyPolicy,
        #[serde(default)]
        return_step: Option<String>,
        steps: Vec<ProgrammaticStep>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ProgrammaticStep {
    SetLiteral {
        step_id: String,
        value: Value,
    },
    JsonPointer {
        step_id: String,
        from_step: String,
        pointer: String,
    },
    ConnectorCall {
        step_id: String,
        connector_name: String,
        operation: String,
        #[serde(default)]
        required_capabilities: BTreeSet<Capability>,
        #[serde(default)]
        retry: Option<ProgrammaticRetryPolicy>,
        #[serde(default = "default_programmatic_priority_class")]
        priority_class: ProgrammaticPriorityClass,
        #[serde(default)]
        payload: Value,
    },
    ConnectorBatch {
        step_id: String,
        #[serde(default = "default_true")]
        parallel: bool,
        #[serde(default)]
        continue_on_error: bool,
        calls: Vec<ProgrammaticBatchCall>,
    },
    Conditional {
        step_id: String,
        from_step: String,
        #[serde(default)]
        pointer: Option<String>,
        #[serde(default)]
        equals: Option<Value>,
        #[serde(default)]
        exists: Option<bool>,
        when_true: Value,
        #[serde(default)]
        when_false: Option<Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProgrammaticBatchCall {
    call_id: String,
    connector_name: String,
    operation: String,
    #[serde(default)]
    required_capabilities: BTreeSet<Capability>,
    #[serde(default)]
    retry: Option<ProgrammaticRetryPolicy>,
    #[serde(default = "default_programmatic_priority_class")]
    priority_class: ProgrammaticPriorityClass,
    #[serde(default)]
    payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProgrammaticRetryPolicy {
    #[serde(default = "default_programmatic_retry_max_attempts")]
    max_attempts: usize,
    #[serde(default = "default_programmatic_retry_initial_backoff_ms")]
    initial_backoff_ms: u64,
    #[serde(default = "default_programmatic_retry_max_backoff_ms")]
    max_backoff_ms: u64,
    #[serde(default = "default_programmatic_retry_jitter_ratio")]
    jitter_ratio: f64,
    #[serde(default = "default_true")]
    adaptive_jitter: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProgrammaticConnectorRateLimit {
    min_interval_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
enum ProgrammaticPriorityClass {
    High,
    Normal,
    Low,
}

impl ProgrammaticPriorityClass {
    fn as_str(self) -> &'static str {
        match self {
            ProgrammaticPriorityClass::High => "high",
            ProgrammaticPriorityClass::Normal => "normal",
            ProgrammaticPriorityClass::Low => "low",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ProgrammaticFairnessPolicy {
    StrictRoundRobin,
    WeightedRoundRobin,
}

impl ProgrammaticFairnessPolicy {
    fn as_str(self) -> &'static str {
        match self {
            ProgrammaticFairnessPolicy::StrictRoundRobin => "strict_round_robin",
            ProgrammaticFairnessPolicy::WeightedRoundRobin => "weighted_round_robin",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
enum ProgrammaticAdaptiveReduceOn {
    AnyError,
    ConnectorExecutionError,
    CircuitOpen,
    ConnectorNotFound,
    ConnectorNotAllowed,
    CapabilityDenied,
    PolicyDenied,
}

impl ProgrammaticAdaptiveReduceOn {
    fn as_str(self) -> &'static str {
        match self {
            ProgrammaticAdaptiveReduceOn::AnyError => "any_error",
            ProgrammaticAdaptiveReduceOn::ConnectorExecutionError => "connector_execution_error",
            ProgrammaticAdaptiveReduceOn::CircuitOpen => "circuit_open",
            ProgrammaticAdaptiveReduceOn::ConnectorNotFound => "connector_not_found",
            ProgrammaticAdaptiveReduceOn::ConnectorNotAllowed => "connector_not_allowed",
            ProgrammaticAdaptiveReduceOn::CapabilityDenied => "capability_denied",
            ProgrammaticAdaptiveReduceOn::PolicyDenied => "policy_denied",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProgrammaticConcurrencyPolicy {
    #[serde(default = "default_programmatic_concurrency_max_in_flight")]
    max_in_flight: usize,
    #[serde(default = "default_programmatic_concurrency_min_in_flight")]
    min_in_flight: usize,
    #[serde(default = "default_programmatic_fairness_policy")]
    fairness: ProgrammaticFairnessPolicy,
    #[serde(default = "default_true")]
    adaptive_budget: bool,
    #[serde(default = "default_programmatic_priority_high_weight")]
    high_weight: usize,
    #[serde(default = "default_programmatic_priority_normal_weight")]
    normal_weight: usize,
    #[serde(default = "default_programmatic_priority_low_weight")]
    low_weight: usize,
    #[serde(default = "default_programmatic_adaptive_recovery_successes")]
    adaptive_recovery_successes: usize,
    #[serde(default = "default_programmatic_adaptive_upshift_step")]
    adaptive_upshift_step: usize,
    #[serde(default = "default_programmatic_adaptive_downshift_step")]
    adaptive_downshift_step: usize,
    #[serde(default = "default_programmatic_adaptive_reduce_on")]
    adaptive_reduce_on: BTreeSet<ProgrammaticAdaptiveReduceOn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProgrammaticCircuitBreakerPolicy {
    #[serde(default = "default_programmatic_circuit_failure_threshold")]
    failure_threshold: usize,
    #[serde(default = "default_programmatic_circuit_cooldown_ms")]
    cooldown_ms: u64,
    #[serde(default = "default_programmatic_circuit_half_open_max_calls")]
    half_open_max_calls: usize,
    #[serde(default = "default_programmatic_circuit_success_threshold")]
    success_threshold: usize,
}

#[derive(Debug, Clone)]
struct PreparedProgrammaticBatchCall {
    call_id: String,
    connector_name: String,
    operation: String,
    required_capabilities: BTreeSet<Capability>,
    retry_policy: ProgrammaticRetryPolicy,
    priority_class: ProgrammaticPriorityClass,
    payload: Value,
}

#[derive(Debug, Clone, Serialize)]
struct ProgrammaticInvocationMetrics {
    attempts: usize,
    retries: usize,
    priority_class: String,
    rate_wait_ms_total: u64,
    backoff_ms_total: u64,
    circuit_phase_before: String,
    circuit_phase_after: String,
}

#[derive(Debug, Clone, Serialize)]
struct ProgrammaticBatchExecutionSummary {
    mode: String,
    fairness: String,
    adaptive_budget: bool,
    configured_max_in_flight: usize,
    configured_min_in_flight: usize,
    peak_in_flight: usize,
    final_in_flight_budget: usize,
    budget_reductions: usize,
    budget_increases: usize,
    adaptive_upshift_step: usize,
    adaptive_downshift_step: usize,
    adaptive_reduce_on: Vec<String>,
    scheduler_wait_cycles: usize,
    dispatch_order: Vec<String>,
    priority_dispatch_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgrammaticCircuitPhase {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
struct ProgrammaticCircuitRuntimeState {
    phase: ProgrammaticCircuitPhase,
    consecutive_failures: usize,
    open_until: Option<TokioInstant>,
    half_open_remaining_calls: usize,
    half_open_successes: usize,
}

impl Default for ProgrammaticCircuitRuntimeState {
    fn default() -> Self {
        Self {
            phase: ProgrammaticCircuitPhase::Closed,
            consecutive_failures: 0,
            open_until: None,
            half_open_remaining_calls: 0,
            half_open_successes: 0,
        }
    }
}

fn default_tool_search_limit() -> usize {
    8
}

fn default_programmatic_max_calls() -> usize {
    12
}

fn default_programmatic_retry_max_attempts() -> usize {
    1
}

fn default_programmatic_retry_initial_backoff_ms() -> u64 {
    100
}

fn default_programmatic_retry_max_backoff_ms() -> u64 {
    2_000
}

fn default_programmatic_retry_jitter_ratio() -> f64 {
    0.2
}

fn default_programmatic_priority_class() -> ProgrammaticPriorityClass {
    ProgrammaticPriorityClass::Normal
}

fn default_programmatic_concurrency_max_in_flight() -> usize {
    4
}

fn default_programmatic_concurrency_min_in_flight() -> usize {
    1
}

fn default_programmatic_fairness_policy() -> ProgrammaticFairnessPolicy {
    ProgrammaticFairnessPolicy::WeightedRoundRobin
}

fn default_programmatic_priority_high_weight() -> usize {
    4
}

fn default_programmatic_priority_normal_weight() -> usize {
    2
}

fn default_programmatic_priority_low_weight() -> usize {
    1
}

fn default_programmatic_adaptive_recovery_successes() -> usize {
    2
}

fn default_programmatic_adaptive_upshift_step() -> usize {
    1
}

fn default_programmatic_adaptive_downshift_step() -> usize {
    1
}

fn default_programmatic_adaptive_reduce_on() -> BTreeSet<ProgrammaticAdaptiveReduceOn> {
    BTreeSet::from([
        ProgrammaticAdaptiveReduceOn::ConnectorExecutionError,
        ProgrammaticAdaptiveReduceOn::CircuitOpen,
    ])
}

fn default_programmatic_concurrency_policy() -> ProgrammaticConcurrencyPolicy {
    ProgrammaticConcurrencyPolicy {
        max_in_flight: default_programmatic_concurrency_max_in_flight(),
        min_in_flight: default_programmatic_concurrency_min_in_flight(),
        fairness: default_programmatic_fairness_policy(),
        adaptive_budget: default_true(),
        high_weight: default_programmatic_priority_high_weight(),
        normal_weight: default_programmatic_priority_normal_weight(),
        low_weight: default_programmatic_priority_low_weight(),
        adaptive_recovery_successes: default_programmatic_adaptive_recovery_successes(),
        adaptive_upshift_step: default_programmatic_adaptive_upshift_step(),
        adaptive_downshift_step: default_programmatic_adaptive_downshift_step(),
        adaptive_reduce_on: default_programmatic_adaptive_reduce_on(),
    }
}

fn default_programmatic_circuit_failure_threshold() -> usize {
    3
}

fn default_programmatic_circuit_cooldown_ms() -> u64 {
    1_000
}

fn default_programmatic_circuit_half_open_max_calls() -> usize {
    1
}

fn default_programmatic_circuit_success_threshold() -> usize {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunnerSpec {
    pack: VerticalPackManifest,
    agent_id: String,
    ttl_s: u64,
    #[serde(default)]
    approval: Option<HumanApprovalSpec>,
    defaults: Option<DefaultCoreSelection>,
    self_awareness: Option<SelfAwarenessSpec>,
    plugin_scan: Option<PluginScanSpec>,
    bridge_support: Option<BridgeSupportSpec>,
    bootstrap: Option<BootstrapSpec>,
    auto_provision: Option<AutoProvisionSpec>,
    #[serde(default)]
    hotfixes: Vec<HotfixSpec>,
    operation: OperationSpec,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum HumanApprovalMode {
    Disabled,
    #[default]
    MediumBalanced,
    Strict,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum HumanApprovalStrategy {
    #[default]
    PerCall,
    OneTimeFullAccess,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum HumanApprovalScope {
    #[default]
    ToolCalls,
    AllOperations,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HumanApprovalSpec {
    #[serde(default)]
    mode: HumanApprovalMode,
    #[serde(default)]
    strategy: HumanApprovalStrategy,
    #[serde(default)]
    scope: HumanApprovalScope,
    #[serde(default)]
    one_time_full_access_granted: bool,
    #[serde(default)]
    approved_calls: Vec<String>,
    #[serde(default)]
    denied_calls: Vec<String>,
    #[serde(default)]
    risk_profile_path: Option<String>,
    #[serde(default)]
    one_time_full_access_expires_at_epoch_s: Option<u64>,
    #[serde(default)]
    one_time_full_access_remaining_uses: Option<u32>,
    #[serde(default)]
    high_risk_keywords: Vec<String>,
    #[serde(default)]
    high_risk_tool_names: Vec<String>,
    #[serde(default)]
    high_risk_payload_keys: Vec<String>,
}

impl Default for HumanApprovalSpec {
    fn default() -> Self {
        Self {
            mode: HumanApprovalMode::MediumBalanced,
            strategy: HumanApprovalStrategy::PerCall,
            scope: HumanApprovalScope::ToolCalls,
            one_time_full_access_granted: false,
            approved_calls: Vec::new(),
            denied_calls: Vec::new(),
            risk_profile_path: None,
            one_time_full_access_expires_at_epoch_s: None,
            one_time_full_access_remaining_uses: None,
            // Keep inline overrides empty by default so policy is profile-driven.
            high_risk_keywords: Vec::new(),
            high_risk_tool_names: Vec::new(),
            high_risk_payload_keys: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ApprovalRiskLevel {
    Low,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApprovalDecisionReport {
    mode: HumanApprovalMode,
    strategy: HumanApprovalStrategy,
    scope: HumanApprovalScope,
    now_epoch_s: u64,
    operation_key: String,
    operation_kind: &'static str,
    risk_level: ApprovalRiskLevel,
    risk_score: u8,
    denylisted: bool,
    requires_human_approval: bool,
    approved: bool,
    reason: String,
    matched_keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApprovalRiskScoring {
    keyword_weight: u8,
    tool_name_weight: u8,
    payload_key_weight: u8,
    keyword_hit_cap: usize,
    payload_key_hit_cap: usize,
    high_risk_threshold: u8,
}

impl Default for ApprovalRiskScoring {
    fn default() -> Self {
        Self {
            keyword_weight: 20,
            tool_name_weight: 40,
            payload_key_weight: 20,
            keyword_hit_cap: 4,
            payload_key_hit_cap: 2,
            high_risk_threshold: 20,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApprovalRiskProfile {
    #[serde(default)]
    high_risk_keywords: Vec<String>,
    #[serde(default)]
    high_risk_tool_names: Vec<String>,
    #[serde(default)]
    high_risk_payload_keys: Vec<String>,
    #[serde(default)]
    scoring: ApprovalRiskScoring,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SelfAwarenessSpec {
    enabled: bool,
    #[serde(default)]
    roots: Vec<String>,
    #[serde(default)]
    plugin_roots: Vec<String>,
    #[serde(default)]
    proposed_mutations: Vec<String>,
    #[serde(default)]
    enforce_guard: bool,
    #[serde(default)]
    immutable_core_paths: Vec<String>,
    #[serde(default)]
    mutable_extension_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginScanSpec {
    enabled: bool,
    roots: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BridgeSupportSpec {
    enabled: bool,
    #[serde(default)]
    supported_bridges: Vec<PluginBridgeKind>,
    #[serde(default)]
    supported_adapter_families: Vec<String>,
    #[serde(default)]
    enforce_supported: bool,
    #[serde(default)]
    policy_version: Option<String>,
    #[serde(default)]
    expected_checksum: Option<String>,
    #[serde(default)]
    expected_sha256: Option<String>,
    #[serde(default)]
    execute_process_stdio: bool,
    #[serde(default)]
    execute_http_json: bool,
    #[serde(default)]
    allowed_process_commands: Vec<String>,
    #[serde(default)]
    enforce_execution_success: bool,
    #[serde(default)]
    security_scan: Option<SecurityScanSpec>,
}

#[derive(Debug, Clone, Default)]
struct BridgeRuntimePolicy {
    execute_process_stdio: bool,
    execute_http_json: bool,
    execute_wasm_component: bool,
    allowed_process_commands: BTreeSet<String>,
    wasm_allowed_path_prefixes: Vec<PathBuf>,
    wasm_max_component_bytes: Option<usize>,
    wasm_fuel_limit: Option<u64>,
    enforce_execution_success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecurityScanSpec {
    enabled: bool,
    #[serde(default = "default_true")]
    block_on_high: bool,
    #[serde(default)]
    profile_path: Option<String>,
    #[serde(default)]
    profile_sha256: Option<String>,
    #[serde(default)]
    profile_signature: Option<SecurityProfileSignatureSpec>,
    #[serde(default)]
    siem_export: Option<SecuritySiemExportSpec>,
    #[serde(default)]
    runtime: SecurityRuntimeExecutionSpec,
    #[serde(default)]
    high_risk_metadata_keywords: Vec<String>,
    #[serde(default)]
    wasm: WasmSecurityScanSpec,
}

impl Default for SecurityScanSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            block_on_high: true,
            profile_path: None,
            profile_sha256: None,
            profile_signature: None,
            siem_export: None,
            runtime: SecurityRuntimeExecutionSpec::default(),
            high_risk_metadata_keywords: Vec::new(),
            wasm: WasmSecurityScanSpec::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecurityProfileSignatureSpec {
    #[serde(default = "default_security_profile_signature_algorithm")]
    algorithm: String,
    public_key_base64: String,
    signature_base64: String,
}

fn default_security_profile_signature_algorithm() -> String {
    "ed25519".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecuritySiemExportSpec {
    enabled: bool,
    path: String,
    #[serde(default = "default_true")]
    include_findings: bool,
    #[serde(default)]
    max_findings_per_record: Option<usize>,
    #[serde(default)]
    fail_on_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecuritySiemExportReport {
    enabled: bool,
    path: String,
    success: bool,
    exported_records: usize,
    exported_findings: usize,
    truncated_findings: usize,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SecurityRuntimeExecutionSpec {
    #[serde(default)]
    execute_wasm_component: bool,
    #[serde(default)]
    allowed_path_prefixes: Vec<String>,
    #[serde(default)]
    max_component_bytes: Option<usize>,
    #[serde(default)]
    fuel_limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WasmSecurityScanSpec {
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    max_module_bytes: usize,
    #[serde(default)]
    allow_wasi: bool,
    #[serde(default)]
    blocked_import_prefixes: Vec<String>,
    #[serde(default)]
    allowed_path_prefixes: Vec<String>,
    #[serde(default)]
    require_hash_pin: bool,
    #[serde(default)]
    required_sha256_by_plugin: BTreeMap<String, String>,
}

impl Default for WasmSecurityScanSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            max_module_bytes: 0,
            allow_wasi: false,
            blocked_import_prefixes: Vec::new(),
            allowed_path_prefixes: Vec::new(),
            require_hash_pin: false,
            required_sha256_by_plugin: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecurityScanProfile {
    #[serde(default)]
    high_risk_metadata_keywords: Vec<String>,
    #[serde(default)]
    wasm: WasmSecurityScanSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SecurityFindingSeverity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecurityFinding {
    correlation_id: String,
    severity: SecurityFindingSeverity,
    category: String,
    plugin_id: String,
    source_path: String,
    message: String,
    evidence: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecurityScanReport {
    enabled: bool,
    scanned_plugins: usize,
    total_findings: usize,
    high_findings: usize,
    medium_findings: usize,
    low_findings: usize,
    blocked: bool,
    block_reason: Option<String>,
    siem_export: Option<SecuritySiemExportReport>,
    findings: Vec<SecurityFinding>,
}

impl Default for SecurityScanReport {
    fn default() -> Self {
        Self {
            enabled: true,
            scanned_plugins: 0,
            total_findings: 0,
            high_findings: 0,
            medium_findings: 0,
            low_findings: 0,
            blocked: false,
            block_reason: None,
            siem_export: None,
            findings: Vec::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BootstrapSpec {
    enabled: bool,
    #[serde(default)]
    allow_http_json_auto_apply: Option<bool>,
    #[serde(default)]
    allow_process_stdio_auto_apply: Option<bool>,
    #[serde(default)]
    allow_native_ffi_auto_apply: Option<bool>,
    #[serde(default)]
    allow_wasm_component_auto_apply: Option<bool>,
    #[serde(default)]
    allow_mcp_server_auto_apply: Option<bool>,
    #[serde(default)]
    enforce_ready_execution: Option<bool>,
    #[serde(default)]
    max_tasks: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutoProvisionSpec {
    enabled: bool,
    provider_id: String,
    channel_id: String,
    connector_name: Option<String>,
    endpoint: Option<String>,
    required_capabilities: BTreeSet<Capability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum HotfixSpec {
    ProviderVersion {
        provider_id: String,
        new_version: String,
    },
    ProviderConnector {
        provider_id: String,
        new_connector_name: String,
    },
    ChannelEndpoint {
        channel_id: String,
        new_endpoint: String,
    },
    ChannelEnabled {
        channel_id: String,
        enabled: bool,
    },
}

impl HotfixSpec {
    fn to_kernel_hotfix(&self) -> IntegrationHotfix {
        match self {
            Self::ProviderVersion {
                provider_id,
                new_version,
            } => IntegrationHotfix::ProviderVersion {
                provider_id: provider_id.clone(),
                new_version: new_version.clone(),
            },
            Self::ProviderConnector {
                provider_id,
                new_connector_name,
            } => IntegrationHotfix::ProviderConnector {
                provider_id: provider_id.clone(),
                new_connector_name: new_connector_name.clone(),
            },
            Self::ChannelEndpoint {
                channel_id,
                new_endpoint,
            } => IntegrationHotfix::ChannelEndpoint {
                channel_id: channel_id.clone(),
                new_endpoint: new_endpoint.clone(),
            },
            Self::ChannelEnabled {
                channel_id,
                enabled,
            } => IntegrationHotfix::ChannelEnabled {
                channel_id: channel_id.clone(),
                enabled: *enabled,
            },
        }
    }
}

impl RunnerSpec {
    fn template() -> Self {
        Self {
            pack: VerticalPackManifest {
                pack_id: "sales-intel-local".to_owned(),
                domain: "sales".to_owned(),
                version: "0.1.0".to_owned(),
                default_route: ExecutionRoute {
                    harness_kind: HarnessKind::EmbeddedPi,
                    adapter: Some("pi-local".to_owned()),
                },
                allowed_connectors: BTreeSet::from(["webhook".to_owned(), "crm".to_owned()]),
                granted_capabilities: BTreeSet::from([
                    Capability::InvokeTool,
                    Capability::InvokeConnector,
                    Capability::MemoryRead,
                    Capability::MemoryWrite,
                    Capability::ObserveTelemetry,
                ]),
                metadata: BTreeMap::from([
                    ("owner".to_owned(), "platform-team".to_owned()),
                    ("stage".to_owned(), "prototype".to_owned()),
                ]),
            },
            agent_id: "agent-spec-01".to_owned(),
            ttl_s: 300,
            approval: Some(HumanApprovalSpec::default()),
            defaults: Some(DefaultCoreSelection {
                connector: Some("http-core".to_owned()),
                runtime: Some("native-core".to_owned()),
                tool: Some("core-tools".to_owned()),
                memory: Some("kv-core".to_owned()),
            }),
            self_awareness: None,
            plugin_scan: None,
            bridge_support: None,
            bootstrap: None,
            auto_provision: Some(AutoProvisionSpec {
                enabled: true,
                provider_id: "openrouter".to_owned(),
                channel_id: "primary".to_owned(),
                connector_name: Some("openrouter".to_owned()),
                endpoint: Some("https://openrouter.ai/api/v1/chat/completions".to_owned()),
                required_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            }),
            hotfixes: Vec::new(),
            operation: OperationSpec::RuntimeExtension {
                action: "start-session".to_owned(),
                required_capabilities: BTreeSet::from([Capability::ObserveTelemetry]),
                payload: json!({"session_id": "s-42"}),
                extension: "acp-bridge".to_owned(),
                core: None,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct SpecRunReport {
    pack_id: String,
    agent_id: String,
    operation_kind: &'static str,
    blocked_reason: Option<String>,
    approval_guard: ApprovalDecisionReport,
    bridge_support_checksum: Option<String>,
    bridge_support_sha256: Option<String>,
    self_awareness: Option<CodebaseAwarenessSnapshot>,
    architecture_guard: Option<ArchitectureGuardReport>,
    plugin_scan_reports: Vec<PluginScanReport>,
    plugin_translation_reports: Vec<PluginTranslationReport>,
    plugin_activation_plans: Vec<PluginActivationPlan>,
    plugin_bootstrap_reports: Vec<BootstrapReport>,
    plugin_bootstrap_queue: Vec<String>,
    plugin_absorb_reports: Vec<PluginAbsorbReport>,
    security_scan_report: Option<SecurityScanReport>,
    auto_provision_plan: Option<ProvisionPlan>,
    outcome: Value,
    integration_catalog: IntegrationCatalog,
    audit_events: Option<Vec<AuditEvent>>,
}

#[derive(Debug, Clone)]
struct ToolSearchEntry {
    tool_id: String,
    plugin_id: Option<String>,
    connector_name: String,
    provider_id: String,
    source_path: Option<String>,
    bridge_kind: PluginBridgeKind,
    adapter_family: Option<String>,
    entrypoint_hint: Option<String>,
    source_language: Option<String>,
    summary: Option<String>,
    tags: Vec<String>,
    input_examples: Vec<Value>,
    output_examples: Vec<Value>,
    deferred: bool,
    loaded: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ToolSearchResult {
    tool_id: String,
    plugin_id: Option<String>,
    connector_name: String,
    provider_id: String,
    source_path: Option<String>,
    bridge_kind: String,
    adapter_family: Option<String>,
    entrypoint_hint: Option<String>,
    source_language: Option<String>,
    score: u32,
    deferred: bool,
    loaded: bool,
    summary: Option<String>,
    tags: Vec<String>,
    input_examples: Vec<Value>,
    output_examples: Vec<Value>,
}

struct EmbeddedPiHarness {
    seen: Mutex<Vec<String>>,
}

#[async_trait]
impl HarnessAdapter for EmbeddedPiHarness {
    fn name(&self) -> &str {
        "pi-local"
    }

    fn kind(&self) -> HarnessKind {
        HarnessKind::EmbeddedPi
    }

    async fn execute(&self, request: HarnessRequest) -> Result<HarnessOutcome, HarnessError> {
        self.seen
            .lock()
            .expect("mutex poisoned")
            .push(request.task_id.clone());

        Ok(HarnessOutcome {
            status: "ok".to_owned(),
            output: json!({
                "adapter": "pi-local",
                "task": request.task_id,
                "objective": request.objective,
            }),
        })
    }
}

struct WebhookConnector;

#[async_trait]
impl ConnectorAdapter for WebhookConnector {
    fn name(&self) -> &str {
        "webhook"
    }

    async fn invoke(&self, command: ConnectorCommand) -> Result<ConnectorOutcome, ConnectorError> {
        if let Some(test_config) = command
            .payload
            .as_object()
            .and_then(|payload| payload.get("_loongclaw_test"))
            .and_then(Value::as_object)
        {
            let delay_ms = test_config
                .get("delay_ms")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if delay_ms > 0 {
                sleep(Duration::from_millis(delay_ms)).await;
            }
            let request_id = test_config
                .get("request_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_owned();
            let failures_before_success = test_config
                .get("failures_before_success")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            if !request_id.is_empty() && failures_before_success > 0 {
                let attempts_map =
                    WEBHOOK_TEST_RETRY_STATE.get_or_init(|| Mutex::new(BTreeMap::new()));
                let current_attempt = {
                    let mut guard = attempts_map.lock().map_err(|_| {
                        ConnectorError::Execution("retry test state mutex poisoned".to_owned())
                    })?;
                    let entry = guard.entry(request_id.clone()).or_insert(0);
                    *entry = entry.saturating_add(1);
                    *entry
                };
                if current_attempt <= failures_before_success {
                    return Err(ConnectorError::Execution(format!(
                        "simulated transient failure for request_id={request_id}, attempt={current_attempt}, threshold={failures_before_success}"
                    )));
                }
            }
        }

        Ok(ConnectorOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "connector": "webhook",
                "operation": command.operation,
                "payload": command.payload,
            }),
        })
    }
}

struct DynamicCatalogConnector {
    connector_name: String,
    provider_id: String,
    catalog: Arc<Mutex<IntegrationCatalog>>,
    bridge_runtime_policy: BridgeRuntimePolicy,
}

#[async_trait]
impl ConnectorAdapter for DynamicCatalogConnector {
    fn name(&self) -> &str {
        &self.connector_name
    }

    async fn invoke(&self, command: ConnectorCommand) -> Result<ConnectorOutcome, ConnectorError> {
        let catalog = self.catalog.lock().map_err(|_| {
            ConnectorError::Execution("integration catalog mutex poisoned".to_owned())
        })?;

        let provider = catalog.provider(&self.provider_id).ok_or_else(|| {
            ConnectorError::Execution(format!(
                "provider {} is not registered in integration catalog",
                self.provider_id
            ))
        })?;

        let allowed_callers = provider_allowed_callers(provider);
        if !allowed_callers.is_empty() {
            let caller = caller_from_payload(&command.payload);
            if !caller_is_allowed(caller.as_deref(), &allowed_callers) {
                let caller_label = caller.unwrap_or_else(|| "unknown".to_owned());
                return Err(ConnectorError::Execution(format!(
                    "caller {caller_label} is not allowed for connector {} (allowed_callers={})",
                    self.connector_name,
                    allowed_callers
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(",")
                )));
            }
        }

        let requested_channel = command
            .payload
            .get("channel_id")
            .and_then(Value::as_str)
            .map(std::string::ToString::to_string);

        let chosen_channel = if let Some(channel_id) = requested_channel {
            let channel = catalog.channel(&channel_id).ok_or_else(|| {
                ConnectorError::Execution(format!("channel {channel_id} not found"))
            })?;
            if !channel.enabled {
                return Err(ConnectorError::Execution(format!(
                    "channel {channel_id} is disabled"
                )));
            }
            if channel.provider_id != provider.provider_id {
                return Err(ConnectorError::Execution(format!(
                    "channel {} does not belong to provider {}",
                    channel.channel_id, provider.provider_id
                )));
            }
            channel.clone()
        } else {
            catalog
                .channels_for_provider(&provider.provider_id)
                .into_iter()
                .find(|channel| channel.enabled)
                .ok_or_else(|| {
                    ConnectorError::Execution(format!(
                        "no enabled channel for provider {}",
                        provider.provider_id
                    ))
                })?
        };

        let operation = command.operation.clone();
        let payload = command.payload.clone();
        let bridge_execution = bridge_execution_payload(
            provider,
            &chosen_channel,
            &command,
            &self.bridge_runtime_policy,
        );

        if self.bridge_runtime_policy.enforce_execution_success
            && bridge_execution
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|status| matches!(status, "blocked" | "failed"))
        {
            let reason = bridge_execution
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("bridge execution failed under strict runtime policy");
            return Err(ConnectorError::Execution(reason.to_owned()));
        }

        Ok(ConnectorOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "connector": self.connector_name,
                "provider_id": provider.provider_id,
                "provider_version": provider.version,
                "channel_id": chosen_channel.channel_id,
                "endpoint": chosen_channel.endpoint,
                "operation": operation,
                "payload": payload,
                "bridge_execution": bridge_execution,
            }),
        })
    }
}

fn bridge_execution_payload(
    provider: &kernel::ProviderConfig,
    channel: &kernel::ChannelConfig,
    command: &ConnectorCommand,
    runtime_policy: &BridgeRuntimePolicy,
) -> Value {
    let bridge_kind = detect_provider_bridge_kind(provider, &channel.endpoint);
    let adapter_family = provider
        .metadata
        .get("adapter_family")
        .cloned()
        .unwrap_or_else(|| default_bridge_adapter_family(bridge_kind));
    let entrypoint = provider
        .metadata
        .get("entrypoint")
        .or_else(|| provider.metadata.get("entrypoint_hint"))
        .cloned()
        .unwrap_or_else(|| default_bridge_entrypoint(bridge_kind, &channel.endpoint));

    let plan = match bridge_kind {
        PluginBridgeKind::HttpJson => {
            let method = provider
                .metadata
                .get("http_method")
                .map(|value| value.to_ascii_uppercase())
                .unwrap_or_else(|| "POST".to_owned());
            json!({
                "status": "planned",
                "bridge_kind": bridge_kind.as_str(),
                "adapter_family": adapter_family,
                "entrypoint": entrypoint,
                "request": {
                    "method": method,
                    "url": channel.endpoint,
                    "operation": command.operation,
                    "payload": command.payload.clone(),
                }
            })
        }
        PluginBridgeKind::ProcessStdio => json!({
            "status": "planned",
            "bridge_kind": bridge_kind.as_str(),
            "adapter_family": adapter_family,
            "entrypoint": entrypoint,
            "stdio": {
                "stdin_envelope": {
                    "operation": command.operation,
                    "payload": command.payload.clone(),
                },
                "stdout_contract": "json",
            }
        }),
        PluginBridgeKind::NativeFfi => json!({
            "status": "planned",
            "bridge_kind": bridge_kind.as_str(),
            "adapter_family": adapter_family,
            "entrypoint": entrypoint,
            "ffi": {
                "library": provider
                    .metadata
                    .get("library")
                    .cloned()
                    .unwrap_or_else(|| format!("lib{}.so", provider.provider_id)),
                "symbol": entrypoint,
            }
        }),
        PluginBridgeKind::WasmComponent => json!({
            "status": "planned",
            "bridge_kind": bridge_kind.as_str(),
            "adapter_family": adapter_family,
            "entrypoint": entrypoint,
            "wasm": {
                "component": provider
                    .metadata
                    .get("component")
                    .cloned()
                    .unwrap_or_else(|| format!("{}.wasm", provider.provider_id)),
                "function": entrypoint,
            }
        }),
        PluginBridgeKind::McpServer => json!({
            "status": "planned",
            "bridge_kind": bridge_kind.as_str(),
            "adapter_family": adapter_family,
            "entrypoint": entrypoint,
            "mcp": {
                "transport": provider
                    .metadata
                    .get("transport")
                    .cloned()
                    .unwrap_or_else(|| "stdio".to_owned()),
                "handshake": "capability_schema_exchange",
            }
        }),
        PluginBridgeKind::Unknown => json!({
            "status": "deferred",
            "bridge_kind": bridge_kind.as_str(),
            "reason": "provider metadata does not declare a resolvable bridge_kind",
            "next_action": "set metadata.bridge_kind and rerun bootstrap",
        }),
    };

    maybe_execute_bridge(
        plan,
        bridge_kind,
        provider,
        channel,
        command,
        runtime_policy,
    )
}

fn maybe_execute_bridge(
    execution: Value,
    bridge_kind: PluginBridgeKind,
    provider: &kernel::ProviderConfig,
    channel: &kernel::ChannelConfig,
    command: &ConnectorCommand,
    runtime_policy: &BridgeRuntimePolicy,
) -> Value {
    if runtime_policy.execute_http_json && matches!(bridge_kind, PluginBridgeKind::HttpJson) {
        return execute_http_json_bridge(execution, provider, channel, command);
    }

    if runtime_policy.execute_process_stdio && matches!(bridge_kind, PluginBridgeKind::ProcessStdio)
    {
        return execute_process_stdio_bridge(execution, provider, channel, command, runtime_policy);
    }

    if runtime_policy.execute_wasm_component
        && matches!(bridge_kind, PluginBridgeKind::WasmComponent)
    {
        return execute_wasm_component_bridge(
            execution,
            provider,
            channel,
            command,
            runtime_policy,
        );
    }

    execution
}

fn execute_http_json_bridge(
    mut execution: Value,
    provider: &kernel::ProviderConfig,
    channel: &kernel::ChannelConfig,
    command: &ConnectorCommand,
) -> Value {
    let method_label = provider
        .metadata
        .get("http_method")
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "POST".to_owned());
    let method = match Method::from_bytes(method_label.as_bytes()) {
        Ok(method) => method,
        Err(error) => {
            execution["status"] = Value::String("blocked".to_owned());
            execution["reason"] =
                Value::String(format!("invalid http_method {method_label}: {error}"));
            return execution;
        }
    };

    let timeout_ms = provider
        .metadata
        .get("http_timeout_ms")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(8_000);

    let request_payload = json!({
        "provider_id": provider.provider_id,
        "channel_id": channel.channel_id,
        "operation": command.operation,
        "payload": command.payload,
    });
    let url = channel.endpoint.clone();
    let request_payload_for_runtime = request_payload.clone();
    let request_payload_for_worker = request_payload.clone();

    let run = std::thread::spawn(move || -> Result<(u16, bool, String, Value), String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .build()
            .map_err(|error| format!("failed to initialize http_json client: {error}"))?;

        let response = client
            .request(method, &url)
            .header("content-type", "application/json")
            .json(&request_payload_for_worker)
            .send()
            .map_err(|error| format!("http_json bridge request failed: {error}"))?;

        let status = response.status();
        let status_code = status.as_u16();
        let success = status.is_success();
        let body = response
            .text()
            .map_err(|error| format!("failed to read http_json response body: {error}"))?;
        let body_json = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
        Ok((status_code, success, body, body_json))
    })
    .join();

    match run {
        Ok(Ok((status_code, success, body, body_json))) => {
            execution["status"] = Value::String(if success {
                "executed".to_owned()
            } else {
                "failed".to_owned()
            });
            if !success {
                execution["reason"] = Value::String(format!(
                    "http_json bridge request failed with status {status_code}"
                ));
            }
            execution["runtime"] = json!({
                "executor": "http_json_reqwest",
                "method": method_label,
                "url": channel.endpoint,
                "status_code": status_code,
                "request": request_payload_for_runtime,
                "response_text": body,
                "response_json": body_json,
                "timeout_ms": timeout_ms,
            });
            execution
        }
        Ok(Err(reason)) => {
            execution["status"] = Value::String("failed".to_owned());
            execution["reason"] = Value::String(reason);
            execution["runtime"] = json!({
                "executor": "http_json_reqwest",
                "method": method_label,
                "url": channel.endpoint,
                "request": request_payload_for_runtime,
                "timeout_ms": timeout_ms,
            });
            execution
        }
        Err(_) => {
            execution["status"] = Value::String("failed".to_owned());
            execution["reason"] =
                Value::String("http_json bridge worker thread panicked".to_owned());
            execution["runtime"] = json!({
                "executor": "http_json_reqwest",
                "method": method_label,
                "url": channel.endpoint,
                "request": request_payload_for_runtime,
                "timeout_ms": timeout_ms,
            });
            execution
        }
    }
}

fn execute_process_stdio_bridge(
    mut execution: Value,
    provider: &kernel::ProviderConfig,
    channel: &kernel::ChannelConfig,
    command: &ConnectorCommand,
    runtime_policy: &BridgeRuntimePolicy,
) -> Value {
    let Some(program) = provider.metadata.get("command").cloned() else {
        execution["status"] = Value::String("blocked".to_owned());
        execution["reason"] =
            Value::String("process_stdio execution requires provider metadata.command".to_owned());
        return execution;
    };

    if !is_process_command_allowed(&program, &runtime_policy.allowed_process_commands) {
        execution["status"] = Value::String("blocked".to_owned());
        execution["reason"] = Value::String(format!(
            "process command {program} is not allowed by runtime policy"
        ));
        return execution;
    }

    let args = parse_process_args(provider);
    let envelope = json!({
        "provider_id": provider.provider_id,
        "channel_id": channel.channel_id,
        "operation": command.operation,
        "payload": command.payload,
    });
    let stdin_payload = match serde_json::to_vec(&envelope) {
        Ok(mut bytes) => {
            bytes.push(b'\n');
            bytes
        }
        Err(error) => {
            execution["status"] = Value::String("failed".to_owned());
            execution["reason"] =
                Value::String(format!("failed to encode process envelope: {error}"));
            return execution;
        }
    };

    let output = (|| -> Result<std::process::Output, String> {
        let mut child = Command::new(&program)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to spawn process command {program}: {error}"))?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(&stdin_payload)
                .map_err(|error| format!("failed to write process stdin payload: {error}"))?;
        }

        child
            .wait_with_output()
            .map_err(|error| format!("failed to wait process output: {error}"))
    })();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let stdout_json = serde_json::from_str::<Value>(&stdout).unwrap_or(Value::Null);
            let success = output.status.success();

            execution["status"] = Value::String(if success {
                "executed".to_owned()
            } else {
                "failed".to_owned()
            });
            if !success {
                execution["reason"] = Value::String(format!(
                    "process command exited with code {:?}",
                    output.status.code()
                ));
            }
            execution["runtime"] = json!({
                "executor": "process_stdio_local",
                "command": program,
                "args": args,
                "exit_code": output.status.code(),
                "stdout": stdout,
                "stderr": stderr,
                "stdout_json": stdout_json,
            });
            execution
        }
        Err(reason) => {
            execution["status"] = Value::String("failed".to_owned());
            execution["reason"] = Value::String(reason);
            execution
        }
    }
}

fn execute_wasm_component_bridge(
    mut execution: Value,
    provider: &kernel::ProviderConfig,
    channel: &kernel::ChannelConfig,
    command: &ConnectorCommand,
    runtime_policy: &BridgeRuntimePolicy,
) -> Value {
    let artifact_path = match resolve_wasm_component_artifact_path(provider, &channel.endpoint) {
        Ok(path) => path,
        Err(reason) => {
            execution["status"] = Value::String("blocked".to_owned());
            execution["reason"] = Value::String(reason);
            return execution;
        }
    };

    if !runtime_policy.wasm_allowed_path_prefixes.is_empty()
        && !runtime_policy
            .wasm_allowed_path_prefixes
            .iter()
            .any(|prefix| artifact_path.starts_with(prefix))
    {
        execution["status"] = Value::String("blocked".to_owned());
        execution["reason"] =
            Value::String("wasm artifact path is outside runtime allowed_path_prefixes".to_owned());
        execution["runtime"] = json!({
            "executor": "wasmtime_module",
            "artifact_path": artifact_path.display().to_string(),
            "allowed_path_prefixes": runtime_policy
                .wasm_allowed_path_prefixes
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>(),
        });
        return execution;
    }

    let module_bytes = match fs::read(&artifact_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            execution["status"] = Value::String("failed".to_owned());
            execution["reason"] = Value::String(format!("failed to read wasm artifact: {error}"));
            execution["runtime"] = json!({
                "executor": "wasmtime_module",
                "artifact_path": artifact_path.display().to_string(),
            });
            return execution;
        }
    };

    if let Some(limit) = runtime_policy.wasm_max_component_bytes {
        if module_bytes.len() > limit {
            execution["status"] = Value::String("blocked".to_owned());
            execution["reason"] = Value::String(format!(
                "wasm artifact size {} exceeds runtime max_component_bytes {limit}",
                module_bytes.len()
            ));
            execution["runtime"] = json!({
                "executor": "wasmtime_module",
                "artifact_path": artifact_path.display().to_string(),
                "module_size_bytes": module_bytes.len(),
                "max_component_bytes": limit,
            });
            return execution;
        }
    }

    let export_name = resolve_wasm_export_name(provider);

    let run_result = (|| -> Result<Option<u64>, String> {
        let mut config = WasmtimeConfig::new();
        if runtime_policy.wasm_fuel_limit.is_some() {
            config.consume_fuel(true);
        }
        let engine = WasmtimeEngine::new(&config)
            .map_err(|error| format!("failed to initialize wasmtime engine: {error}"))?;
        let module = WasmtimeModule::new(&engine, &module_bytes)
            .map_err(|error| format!("failed to compile wasm module: {error}"))?;
        let mut store = WasmtimeStore::new(&engine, ());
        if let Some(limit) = runtime_policy.wasm_fuel_limit {
            store
                .set_fuel(limit)
                .map_err(|error| format!("failed to set wasm fuel limit: {error}"))?;
        }
        let linker = WasmtimeLinker::new(&engine);
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|error| format!("failed to instantiate wasm module: {error}"))?;
        let func = instance
            .get_typed_func::<(), ()>(&mut store, export_name.as_str())
            .map_err(|error| {
                format!("failed to resolve exported wasm function {export_name}: {error}")
            })?;
        func.call(&mut store, ())
            .map_err(|error| format!("wasm function call failed: {error}"))?;
        let consumed_fuel = runtime_policy
            .wasm_fuel_limit
            .map(|limit| {
                store
                    .get_fuel()
                    .map(|remaining| limit.saturating_sub(remaining))
            })
            .transpose()
            .map_err(|error| format!("failed to query wasm fuel: {error}"))?;
        Ok(consumed_fuel)
    })();

    match run_result {
        Ok(consumed_fuel) => {
            execution["status"] = Value::String("executed".to_owned());
            execution["runtime"] = json!({
                "executor": "wasmtime_module",
                "artifact_path": artifact_path.display().to_string(),
                "export": export_name,
                "operation": command.operation,
                "payload": command.payload,
                "module_size_bytes": module_bytes.len(),
                "fuel_limit": runtime_policy.wasm_fuel_limit,
                "fuel_consumed": consumed_fuel,
            });
            execution
        }
        Err(reason) => {
            execution["status"] = Value::String("failed".to_owned());
            execution["reason"] = Value::String(reason);
            execution["runtime"] = json!({
                "executor": "wasmtime_module",
                "artifact_path": artifact_path.display().to_string(),
                "export": export_name,
            });
            execution
        }
    }
}

fn resolve_wasm_component_artifact_path(
    provider: &kernel::ProviderConfig,
    channel_endpoint: &str,
) -> Result<PathBuf, String> {
    let raw = provider
        .metadata
        .get("component_resolved_path")
        .cloned()
        .or_else(|| provider.metadata.get("component_path").cloned())
        .or_else(|| provider.metadata.get("component").cloned())
        .or_else(|| {
            let endpoint = channel_endpoint.trim();
            endpoint
                .to_ascii_lowercase()
                .ends_with(".wasm")
                .then(|| endpoint.to_owned())
        })
        .ok_or_else(|| "wasm_component execution requires component artifact path".to_owned())?;

    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Err(
            "wasm_component execution requires a local artifact path, remote URL is not allowed"
                .to_owned(),
        );
    }

    let candidate = PathBuf::from(&raw);
    let resolved = if candidate.is_absolute() {
        candidate
    } else if let Some(source_path) = provider.metadata.get("plugin_source_path") {
        resolve_plugin_relative_path(source_path, &raw)
    } else {
        candidate
    };

    Ok(normalize_path_for_policy(&resolved))
}

fn resolve_wasm_export_name(provider: &kernel::ProviderConfig) -> String {
    let raw = provider
        .metadata
        .get("entrypoint")
        .or_else(|| provider.metadata.get("entrypoint_hint"))
        .cloned()
        .unwrap_or_else(|| "run".to_owned());
    raw.split([':', '/'])
        .rfind(|segment| !segment.trim().is_empty())
        .unwrap_or("run")
        .to_owned()
}

fn parse_process_args(provider: &kernel::ProviderConfig) -> Vec<String> {
    if let Some(args_json) = provider.metadata.get("args_json") {
        if let Ok(args) = serde_json::from_str::<Vec<String>>(args_json) {
            return args;
        }
    }

    provider
        .metadata
        .get("args")
        .map(|value| value.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
}

fn provider_allowed_callers(provider: &kernel::ProviderConfig) -> BTreeSet<String> {
    let mut allowed = BTreeSet::new();

    if let Some(raw_json) = provider.metadata.get("allowed_callers_json") {
        if let Ok(values) = serde_json::from_str::<Vec<String>>(raw_json) {
            for value in values {
                let normalized = value.trim().to_ascii_lowercase();
                if !normalized.is_empty() {
                    allowed.insert(normalized);
                }
            }
        }
    }

    if let Some(raw_list) = provider.metadata.get("allowed_callers") {
        for token in raw_list.split([',', ';', ' ']) {
            let normalized = token.trim().to_ascii_lowercase();
            if !normalized.is_empty() {
                allowed.insert(normalized);
            }
        }
    }

    allowed
}

fn caller_from_payload(payload: &Value) -> Option<String> {
    payload
        .get("_loongclaw")
        .and_then(Value::as_object)
        .and_then(|meta| meta.get("caller"))
        .and_then(Value::as_str)
        .map(|caller| caller.trim().to_ascii_lowercase())
        .filter(|caller| !caller.is_empty())
}

fn caller_is_allowed(caller: Option<&str>, allowed: &BTreeSet<String>) -> bool {
    if allowed.is_empty() {
        return true;
    }
    if allowed.contains("*") {
        return true;
    }
    caller
        .map(|value| value.trim().to_ascii_lowercase())
        .is_some_and(|value| allowed.contains(&value))
}

fn is_process_command_allowed(program: &str, allowed: &BTreeSet<String>) -> bool {
    if allowed.is_empty() {
        return false;
    }

    let normalized = program.trim().to_ascii_lowercase();
    if allowed.contains(&normalized) {
        return true;
    }

    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| allowed.contains(&name.to_ascii_lowercase()))
        .unwrap_or(false)
}

fn detect_provider_bridge_kind(
    provider: &kernel::ProviderConfig,
    endpoint: &str,
) -> PluginBridgeKind {
    if let Some(raw) = provider.metadata.get("bridge_kind") {
        if let Some(kind) = parse_bridge_kind_label(raw) {
            return kind;
        }
    }

    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return PluginBridgeKind::HttpJson;
    }

    PluginBridgeKind::Unknown
}

fn parse_bridge_kind_label(raw: &str) -> Option<PluginBridgeKind> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "http_json" | "http" => Some(PluginBridgeKind::HttpJson),
        "process_stdio" | "stdio" => Some(PluginBridgeKind::ProcessStdio),
        "native_ffi" | "ffi" => Some(PluginBridgeKind::NativeFfi),
        "wasm_component" | "wasm" => Some(PluginBridgeKind::WasmComponent),
        "mcp_server" | "mcp" => Some(PluginBridgeKind::McpServer),
        "unknown" => Some(PluginBridgeKind::Unknown),
        _ => None,
    }
}

fn default_bridge_adapter_family(bridge_kind: PluginBridgeKind) -> String {
    match bridge_kind {
        PluginBridgeKind::HttpJson => "http-adapter".to_owned(),
        PluginBridgeKind::ProcessStdio => "stdio-adapter".to_owned(),
        PluginBridgeKind::NativeFfi => "ffi-adapter".to_owned(),
        PluginBridgeKind::WasmComponent => "wasm-component-adapter".to_owned(),
        PluginBridgeKind::McpServer => "mcp-adapter".to_owned(),
        PluginBridgeKind::Unknown => "unknown-adapter".to_owned(),
    }
}

fn default_bridge_entrypoint(bridge_kind: PluginBridgeKind, endpoint: &str) -> String {
    match bridge_kind {
        PluginBridgeKind::HttpJson => endpoint.to_owned(),
        PluginBridgeKind::ProcessStdio => "stdin/stdout::invoke".to_owned(),
        PluginBridgeKind::NativeFfi => "lib::invoke".to_owned(),
        PluginBridgeKind::WasmComponent => "component::run".to_owned(),
        PluginBridgeKind::McpServer => "mcp::stdio".to_owned(),
        PluginBridgeKind::Unknown => "unknown::invoke".to_owned(),
    }
}

struct CrmCoreConnector;

#[async_trait]
impl CoreConnectorAdapter for CrmCoreConnector {
    fn name(&self) -> &str {
        "http-core"
    }

    async fn invoke_core(
        &self,
        command: ConnectorCommand,
    ) -> Result<ConnectorOutcome, ConnectorError> {
        Ok(ConnectorOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "tier": "core",
                "adapter": "http-core",
                "connector": command.connector_name,
                "operation": command.operation,
                "payload": command.payload,
            }),
        })
    }
}

struct CrmGrpcCoreConnector;

#[async_trait]
impl CoreConnectorAdapter for CrmGrpcCoreConnector {
    fn name(&self) -> &str {
        "grpc-core"
    }

    async fn invoke_core(
        &self,
        command: ConnectorCommand,
    ) -> Result<ConnectorOutcome, ConnectorError> {
        Ok(ConnectorOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "tier": "core",
                "adapter": "grpc-core",
                "connector": command.connector_name,
                "operation": command.operation,
                "payload": command.payload,
            }),
        })
    }
}

struct ShieldedConnectorExtension;

#[async_trait]
impl kernel::ConnectorExtensionAdapter for ShieldedConnectorExtension {
    fn name(&self) -> &str {
        "shielded-bridge"
    }

    async fn invoke_extension(
        &self,
        command: ConnectorCommand,
        core: &(dyn CoreConnectorAdapter + Sync),
    ) -> Result<ConnectorOutcome, ConnectorError> {
        let probe = core
            .invoke_core(ConnectorCommand {
                connector_name: command.connector_name.clone(),
                operation: "probe".to_owned(),
                required_capabilities: BTreeSet::new(),
                payload: json!({}),
            })
            .await?;
        Ok(ConnectorOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "tier": "extension",
                "extension": "shielded-bridge",
                "operation": command.operation,
                "core_probe": probe.payload,
                "payload": command.payload,
            }),
        })
    }
}

struct NativeCoreRuntime;

#[async_trait]
impl CoreRuntimeAdapter for NativeCoreRuntime {
    fn name(&self) -> &str {
        "native-core"
    }

    async fn execute_core(
        &self,
        request: RuntimeCoreRequest,
    ) -> Result<RuntimeCoreOutcome, kernel::RuntimePlaneError> {
        Ok(RuntimeCoreOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "adapter": "native-core",
                "action": request.action,
                "payload": request.payload,
            }),
        })
    }
}

struct FallbackCoreRuntime;

#[async_trait]
impl CoreRuntimeAdapter for FallbackCoreRuntime {
    fn name(&self) -> &str {
        "fallback-core"
    }

    async fn execute_core(
        &self,
        request: RuntimeCoreRequest,
    ) -> Result<RuntimeCoreOutcome, kernel::RuntimePlaneError> {
        Ok(RuntimeCoreOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "adapter": "fallback-core",
                "action": request.action,
                "payload": request.payload,
            }),
        })
    }
}

struct AcpBridgeRuntimeExtension;

#[async_trait]
impl RuntimeExtensionAdapter for AcpBridgeRuntimeExtension {
    fn name(&self) -> &str {
        "acp-bridge"
    }

    async fn execute_extension(
        &self,
        request: RuntimeExtensionRequest,
        core: &(dyn CoreRuntimeAdapter + Sync),
    ) -> Result<RuntimeExtensionOutcome, kernel::RuntimePlaneError> {
        let core_probe = core
            .execute_core(RuntimeCoreRequest {
                action: "probe".to_owned(),
                payload: json!({}),
            })
            .await?;
        Ok(RuntimeExtensionOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "extension": "acp-bridge",
                "action": request.action,
                "core_probe": core_probe.payload,
                "payload": request.payload,
            }),
        })
    }
}

struct CoreToolRuntime;

#[async_trait]
impl CoreToolAdapter for CoreToolRuntime {
    fn name(&self) -> &str {
        "core-tools"
    }

    async fn execute_core_tool(
        &self,
        request: ToolCoreRequest,
    ) -> Result<ToolCoreOutcome, kernel::ToolPlaneError> {
        mvp::tools::execute_tool_core(request).map_err(kernel::ToolPlaneError::Execution)
    }
}

struct SqlAnalyticsToolExtension;

#[async_trait]
impl ToolExtensionAdapter for SqlAnalyticsToolExtension {
    fn name(&self) -> &str {
        "sql-analytics"
    }

    async fn execute_tool_extension(
        &self,
        request: ToolExtensionRequest,
        core: &(dyn CoreToolAdapter + Sync),
    ) -> Result<ToolExtensionOutcome, kernel::ToolPlaneError> {
        let core_probe = core
            .execute_core_tool(ToolCoreRequest {
                tool_name: "schema_probe".to_owned(),
                payload: json!({}),
            })
            .await?;
        Ok(ToolExtensionOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "extension": "sql-analytics",
                "action": request.extension_action,
                "core_probe": core_probe.payload,
                "payload": request.payload,
            }),
        })
    }
}

struct KvCoreMemory;

#[async_trait]
impl CoreMemoryAdapter for KvCoreMemory {
    fn name(&self) -> &str {
        "kv-core"
    }

    async fn execute_core_memory(
        &self,
        request: MemoryCoreRequest,
    ) -> Result<MemoryCoreOutcome, kernel::MemoryPlaneError> {
        mvp::memory::execute_memory_core(request).map_err(kernel::MemoryPlaneError::Execution)
    }
}

struct VectorIndexMemoryExtension;

#[async_trait]
impl MemoryExtensionAdapter for VectorIndexMemoryExtension {
    fn name(&self) -> &str {
        "vector-index"
    }

    async fn execute_memory_extension(
        &self,
        request: MemoryExtensionRequest,
        core: &(dyn CoreMemoryAdapter + Sync),
    ) -> Result<MemoryExtensionOutcome, kernel::MemoryPlaneError> {
        let core_probe = core
            .execute_core_memory(MemoryCoreRequest {
                operation: "probe".to_owned(),
                payload: json!({}),
            })
            .await?;
        Ok(MemoryExtensionOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "extension": "vector-index",
                "operation": request.operation,
                "core_probe": core_probe.payload,
                "payload": request.payload,
            }),
        })
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let result = match cli.command.unwrap_or(Commands::Demo) {
        Commands::Demo => run_demo().await,
        Commands::RunTask { objective, payload } => run_task_cli(&objective, &payload).await,
        Commands::InvokeConnector { operation, payload } => {
            invoke_connector_cli(&operation, &payload).await
        }
        Commands::AuditDemo => run_audit_demo().await,
        Commands::InitSpec { output } => init_spec_cli(&output),
        Commands::RunSpec { spec, print_audit } => run_spec_cli(&spec, print_audit).await,
        Commands::BenchmarkProgrammaticPressure {
            matrix,
            baseline,
            output,
            enforce_gate,
        } => {
            run_programmatic_pressure_benchmark_cli(
                &matrix,
                baseline.as_deref(),
                &output,
                enforce_gate,
            )
            .await
        }
        Commands::Setup { output, force } => run_setup_cli(output.as_deref(), force),
        Commands::Chat { config, session } => {
            run_chat_cli(config.as_deref(), session.as_deref()).await
        }
        Commands::TelegramServe { config, once } => {
            run_telegram_serve_cli(config.as_deref(), once).await
        }
        Commands::FeishuSend {
            config,
            receive_id,
            text,
            card,
        } => run_feishu_send_cli(config.as_deref(), &receive_id, &text, card).await,
        Commands::FeishuServe { config, bind, path } => {
            run_feishu_serve_cli(config.as_deref(), bind.as_deref(), path.as_deref()).await
        }
    };
    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(2);
    }
}

async fn run_demo() -> CliResult<()> {
    let kernel = bootstrap_kernel_default();
    let token = kernel
        .issue_token(DEFAULT_PACK_ID, DEFAULT_AGENT_ID, 300)
        .map_err(|error| format!("token issue failed: {error}"))?;

    let task = TaskIntent {
        task_id: "task-bootstrap-01".to_owned(),
        objective: "summarize flaky test clusters".to_owned(),
        required_capabilities: BTreeSet::from([Capability::InvokeTool, Capability::MemoryRead]),
        payload: json!({"repo": "chumyin/LoongClaw"}),
    };

    let task_dispatch = kernel
        .execute_task(DEFAULT_PACK_ID, &token, task)
        .await
        .map_err(|error| format!("task dispatch failed: {error}"))?;

    println!(
        "task dispatched via {:?}: {}",
        task_dispatch.adapter_route.harness_kind, task_dispatch.outcome.output
    );

    let connector_dispatch = kernel
        .invoke_connector(
            DEFAULT_PACK_ID,
            &token,
            ConnectorCommand {
                connector_name: "webhook".to_owned(),
                operation: "notify".to_owned(),
                required_capabilities: BTreeSet::from([Capability::InvokeConnector]),
                payload: json!({"channel": "ops-alerts", "message": "task complete"}),
            },
        )
        .await
        .map_err(|error| format!("connector dispatch failed: {error}"))?;

    println!("connector dispatch: {}", connector_dispatch.outcome.payload);
    Ok(())
}

async fn run_task_cli(objective: &str, payload_raw: &str) -> CliResult<()> {
    let payload = parse_json_payload(payload_raw, "run-task payload")?;

    let kernel = bootstrap_kernel_default();
    let token = kernel
        .issue_token(DEFAULT_PACK_ID, DEFAULT_AGENT_ID, 120)
        .map_err(|error| format!("token issue failed: {error}"))?;

    let dispatch = kernel
        .execute_task(
            DEFAULT_PACK_ID,
            &token,
            TaskIntent {
                task_id: "task-cli-01".to_owned(),
                objective: objective.to_owned(),
                required_capabilities: BTreeSet::from([
                    Capability::InvokeTool,
                    Capability::MemoryRead,
                ]),
                payload,
            },
        )
        .await
        .map_err(|error| format!("task dispatch failed: {error}"))?;

    let pretty = serde_json::to_string_pretty(&dispatch.outcome)
        .map_err(|error| format!("serialize task outcome failed: {error}"))?;
    println!("{pretty}");
    Ok(())
}

async fn invoke_connector_cli(operation: &str, payload_raw: &str) -> CliResult<()> {
    let payload = parse_json_payload(payload_raw, "invoke-connector payload")?;

    let kernel = bootstrap_kernel_default();
    let token = kernel
        .issue_token(DEFAULT_PACK_ID, DEFAULT_AGENT_ID, 120)
        .map_err(|error| format!("token issue failed: {error}"))?;

    let dispatch = kernel
        .invoke_connector(
            DEFAULT_PACK_ID,
            &token,
            ConnectorCommand {
                connector_name: "webhook".to_owned(),
                operation: operation.to_owned(),
                required_capabilities: BTreeSet::from([Capability::InvokeConnector]),
                payload,
            },
        )
        .await
        .map_err(|error| format!("connector dispatch failed: {error}"))?;

    let pretty = serde_json::to_string_pretty(&dispatch.outcome)
        .map_err(|error| format!("serialize connector outcome failed: {error}"))?;
    println!("{pretty}");
    Ok(())
}

async fn run_audit_demo() -> CliResult<()> {
    let fixed_clock = Arc::new(FixedClock::new(1_700_000_000));
    let audit_sink = Arc::new(InMemoryAuditSink::default());

    let kernel = bootstrap_kernel_with_runtime(fixed_clock.clone(), audit_sink.clone());

    let token = kernel
        .issue_token(DEFAULT_PACK_ID, DEFAULT_AGENT_ID, 30)
        .map_err(|error| format!("token issue failed: {error}"))?;

    let _ = kernel
        .execute_task(
            DEFAULT_PACK_ID,
            &token,
            TaskIntent {
                task_id: "task-audit-01".to_owned(),
                objective: "produce audit evidence".to_owned(),
                required_capabilities: BTreeSet::from([Capability::InvokeTool]),
                payload: json!({}),
            },
        )
        .await
        .map_err(|error| format!("task dispatch failed: {error}"))?;

    fixed_clock.advance_by(5);

    let _ = kernel
        .invoke_connector(
            DEFAULT_PACK_ID,
            &token,
            ConnectorCommand {
                connector_name: "webhook".to_owned(),
                operation: "notify".to_owned(),
                required_capabilities: BTreeSet::from([Capability::InvokeConnector]),
                payload: json!({"channel": "audit"}),
            },
        )
        .await
        .map_err(|error| format!("connector invoke failed: {error}"))?;

    kernel
        .revoke_token(&token.token_id, Some(DEFAULT_AGENT_ID))
        .map_err(|error| format!("token revoke failed: {error}"))?;

    let pretty = serde_json::to_string_pretty(&audit_sink.snapshot())
        .map_err(|error| format!("serialize audit events failed: {error}"))?;
    println!("{pretty}");
    Ok(())
}

fn init_spec_cli(output_path: &str) -> CliResult<()> {
    let spec = RunnerSpec::template();
    write_json_file(output_path, &spec)?;
    println!("spec template written to {}", output_path);
    Ok(())
}

async fn run_spec_cli(spec_path: &str, print_audit: bool) -> CliResult<()> {
    let spec = read_spec_file(spec_path)?;
    let report = execute_spec(spec, print_audit).await;
    let pretty = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("serialize spec run report failed: {error}"))?;
    println!("{pretty}");
    Ok(())
}

fn run_setup_cli(output: Option<&str>, force: bool) -> CliResult<()> {
    let path = mvp::config::write_template(output, force)?;
    #[cfg(feature = "memory-sqlite")]
    {
        let path_str = path
            .to_str()
            .ok_or_else(|| format!("config path is not valid UTF-8: {}", path.display()))?;
        let (_, parsed) = mvp::config::load(Some(path_str))?;
        let memory_db =
            mvp::memory::ensure_memory_db_ready(Some(parsed.memory.resolved_sqlite_path()))
                .map_err(|error| format!("failed to bootstrap sqlite memory: {error}"))?;
        println!(
            "setup complete\n- config: {}\n- sqlite memory: {}",
            path.display(),
            memory_db.display()
        );
    }
    #[cfg(not(feature = "memory-sqlite"))]
    {
        println!("setup complete\n- config: {}", path.display());
    }
    println!("next step: loongclawd chat --config {}", path.display());
    Ok(())
}

async fn run_chat_cli(config_path: Option<&str>, session: Option<&str>) -> CliResult<()> {
    mvp::chat::run_cli_chat(config_path, session).await
}

async fn run_telegram_serve_cli(config_path: Option<&str>, once: bool) -> CliResult<()> {
    mvp::channel::run_telegram_channel(config_path, once).await
}

async fn run_feishu_send_cli(
    config_path: Option<&str>,
    receive_id: &str,
    text: &str,
    as_card: bool,
) -> CliResult<()> {
    mvp::channel::run_feishu_send(config_path, receive_id, text, as_card).await
}

async fn run_feishu_serve_cli(
    config_path: Option<&str>,
    bind_override: Option<&str>,
    path_override: Option<&str>,
) -> CliResult<()> {
    mvp::channel::run_feishu_channel(config_path, bind_override, path_override).await
}

async fn execute_spec(spec: RunnerSpec, include_audit: bool) -> SpecRunReport {
    let mut spec = spec;
    let audit_sink = Arc::new(InMemoryAuditSink::default());
    let mut kernel =
        bootstrap_kernel_with_runtime(Arc::new(SystemClock) as Arc<dyn Clock>, audit_sink.clone());

    let mut integration_catalog = default_integration_catalog();
    let mut blocked_reason = None;
    let mut bridge_support_checksum = None;
    let mut bridge_support_sha256 = None;
    let approval_guard = evaluate_approval_guard(&spec);
    let mut self_awareness = None;
    let mut architecture_guard = None;
    let mut plugin_scan_reports = Vec::new();
    let mut plugin_translation_reports = Vec::new();
    let mut plugin_activation_plans = Vec::new();
    let mut plugin_bootstrap_reports = Vec::new();
    let mut plugin_bootstrap_queue = Vec::new();
    let mut plugin_absorb_reports = Vec::new();
    let security_scan_policy = match security_scan_policy(&spec) {
        Ok(policy) => policy,
        Err(error) => {
            blocked_reason = Some(match blocked_reason {
                Some(existing) => format!("{existing}; {error}"),
                None => error,
            });
            None
        }
    };
    let security_process_allowlist = security_scan_process_allowlist(&spec);
    let mut security_scan_report = security_scan_policy
        .as_ref()
        .map(|_| SecurityScanReport::default());
    let mut auto_provision_plan = None;

    if !approval_guard.approved {
        blocked_reason = Some(approval_guard.reason.clone());
    }

    if let Some(bridge) = &spec.bridge_support {
        if bridge.enabled {
            let checksum = bridge_support_policy_checksum(bridge);
            let sha256 = bridge_support_policy_sha256(bridge);
            bridge_support_checksum = Some(checksum.clone());
            bridge_support_sha256 = Some(sha256.clone());

            let version = bridge.policy_version.as_deref().unwrap_or("unknown");
            let mut mismatch_reasons = Vec::new();
            if let Some(expected) = &bridge.expected_checksum {
                if !expected.eq_ignore_ascii_case(&checksum) {
                    mismatch_reasons.push(format!(
                        "bridge support policy checksum mismatch (version {version})"
                    ));
                }
            }
            if let Some(expected_sha256) = &bridge.expected_sha256 {
                if !expected_sha256.eq_ignore_ascii_case(&sha256) {
                    mismatch_reasons.push(format!(
                        "bridge support policy sha256 mismatch (version {version})"
                    ));
                }
            }
            if !mismatch_reasons.is_empty() {
                blocked_reason = Some(mismatch_reasons.join("; "));
            }
        }
    }

    if let Some(self_awareness_spec) = &spec.self_awareness {
        if self_awareness_spec.enabled {
            let mut architecture_policy = ArchitectureBoundaryPolicy::default();
            if !self_awareness_spec.immutable_core_paths.is_empty() {
                architecture_policy.immutable_prefixes = self_awareness_spec
                    .immutable_core_paths
                    .iter()
                    .cloned()
                    .collect();
            }
            if !self_awareness_spec.mutable_extension_paths.is_empty() {
                architecture_policy.mutable_prefixes = self_awareness_spec
                    .mutable_extension_paths
                    .iter()
                    .cloned()
                    .collect();
            }

            let engine = CodebaseAwarenessEngine::new();
            match engine.snapshot(&CodebaseAwarenessConfig {
                roots: self_awareness_spec.roots.clone(),
                plugin_roots: self_awareness_spec.plugin_roots.clone(),
                proposed_mutations: self_awareness_spec.proposed_mutations.clone(),
                architecture_policy,
            }) {
                Ok(snapshot) => {
                    architecture_guard = Some(snapshot.architecture_guard.clone());
                    if self_awareness_spec.enforce_guard
                        && snapshot.architecture_guard.has_denials()
                    {
                        blocked_reason = Some(
                            "architecture guard blocked proposed mutations outside mutable boundaries"
                                .to_owned(),
                        );
                    }
                    self_awareness = Some(snapshot);
                }
                Err(error) => {
                    blocked_reason = Some(format!("self-awareness snapshot failed: {error}"));
                }
            }
        }
    }

    if let Some(reason) = blocked_reason.clone() {
        return SpecRunReport {
            pack_id: spec.pack.pack_id,
            agent_id: spec.agent_id,
            operation_kind: "blocked",
            blocked_reason: Some(reason.clone()),
            approval_guard,
            bridge_support_checksum,
            bridge_support_sha256,
            self_awareness,
            architecture_guard,
            plugin_scan_reports,
            plugin_translation_reports,
            plugin_activation_plans,
            plugin_bootstrap_reports,
            plugin_bootstrap_queue,
            plugin_absorb_reports,
            security_scan_report: security_scan_report.clone(),
            auto_provision_plan,
            outcome: json!({
                "status": "blocked",
                "reason": reason,
            }),
            integration_catalog,
            audit_events: if include_audit {
                Some(audit_sink.snapshot())
            } else {
                None
            },
        };
    }

    if let Some(plugin_scan) = &spec.plugin_scan {
        if plugin_scan.enabled {
            let scanner = PluginScanner::new();
            let translator = PluginTranslator::new();
            let bootstrap_executor = PluginBootstrapExecutor::new();
            let bootstrap_policy = bootstrap_policy(&spec);
            let (bridge_matrix, enforce_bridge_support) = bridge_support_matrix(&spec);
            let mut pending_absorb_inputs = Vec::new();
            let mut remaining_bootstrap_budget =
                bootstrap_policy.as_ref().map(|policy| policy.max_tasks);
            for root in &plugin_scan.roots {
                let report = match scanner.scan_path(root) {
                    Ok(report) => report,
                    Err(error) => {
                        blocked_reason =
                            Some(format!("plugin scan failed for root {root}: {error}"));
                        break;
                    }
                };
                let translation = translator.translate_scan_report(&report);
                let activation = translator.plan_activation(&translation, &bridge_matrix);

                if enforce_bridge_support && activation.has_blockers() {
                    blocked_reason = Some(format!(
                        "bridge support enforcement blocked {} plugin(s)",
                        activation.blocked_plugins
                    ));
                }

                let ready_report = filter_scan_report_by_activation(&report, &activation);
                let mut filtered_report = ready_report.clone();
                if let Some(policy) = bootstrap_policy.as_ref() {
                    let mut effective_policy = policy.clone();
                    if let Some(remaining) = remaining_bootstrap_budget {
                        effective_policy.max_tasks = remaining;
                    }
                    let bootstrap_report =
                        bootstrap_executor.execute(&activation, &effective_policy);
                    if blocked_reason.is_none() && bootstrap_report.blocked {
                        blocked_reason =
                            Some(bootstrap_report.block_reason.clone().unwrap_or_else(|| {
                                "bootstrap policy blocked ready plugins".to_owned()
                            }));
                    }

                    if let Some(remaining) = remaining_bootstrap_budget.as_mut() {
                        *remaining = remaining.saturating_sub(bootstrap_report.applied_tasks);
                    }

                    plugin_bootstrap_queue.extend(
                        bootstrap_report
                            .tasks
                            .iter()
                            .filter(|task| matches!(task.status, BootstrapTaskStatus::Applied))
                            .map(|task| task.bootstrap_hint.clone()),
                    );
                    filtered_report =
                        filter_scan_report_by_keys(&report, &bootstrap_report.applied_plugin_keys);
                    plugin_bootstrap_reports.push(bootstrap_report);
                } else {
                    plugin_bootstrap_queue.extend(
                        activation
                            .candidates
                            .iter()
                            .filter(|candidate| {
                                matches!(candidate.status, PluginActivationStatus::Ready)
                            })
                            .map(|candidate| candidate.bootstrap_hint.clone()),
                    );
                }

                let enriched_ready_report =
                    enrich_scan_report_with_translation(&ready_report, &translation);
                let enriched_filtered_report =
                    enrich_scan_report_with_translation(&filtered_report, &translation);

                if let (Some(policy), Some(report)) =
                    (security_scan_policy.as_ref(), security_scan_report.as_mut())
                {
                    let delta = evaluate_plugin_security_scan(
                        &enriched_ready_report,
                        policy,
                        &security_process_allowlist,
                    );
                    apply_security_scan_delta(report, delta);

                    if blocked_reason.is_none() && report.blocked {
                        blocked_reason = report.block_reason.clone();
                    }
                }

                plugin_translation_reports.push(translation);
                plugin_activation_plans.push(activation);
                plugin_scan_reports.push(report);
                pending_absorb_inputs.push(enriched_filtered_report);

                if blocked_reason.is_some() {
                    break;
                }
            }

            if blocked_reason.is_none() {
                for pending in pending_absorb_inputs {
                    let absorb = scanner.absorb(&mut integration_catalog, &mut spec.pack, &pending);
                    plugin_absorb_reports.push(absorb);
                }
            }
        }
    }

    if let (Some(policy), Some(report)) =
        (security_scan_policy.as_ref(), security_scan_report.as_mut())
    {
        if let Some(export_spec) = policy.siem_export.as_ref().filter(|value| value.enabled) {
            match emit_security_scan_siem_record(
                &spec.pack.pack_id,
                &spec.agent_id,
                report,
                export_spec,
            ) {
                Ok(export_report) => report.siem_export = Some(export_report),
                Err(error) => {
                    report.siem_export = Some(SecuritySiemExportReport {
                        enabled: true,
                        path: export_spec.path.clone(),
                        success: false,
                        exported_records: 0,
                        exported_findings: 0,
                        truncated_findings: 0,
                        error: Some(error.clone()),
                    });
                    if export_spec.fail_on_error && blocked_reason.is_none() {
                        blocked_reason = Some(format!("security scan siem export failed: {error}"));
                    }
                }
            }
        }
    }

    if let Some(report) = security_scan_report.as_ref() {
        if let Err(error) =
            emit_security_scan_audit_event(&kernel, &spec.pack.pack_id, &spec.agent_id, report)
        {
            if blocked_reason.is_none() {
                blocked_reason = Some(error);
            }
        }
    }

    if let Some(reason) = blocked_reason.clone() {
        return SpecRunReport {
            pack_id: spec.pack.pack_id,
            agent_id: spec.agent_id,
            operation_kind: "blocked",
            blocked_reason: Some(reason.clone()),
            approval_guard,
            bridge_support_checksum,
            bridge_support_sha256,
            self_awareness,
            architecture_guard,
            plugin_scan_reports,
            plugin_translation_reports,
            plugin_activation_plans,
            plugin_bootstrap_reports,
            plugin_bootstrap_queue,
            plugin_absorb_reports,
            security_scan_report: security_scan_report.clone(),
            auto_provision_plan,
            outcome: json!({
                "status": "blocked",
                "reason": reason,
            }),
            integration_catalog,
            audit_events: if include_audit {
                Some(audit_sink.snapshot())
            } else {
                None
            },
        };
    }

    if let Some(auto) = &spec.auto_provision {
        if auto.enabled {
            let agent = AutoProvisionAgent::new();
            let connector_name = auto
                .connector_name
                .clone()
                .or_else(|| operation_connector_name(&spec.operation));
            let request = AutoProvisionRequest {
                provider_id: auto.provider_id.clone(),
                channel_id: auto.channel_id.clone(),
                connector_name,
                endpoint: auto.endpoint.clone(),
                required_capabilities: auto.required_capabilities.clone(),
            };

            match agent.plan(&integration_catalog, &spec.pack, &request) {
                Ok(plan) => {
                    if !plan.is_noop() {
                        if let Err(error) = integration_catalog.apply_plan(&mut spec.pack, &plan) {
                            blocked_reason =
                                Some(format!("applying auto-provision plan failed: {error}"));
                        } else {
                            auto_provision_plan = Some(plan);
                        }
                    }
                }
                Err(error) => {
                    blocked_reason = Some(format!("auto-provision planning failed: {error}"));
                }
            }
        }
    }

    if blocked_reason.is_none() {
        for hotfix in &spec.hotfixes {
            if let Err(error) = integration_catalog.apply_hotfix(&hotfix.to_kernel_hotfix()) {
                blocked_reason = Some(format!("hotfix application failed: {error}"));
                break;
            }
        }
    }

    if let Some(reason) = blocked_reason.clone() {
        return SpecRunReport {
            pack_id: spec.pack.pack_id,
            agent_id: spec.agent_id,
            operation_kind: "blocked",
            blocked_reason: Some(reason.clone()),
            approval_guard,
            bridge_support_checksum,
            bridge_support_sha256,
            self_awareness,
            architecture_guard,
            plugin_scan_reports,
            plugin_translation_reports,
            plugin_activation_plans,
            plugin_bootstrap_reports,
            plugin_bootstrap_queue,
            plugin_absorb_reports,
            security_scan_report,
            auto_provision_plan,
            outcome: json!({
                "status": "blocked",
                "reason": reason,
            }),
            integration_catalog,
            audit_events: if include_audit {
                Some(audit_sink.snapshot())
            } else {
                None
            },
        };
    }

    let shared_catalog = Arc::new(Mutex::new(integration_catalog.clone()));
    let bridge_runtime_policy = bridge_runtime_policy(&spec, security_scan_policy.as_ref());
    register_dynamic_catalog_connectors(&mut kernel, shared_catalog, bridge_runtime_policy);

    if let Err(error) = kernel.register_pack(spec.pack.clone()) {
        let reason = format!("spec pack registration failed: {error}");
        return SpecRunReport {
            pack_id: spec.pack.pack_id,
            agent_id: spec.agent_id,
            operation_kind: "blocked",
            blocked_reason: Some(reason.clone()),
            approval_guard,
            bridge_support_checksum,
            bridge_support_sha256,
            self_awareness,
            architecture_guard,
            plugin_scan_reports,
            plugin_translation_reports,
            plugin_activation_plans,
            plugin_bootstrap_reports,
            plugin_bootstrap_queue,
            plugin_absorb_reports,
            security_scan_report,
            auto_provision_plan,
            outcome: json!({
                "status": "blocked",
                "reason": reason,
            }),
            integration_catalog,
            audit_events: if include_audit {
                Some(audit_sink.snapshot())
            } else {
                None
            },
        };
    }
    if let Err(error) = apply_default_selection(&mut kernel, spec.defaults.as_ref()) {
        return SpecRunReport {
            pack_id: spec.pack.pack_id,
            agent_id: spec.agent_id,
            operation_kind: "blocked",
            blocked_reason: Some(error.clone()),
            approval_guard,
            bridge_support_checksum,
            bridge_support_sha256,
            self_awareness,
            architecture_guard,
            plugin_scan_reports,
            plugin_translation_reports,
            plugin_activation_plans,
            plugin_bootstrap_reports,
            plugin_bootstrap_queue,
            plugin_absorb_reports,
            security_scan_report,
            auto_provision_plan,
            outcome: json!({
                "status": "blocked",
                "reason": error,
            }),
            integration_catalog,
            audit_events: if include_audit {
                Some(audit_sink.snapshot())
            } else {
                None
            },
        };
    }

    let token = match kernel.issue_token(&spec.pack.pack_id, &spec.agent_id, spec.ttl_s) {
        Ok(token) => token,
        Err(error) => {
            let reason = format!("token issue for spec failed: {error}");
            return SpecRunReport {
                pack_id: spec.pack.pack_id,
                agent_id: spec.agent_id,
                operation_kind: "blocked",
                blocked_reason: Some(reason.clone()),
                approval_guard,
                bridge_support_checksum,
                bridge_support_sha256,
                self_awareness,
                architecture_guard,
                plugin_scan_reports,
                plugin_translation_reports,
                plugin_activation_plans,
                plugin_bootstrap_reports,
                plugin_bootstrap_queue,
                plugin_absorb_reports,
                security_scan_report,
                auto_provision_plan,
                outcome: json!({
                    "status": "blocked",
                    "reason": reason,
                }),
                integration_catalog,
                audit_events: if include_audit {
                    Some(audit_sink.snapshot())
                } else {
                    None
                },
            };
        }
    };

    let (operation_kind, outcome) = match execute_spec_operation(
        &kernel,
        &spec.pack.pack_id,
        &token,
        &integration_catalog,
        &plugin_scan_reports,
        &plugin_translation_reports,
        &spec.operation,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            return SpecRunReport {
                pack_id: spec.pack.pack_id,
                agent_id: spec.agent_id,
                operation_kind: "blocked",
                blocked_reason: Some(error.clone()),
                approval_guard,
                bridge_support_checksum,
                bridge_support_sha256,
                self_awareness,
                architecture_guard,
                plugin_scan_reports,
                plugin_translation_reports,
                plugin_activation_plans,
                plugin_bootstrap_reports,
                plugin_bootstrap_queue,
                plugin_absorb_reports,
                security_scan_report,
                auto_provision_plan,
                outcome: json!({
                    "status": "blocked",
                    "reason": error,
                }),
                integration_catalog,
                audit_events: if include_audit {
                    Some(audit_sink.snapshot())
                } else {
                    None
                },
            };
        }
    };

    SpecRunReport {
        pack_id: spec.pack.pack_id,
        agent_id: spec.agent_id,
        operation_kind,
        blocked_reason,
        approval_guard,
        bridge_support_checksum,
        bridge_support_sha256,
        self_awareness,
        architecture_guard,
        plugin_scan_reports,
        plugin_translation_reports,
        plugin_activation_plans,
        plugin_bootstrap_reports,
        plugin_bootstrap_queue,
        plugin_absorb_reports,
        security_scan_report,
        auto_provision_plan,
        outcome,
        integration_catalog,
        audit_events: if include_audit {
            Some(audit_sink.snapshot())
        } else {
            None
        },
    }
}

#[derive(Debug, Default)]
struct SecurityScanDelta {
    scanned_plugins: usize,
    high_findings: usize,
    medium_findings: usize,
    low_findings: usize,
    findings: Vec<SecurityFinding>,
    block_reason: Option<String>,
}

fn security_scan_policy(spec: &RunnerSpec) -> Result<Option<SecurityScanSpec>, String> {
    let Some(mut policy) = spec
        .bridge_support
        .as_ref()
        .filter(|bridge| bridge.enabled)
        .and_then(|bridge| bridge.security_scan.clone())
    else {
        return Ok(None);
    };

    if !policy.enabled {
        return Ok(None);
    }

    validate_security_scan_policy(&policy)?;

    let profile = resolve_security_scan_profile(&policy)?;

    if policy.high_risk_metadata_keywords.is_empty() {
        policy.high_risk_metadata_keywords = profile.high_risk_metadata_keywords;
    }

    if policy.wasm.blocked_import_prefixes.is_empty() {
        policy.wasm.blocked_import_prefixes = profile.wasm.blocked_import_prefixes;
    }

    if policy.wasm.max_module_bytes == 0 {
        policy.wasm.max_module_bytes = profile.wasm.max_module_bytes;
    }

    if policy.wasm.allowed_path_prefixes.is_empty() {
        policy.wasm.allowed_path_prefixes = profile.wasm.allowed_path_prefixes;
    }

    if policy.wasm.required_sha256_by_plugin.is_empty() {
        policy.wasm.required_sha256_by_plugin = profile.wasm.required_sha256_by_plugin;
    }

    Ok(Some(policy))
}

fn validate_security_scan_policy(policy: &SecurityScanSpec) -> Result<(), String> {
    if policy.profile_sha256.is_some() && policy.profile_path.is_none() {
        return Err(
            "security scan profile_sha256 requires security_scan.profile_path to be set".to_owned(),
        );
    }
    if policy.profile_signature.is_some() && policy.profile_path.is_none() {
        return Err(
            "security scan profile_signature requires security_scan.profile_path to be set"
                .to_owned(),
        );
    }
    if let Some(signature) = policy.profile_signature.as_ref() {
        if signature.public_key_base64.trim().is_empty() {
            return Err(
                "security scan profile_signature.public_key_base64 cannot be empty".to_owned(),
            );
        }
        if signature.signature_base64.trim().is_empty() {
            return Err(
                "security scan profile_signature.signature_base64 cannot be empty".to_owned(),
            );
        }
    }
    if let Some(export) = policy.siem_export.as_ref().filter(|value| value.enabled) {
        if export.path.trim().is_empty() {
            return Err("security scan siem_export.path cannot be empty when enabled".to_owned());
        }
    }
    if policy.runtime.execute_wasm_component && policy.runtime.allowed_path_prefixes.is_empty() {
        return Err(
            "security scan runtime.execute_wasm_component requires runtime.allowed_path_prefixes to be configured".to_owned(),
        );
    }
    Ok(())
}

fn resolve_security_scan_profile(policy: &SecurityScanSpec) -> Result<SecurityScanProfile, String> {
    if let Some(path) = policy.profile_path.as_deref() {
        let profile = load_security_scan_profile_from_path(path);
        match profile {
            Ok(profile) => {
                if let Some(expected_sha256) = policy.profile_sha256.as_deref() {
                    let actual_sha256 = security_scan_profile_sha256(&profile);
                    if !expected_sha256.eq_ignore_ascii_case(&actual_sha256) {
                        return Err(format!(
                            "security scan profile sha256 mismatch for {path}: expected {expected_sha256}, actual {actual_sha256}",
                        ));
                    }
                }
                if let Some(signature) = policy.profile_signature.as_ref() {
                    verify_security_scan_profile_signature(&profile, signature).map_err(|error| {
                        format!(
                            "security scan profile signature verification failed for {path}: {error}"
                        )
                    })?;
                }
                return Ok(profile);
            }
            Err(error) if policy.profile_sha256.is_some() || policy.profile_signature.is_some() => {
                return Err(format!(
                    "failed to load security scan profile at {path} while profile integrity is pinned: {error}",
                ));
            }
            Err(_) => {}
        }
    }

    Ok(bundled_security_scan_profile())
}

fn load_security_scan_profile_from_path(path: &str) -> Result<SecurityScanProfile, String> {
    let content =
        fs::read_to_string(path).map_err(|error| format!("read profile failed: {error}"))?;
    serde_json::from_str::<SecurityScanProfile>(&content)
        .map_err(|error| format!("parse profile failed: {error}"))
}

fn bundled_security_scan_profile() -> SecurityScanProfile {
    BUNDLED_SECURITY_SCAN_PROFILE
        .get_or_init(|| {
            let raw = include_str!("../config/security-scan-medium-balanced.json");
            serde_json::from_str(raw).unwrap_or_else(|error| {
                panic!("invalid bundled security scan profile config: {error}");
            })
        })
        .clone()
}

fn verify_security_scan_profile_signature(
    profile: &SecurityScanProfile,
    signature: &SecurityProfileSignatureSpec,
) -> Result<(), String> {
    let algorithm = signature.algorithm.trim().to_ascii_lowercase();
    if algorithm != "ed25519" {
        return Err(format!(
            "unsupported profile signature algorithm: {algorithm} (expected ed25519)"
        ));
    }

    let public_key_bytes = BASE64_STANDARD
        .decode(signature.public_key_base64.trim())
        .map_err(|error| format!("invalid public_key_base64: {error}"))?;
    let public_key_bytes: [u8; 32] = public_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "invalid ed25519 public key length (expected 32 bytes)".to_owned())?;
    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|error| format!("invalid ed25519 public key bytes: {error}"))?;

    let signature_bytes = BASE64_STANDARD
        .decode(signature.signature_base64.trim())
        .map_err(|error| format!("invalid signature_base64: {error}"))?;
    let signature_bytes: [u8; 64] = signature_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "invalid ed25519 signature length (expected 64 bytes)".to_owned())?;
    let signature = Ed25519Signature::from_bytes(&signature_bytes);

    let message = security_scan_profile_message(profile);
    verifying_key
        .verify(&message, &signature)
        .map_err(|error| format!("ed25519 verification failed: {error}"))
}

fn security_scan_process_allowlist(spec: &RunnerSpec) -> BTreeSet<String> {
    spec.bridge_support
        .as_ref()
        .filter(|bridge| bridge.enabled)
        .map(|bridge| {
            bridge
                .allowed_process_commands
                .iter()
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| !value.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn apply_security_scan_delta(report: &mut SecurityScanReport, delta: SecurityScanDelta) {
    report.scanned_plugins = report.scanned_plugins.saturating_add(delta.scanned_plugins);
    report.high_findings = report.high_findings.saturating_add(delta.high_findings);
    report.medium_findings = report.medium_findings.saturating_add(delta.medium_findings);
    report.low_findings = report.low_findings.saturating_add(delta.low_findings);
    report.total_findings = report
        .high_findings
        .saturating_add(report.medium_findings)
        .saturating_add(report.low_findings);
    report.findings.extend(delta.findings);
    if let Some(reason) = delta.block_reason {
        report.blocked = true;
        report.block_reason = Some(reason);
    }
}

fn emit_security_scan_audit_event(
    kernel: &LoongClawKernel<StaticPolicyEngine>,
    pack_id: &str,
    agent_id: &str,
    report: &SecurityScanReport,
) -> Result<(), String> {
    if report.scanned_plugins == 0 && report.total_findings == 0 {
        return Ok(());
    }

    let categories: Vec<String> = report
        .findings
        .iter()
        .map(|finding| finding.category.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let finding_ids: Vec<String> = report
        .findings
        .iter()
        .map(|finding| finding.correlation_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    kernel
        .record_audit_event(
            Some(agent_id),
            AuditEventKind::SecurityScanEvaluated {
                pack_id: pack_id.to_owned(),
                scanned_plugins: report.scanned_plugins,
                total_findings: report.total_findings,
                high_findings: report.high_findings,
                medium_findings: report.medium_findings,
                low_findings: report.low_findings,
                blocked: report.blocked,
                block_reason: report.block_reason.clone(),
                categories,
                finding_ids,
            },
        )
        .map_err(|error| format!("failed to record security scan audit event: {error}"))
}

fn emit_security_scan_siem_record(
    pack_id: &str,
    agent_id: &str,
    report: &SecurityScanReport,
    export: &SecuritySiemExportSpec,
) -> Result<SecuritySiemExportReport, String> {
    let mut findings = report.findings.clone();
    let mut truncated_findings = 0usize;

    if export.include_findings {
        if let Some(limit) = export.max_findings_per_record {
            if findings.len() > limit {
                truncated_findings = findings.len().saturating_sub(limit);
                findings.truncate(limit);
            }
        }
    } else {
        truncated_findings = report.findings.len();
        findings.clear();
    }

    let categories: Vec<String> = report
        .findings
        .iter()
        .map(|finding| finding.category.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let finding_ids: Vec<String> = report
        .findings
        .iter()
        .map(|finding| finding.correlation_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let record = json!({
        "event_type": "security_scan_report",
        "ts_epoch_s": current_epoch_s(),
        "pack_id": pack_id,
        "agent_id": agent_id,
        "blocked": report.blocked,
        "block_reason": report.block_reason.clone(),
        "counts": {
            "scanned_plugins": report.scanned_plugins,
            "total_findings": report.total_findings,
            "high_findings": report.high_findings,
            "medium_findings": report.medium_findings,
            "low_findings": report.low_findings,
        },
        "categories": categories,
        "finding_ids": finding_ids,
        "truncated_findings": truncated_findings,
        "findings": findings,
    });

    let path = Path::new(&export.path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("create siem export directory failed: {error}"))?;
        }
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("open siem export file failed: {error}"))?;
    serde_json::to_writer(&mut file, &record)
        .map_err(|error| format!("serialize siem export record failed: {error}"))?;
    file.write_all(b"\n")
        .map_err(|error| format!("flush siem export record failed: {error}"))?;

    Ok(SecuritySiemExportReport {
        enabled: true,
        path: export.path.clone(),
        success: true,
        exported_records: 1,
        exported_findings: findings.len(),
        truncated_findings,
        error: None,
    })
}

fn build_security_finding(
    severity: SecurityFindingSeverity,
    category: impl Into<String>,
    plugin_id: impl Into<String>,
    source_path: impl Into<String>,
    message: impl Into<String>,
    evidence: Value,
) -> SecurityFinding {
    let category = category.into();
    let plugin_id = plugin_id.into();
    let source_path = source_path.into();
    let message = message.into();
    let correlation_id = security_finding_correlation_id(
        &severity,
        &category,
        &plugin_id,
        &source_path,
        &message,
        &evidence,
    );

    SecurityFinding {
        correlation_id,
        severity,
        category,
        plugin_id,
        source_path,
        message,
        evidence,
    }
}

fn security_finding_correlation_id(
    severity: &SecurityFindingSeverity,
    category: &str,
    plugin_id: &str,
    source_path: &str,
    message: &str,
    evidence: &Value,
) -> String {
    let canonical = json!({
        "severity": severity,
        "category": category,
        "plugin_id": plugin_id,
        "source_path": source_path,
        "message": message,
        "evidence": evidence,
    });
    let payload =
        serde_json::to_vec(&canonical).expect("serialize security finding correlation payload");
    let digest = Sha256::digest(&payload);
    let full = hex_lower(&digest);
    format!("sf-{}", &full[..16])
}

fn evaluate_plugin_security_scan(
    report: &PluginScanReport,
    policy: &SecurityScanSpec,
    process_allowlist: &BTreeSet<String>,
) -> SecurityScanDelta {
    let mut delta = SecurityScanDelta::default();
    let metadata_keywords = normalize_signal_list(policy.high_risk_metadata_keywords.clone());
    let blocked_import_prefixes =
        normalize_signal_list(policy.wasm.blocked_import_prefixes.clone());
    let allowed_path_prefixes = normalize_allowed_path_prefixes(&policy.wasm.allowed_path_prefixes);

    for descriptor in &report.descriptors {
        delta.scanned_plugins = delta.scanned_plugins.saturating_add(1);
        let bridge_kind = descriptor_bridge_kind(descriptor);
        let metadata_finding = scan_descriptor_metadata_keywords(descriptor, &metadata_keywords);
        accumulate_security_findings(&mut delta, metadata_finding);

        match bridge_kind {
            PluginBridgeKind::ProcessStdio => {
                let findings = scan_process_stdio_security(descriptor, process_allowlist);
                accumulate_security_findings(&mut delta, findings);
            }
            PluginBridgeKind::NativeFfi => {
                let finding = build_security_finding(
                    SecurityFindingSeverity::Medium,
                    "native_ffi_review",
                    descriptor.manifest.plugin_id.clone(),
                    descriptor.path.clone(),
                    "native_ffi plugin requires manual review and stronger sandboxing",
                    json!({
                        "bridge_kind": bridge_kind.as_str(),
                        "recommendation": "prefer wasm_component for untrusted community plugins",
                    }),
                );
                accumulate_security_findings(&mut delta, vec![finding]);
            }
            PluginBridgeKind::WasmComponent if policy.wasm.enabled => {
                let findings = scan_wasm_plugin_security(
                    descriptor,
                    &policy.wasm,
                    &blocked_import_prefixes,
                    &allowed_path_prefixes,
                );
                accumulate_security_findings(&mut delta, findings);
            }
            PluginBridgeKind::HttpJson
            | PluginBridgeKind::McpServer
            | PluginBridgeKind::Unknown
            | PluginBridgeKind::WasmComponent => {}
        }
    }

    if policy.block_on_high && delta.high_findings > 0 {
        delta.block_reason = Some(format!(
            "security scan blocked {} high-risk finding(s)",
            delta.high_findings
        ));
    }

    delta
}

fn accumulate_security_findings(delta: &mut SecurityScanDelta, findings: Vec<SecurityFinding>) {
    for finding in findings {
        match finding.severity {
            SecurityFindingSeverity::High => {
                delta.high_findings = delta.high_findings.saturating_add(1)
            }
            SecurityFindingSeverity::Medium => {
                delta.medium_findings = delta.medium_findings.saturating_add(1)
            }
            SecurityFindingSeverity::Low => {
                delta.low_findings = delta.low_findings.saturating_add(1)
            }
        }
        delta.findings.push(finding);
    }
}

fn scan_descriptor_metadata_keywords(
    descriptor: &PluginDescriptor,
    keywords: &[String],
) -> Vec<SecurityFinding> {
    if keywords.is_empty() {
        return Vec::new();
    }

    let mut haystack_parts = Vec::new();
    for (key, value) in &descriptor.manifest.metadata {
        haystack_parts.push(key.to_ascii_lowercase());
        haystack_parts.push(value.to_ascii_lowercase());
    }
    let haystack = haystack_parts.join(" ");

    keywords
        .iter()
        .filter(|keyword| haystack.contains(keyword.as_str()))
        .map(|keyword| {
            build_security_finding(
                SecurityFindingSeverity::Medium,
                "metadata_keyword",
                descriptor.manifest.plugin_id.clone(),
                descriptor.path.clone(),
                format!("metadata contains high-risk keyword: {keyword}"),
                json!({
                    "keyword": keyword,
                    "metadata": descriptor.manifest.metadata.clone(),
                }),
            )
        })
        .collect()
}

fn scan_process_stdio_security(
    descriptor: &PluginDescriptor,
    process_allowlist: &BTreeSet<String>,
) -> Vec<SecurityFinding> {
    let mut findings = Vec::new();
    let command = descriptor.manifest.metadata.get("command").cloned();

    match command {
        Some(command) => {
            if !is_process_command_allowed(&command, process_allowlist) {
                findings.push(build_security_finding(
                    SecurityFindingSeverity::High,
                    "process_command_not_allowlisted",
                    descriptor.manifest.plugin_id.clone(),
                    descriptor.path.clone(),
                    format!("process_stdio command {command} is not in runtime allowlist"),
                    json!({
                        "command": command,
                        "allowlist": process_allowlist,
                    }),
                ));
            }
        }
        None => findings.push(build_security_finding(
            SecurityFindingSeverity::Medium,
            "process_command_missing",
            descriptor.manifest.plugin_id.clone(),
            descriptor.path.clone(),
            "process_stdio plugin does not declare metadata.command",
            json!({
                "recommendation": "declare a fixed command and keep bridge allowlist strict",
            }),
        )),
    }

    findings
}

fn scan_wasm_plugin_security(
    descriptor: &PluginDescriptor,
    policy: &WasmSecurityScanSpec,
    blocked_import_prefixes: &[String],
    allowed_path_prefixes: &[PathBuf],
) -> Vec<SecurityFinding> {
    let mut findings = Vec::new();
    let artifact = descriptor_wasm_artifact(descriptor);
    let Some(raw_artifact) = artifact else {
        findings.push(build_security_finding(
            SecurityFindingSeverity::High,
            "wasm_artifact_missing",
            descriptor.manifest.plugin_id.clone(),
            descriptor.path.clone(),
            "wasm plugin does not declare metadata.component/metadata.wasm_path/endpoint artifact",
            json!({}),
        ));
        return findings;
    };

    if raw_artifact.starts_with("http://") || raw_artifact.starts_with("https://") {
        findings.push(build_security_finding(
            SecurityFindingSeverity::High,
            "wasm_remote_artifact",
            descriptor.manifest.plugin_id.clone(),
            descriptor.path.clone(),
            "remote wasm artifact cannot be statically verified for local hotplug safety",
            json!({
                "artifact": raw_artifact,
            }),
        ));
        return findings;
    }

    let artifact_path = resolve_plugin_relative_path(&descriptor.path, &raw_artifact);
    let normalized_artifact_path = normalize_path_for_policy(&artifact_path);
    if !allowed_path_prefixes.is_empty()
        && !allowed_path_prefixes
            .iter()
            .any(|prefix| normalized_artifact_path.starts_with(prefix))
    {
        findings.push(build_security_finding(
            SecurityFindingSeverity::High,
            "wasm_artifact_path_not_allowed",
            descriptor.manifest.plugin_id.clone(),
            descriptor.path.clone(),
            "wasm artifact path is outside allowed_path_prefixes",
            json!({
                "artifact_path": normalized_artifact_path.display().to_string(),
                "allowed_path_prefixes": allowed_path_prefixes
                    .iter()
                    .map(|prefix| prefix.display().to_string())
                    .collect::<Vec<_>>(),
            }),
        ));
        return findings;
    }

    let bytes = match fs::read(&normalized_artifact_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            findings.push(build_security_finding(
                SecurityFindingSeverity::High,
                "wasm_artifact_unreadable",
                descriptor.manifest.plugin_id.clone(),
                descriptor.path.clone(),
                "wasm artifact cannot be read from filesystem",
                json!({
                    "artifact_path": normalized_artifact_path.display().to_string(),
                    "error": error.to_string(),
                }),
            ));
            return findings;
        }
    };

    if bytes.len() > policy.max_module_bytes {
        findings.push(build_security_finding(
            SecurityFindingSeverity::High,
            "wasm_module_too_large",
            descriptor.manifest.plugin_id.clone(),
            descriptor.path.clone(),
            format!(
                "wasm module size {} exceeds max_module_bytes {}",
                bytes.len(),
                policy.max_module_bytes
            ),
            json!({
                "artifact_path": normalized_artifact_path.display().to_string(),
                "module_size_bytes": bytes.len(),
                "max_module_bytes": policy.max_module_bytes,
            }),
        ));
    }

    if !bytes.starts_with(&[0x00, 0x61, 0x73, 0x6d]) {
        findings.push(build_security_finding(
            SecurityFindingSeverity::High,
            "wasm_magic_header_invalid",
            descriptor.manifest.plugin_id.clone(),
            descriptor.path.clone(),
            "artifact does not contain valid wasm magic header",
            json!({
                "artifact_path": normalized_artifact_path.display().to_string(),
            }),
        ));
        return findings;
    }

    let digest = Sha256::digest(&bytes);
    let digest_hex = hex_lower(&digest);

    if let Some(expected) = policy
        .required_sha256_by_plugin
        .get(&descriptor.manifest.plugin_id)
    {
        if !expected.eq_ignore_ascii_case(&digest_hex) {
            findings.push(build_security_finding(
                SecurityFindingSeverity::High,
                "wasm_sha256_mismatch",
                descriptor.manifest.plugin_id.clone(),
                descriptor.path.clone(),
                "wasm sha256 does not match required pin",
                json!({
                    "expected_sha256": expected,
                    "actual_sha256": digest_hex,
                }),
            ));
        }
    } else if policy.require_hash_pin {
        findings.push(build_security_finding(
            SecurityFindingSeverity::High,
            "wasm_sha256_pin_missing",
            descriptor.manifest.plugin_id.clone(),
            descriptor.path.clone(),
            "wasm hash pin is required but missing for plugin",
            json!({
                "required_sha256_by_plugin": policy.required_sha256_by_plugin,
            }),
        ));
    }

    let imports = match parse_wasm_import_modules(&bytes) {
        Ok(imports) => imports,
        Err(error) => {
            findings.push(build_security_finding(
                SecurityFindingSeverity::High,
                "wasm_parse_failed",
                descriptor.manifest.plugin_id.clone(),
                descriptor.path.clone(),
                "wasm parser failed while reading module imports",
                json!({
                    "artifact_path": normalized_artifact_path.display().to_string(),
                    "error": error,
                }),
            ));
            return findings;
        }
    };

    for module_name in &imports {
        let module_name_lower = module_name.to_ascii_lowercase();
        if !policy.allow_wasi && module_name_lower.starts_with("wasi") {
            findings.push(build_security_finding(
                SecurityFindingSeverity::High,
                "wasm_wasi_import_blocked",
                descriptor.manifest.plugin_id.clone(),
                descriptor.path.clone(),
                "wasi import is blocked by wasm security policy",
                json!({
                    "import_module": module_name,
                }),
            ));
        }
        if blocked_import_prefixes
            .iter()
            .any(|prefix| module_name_lower.starts_with(prefix))
        {
            findings.push(build_security_finding(
                SecurityFindingSeverity::High,
                "wasm_import_prefix_blocked",
                descriptor.manifest.plugin_id.clone(),
                descriptor.path.clone(),
                "wasm import module matched blocked prefix",
                json!({
                    "import_module": module_name,
                    "blocked_import_prefixes": blocked_import_prefixes,
                }),
            ));
        }
    }

    findings.push(build_security_finding(
        SecurityFindingSeverity::Low,
        "wasm_digest_observed",
        descriptor.manifest.plugin_id.clone(),
        descriptor.path.clone(),
        "wasm artifact digest captured for audit",
        json!({
            "artifact_path": normalized_artifact_path.display().to_string(),
            "sha256": digest_hex,
            "imports": imports,
        }),
    ));

    findings
}

fn parse_wasm_import_modules(bytes: &[u8]) -> Result<Vec<String>, String> {
    let mut modules = Vec::new();
    for payload in WasmParser::new(0).parse_all(bytes) {
        match payload {
            Ok(WasmPayload::ImportSection(section)) => {
                for import in section {
                    let import = import.map_err(|error| error.to_string())?;
                    modules.push(import.module.to_owned());
                }
            }
            Ok(_) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(modules)
}

fn descriptor_wasm_artifact(descriptor: &PluginDescriptor) -> Option<String> {
    descriptor
        .manifest
        .metadata
        .get("component")
        .cloned()
        .or_else(|| descriptor.manifest.metadata.get("wasm_path").cloned())
        .or_else(|| {
            descriptor
                .manifest
                .endpoint
                .clone()
                .filter(|value| value.to_ascii_lowercase().ends_with(".wasm"))
        })
}

fn resolve_plugin_relative_path(source_path: &str, artifact: &str) -> PathBuf {
    let candidate = PathBuf::from(artifact);
    if candidate.is_absolute() {
        return candidate;
    }

    let source = Path::new(source_path);
    if let Some(parent) = source.parent() {
        parent.join(candidate)
    } else {
        candidate
    }
}

fn normalize_allowed_path_prefixes(prefixes: &[String]) -> Vec<PathBuf> {
    prefixes
        .iter()
        .map(|prefix| normalize_path_for_policy(&PathBuf::from(prefix)))
        .collect()
}

fn normalize_path_for_policy(path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(path) {
        return canonical;
    }

    if path.is_absolute() {
        return path.to_path_buf();
    }

    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn descriptor_bridge_kind(descriptor: &PluginDescriptor) -> PluginBridgeKind {
    if let Some(raw) = descriptor.manifest.metadata.get("bridge_kind") {
        if let Some(kind) = parse_bridge_kind_label(raw) {
            return kind;
        }
    }

    let language = descriptor.language.trim().to_ascii_lowercase();
    match language.as_str() {
        "wasm" | "wat" => return PluginBridgeKind::WasmComponent,
        "rust" | "go" | "c" | "cpp" | "cxx" => return PluginBridgeKind::NativeFfi,
        "python" | "javascript" | "typescript" | "java" => return PluginBridgeKind::ProcessStdio,
        _ => {}
    }

    if let Some(endpoint) = descriptor.manifest.endpoint.as_deref() {
        let endpoint_lower = endpoint.to_ascii_lowercase();
        if endpoint_lower.starts_with("http://") || endpoint_lower.starts_with("https://") {
            return PluginBridgeKind::HttpJson;
        }
        if endpoint_lower.ends_with(".wasm") {
            return PluginBridgeKind::WasmComponent;
        }
    }

    PluginBridgeKind::Unknown
}

fn bridge_support_matrix(spec: &RunnerSpec) -> (BridgeSupportMatrix, bool) {
    match &spec.bridge_support {
        Some(bridge) if bridge.enabled => {
            let mut matrix = BridgeSupportMatrix::default();
            if !bridge.supported_bridges.is_empty() {
                matrix.supported_bridges = bridge.supported_bridges.iter().copied().collect();
            }
            if !bridge.supported_adapter_families.is_empty() {
                matrix.supported_adapter_families =
                    bridge.supported_adapter_families.iter().cloned().collect();
            }
            (matrix, bridge.enforce_supported)
        }
        _ => (BridgeSupportMatrix::default(), false),
    }
}

fn bridge_runtime_policy(
    spec: &RunnerSpec,
    security_scan: Option<&SecurityScanSpec>,
) -> BridgeRuntimePolicy {
    let Some(bridge) = &spec.bridge_support else {
        return BridgeRuntimePolicy::default();
    };
    if !bridge.enabled {
        return BridgeRuntimePolicy::default();
    }

    let runtime = security_scan
        .map(|scan| scan.runtime.clone())
        .unwrap_or_default();
    let wasm_allowed_path_prefixes = runtime
        .allowed_path_prefixes
        .iter()
        .map(PathBuf::from)
        .map(|path| normalize_path_for_policy(&path))
        .collect();

    BridgeRuntimePolicy {
        execute_process_stdio: bridge.execute_process_stdio,
        execute_http_json: bridge.execute_http_json,
        execute_wasm_component: runtime.execute_wasm_component,
        allowed_process_commands: bridge
            .allowed_process_commands
            .iter()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect(),
        wasm_allowed_path_prefixes,
        wasm_max_component_bytes: runtime.max_component_bytes,
        wasm_fuel_limit: runtime.fuel_limit,
        enforce_execution_success: bridge.enforce_execution_success,
    }
}

fn evaluate_approval_guard(spec: &RunnerSpec) -> ApprovalDecisionReport {
    let policy = spec.approval.clone().unwrap_or_default();
    let now_epoch_s = current_epoch_s();
    let operation_key = operation_approval_key(&spec.operation);
    let operation_kind = operation_approval_kind(&spec.operation);
    let target_in_scope = is_operation_in_approval_scope(&spec.operation, policy.scope);
    let denylisted = is_operation_preapproved(&operation_key, &policy.denied_calls);

    let (risk_level, matched_keywords, risk_score) =
        match operation_risk_profile(&spec.operation, &policy) {
            (ApprovalRiskLevel::High, matched, score) => (ApprovalRiskLevel::High, matched, score),
            (_, _, score) => (ApprovalRiskLevel::Low, Vec::new(), score),
        };

    if denylisted {
        return ApprovalDecisionReport {
            mode: policy.mode,
            strategy: policy.strategy,
            scope: policy.scope,
            now_epoch_s,
            operation_key,
            operation_kind,
            risk_level,
            risk_score,
            denylisted: true,
            requires_human_approval: true,
            approved: false,
            reason: "operation is denylisted by human approval policy".to_owned(),
            matched_keywords,
        };
    }

    let one_time_full_access_active = policy.one_time_full_access_granted
        && policy
            .one_time_full_access_expires_at_epoch_s
            .map(|deadline| now_epoch_s <= deadline)
            .unwrap_or(true)
        && policy
            .one_time_full_access_remaining_uses
            .map(|remaining| remaining > 0)
            .unwrap_or(true);

    let one_time_full_access_rejected_reason = if policy.one_time_full_access_granted {
        if let Some(deadline) = policy.one_time_full_access_expires_at_epoch_s {
            if now_epoch_s > deadline {
                Some(format!(
                    "one-time full access grant expired at {deadline}, now is {now_epoch_s}"
                ))
            } else {
                None
            }
        } else if matches!(policy.one_time_full_access_remaining_uses, Some(0)) {
            Some("one-time full access grant has no remaining uses".to_owned())
        } else {
            None
        }
    } else {
        None
    };

    let requires_human_approval = if !target_in_scope {
        false
    } else {
        match policy.mode {
            HumanApprovalMode::Disabled => false,
            HumanApprovalMode::MediumBalanced => matches!(risk_level, ApprovalRiskLevel::High),
            HumanApprovalMode::Strict => true,
        }
    };

    let (approved, reason) = if !requires_human_approval {
        (
            true,
            "operation is allowed by default medium-balanced approval policy".to_owned(),
        )
    } else {
        match policy.strategy {
            HumanApprovalStrategy::OneTimeFullAccess if one_time_full_access_active => (
                true,
                "human granted one-time full access for this execution".to_owned(),
            ),
            HumanApprovalStrategy::PerCall
                if is_operation_preapproved(&operation_key, &policy.approved_calls) =>
            {
                (
                    true,
                    format!("operation {operation_key} is pre-approved by human policy"),
                )
            }
            HumanApprovalStrategy::PerCall => (
                false,
                format!(
                    "human approval required for high-risk operation {operation_key}; \
                     add to approval.approved_calls or switch to one_time_full_access"
                ),
            ),
            HumanApprovalStrategy::OneTimeFullAccess => (false, one_time_full_access_rejected_reason
                .unwrap_or_else(|| {
                    format!(
                        "human one-time full access is not granted for high-risk operation {operation_key}"
                    )
                })),
        }
    };

    ApprovalDecisionReport {
        mode: policy.mode,
        strategy: policy.strategy,
        scope: policy.scope,
        now_epoch_s,
        operation_key,
        operation_kind,
        risk_level,
        risk_score,
        denylisted: false,
        requires_human_approval,
        approved,
        reason,
        matched_keywords,
    }
}

fn operation_approval_key(operation: &OperationSpec) -> String {
    match operation {
        OperationSpec::Task { task_id, .. } => format!("task:{task_id}"),
        OperationSpec::ConnectorLegacy {
            connector_name,
            operation,
            ..
        } => {
            format!("connector_legacy:{connector_name}:{operation}")
        }
        OperationSpec::ConnectorCore {
            connector_name,
            operation,
            ..
        } => {
            format!("connector_core:{connector_name}:{operation}")
        }
        OperationSpec::ConnectorExtension {
            connector_name,
            operation,
            extension,
            ..
        } => {
            format!("connector_extension:{extension}:{connector_name}:{operation}")
        }
        OperationSpec::RuntimeCore { action, .. } => format!("runtime_core:{action}"),
        OperationSpec::RuntimeExtension {
            extension, action, ..
        } => {
            format!("runtime_extension:{extension}:{action}")
        }
        OperationSpec::ToolCore { tool_name, .. } => format!("tool_core:{tool_name}"),
        OperationSpec::ToolExtension {
            extension,
            extension_action,
            ..
        } => {
            format!("tool_extension:{extension}:{extension_action}")
        }
        OperationSpec::MemoryCore { operation, .. } => format!("memory_core:{operation}"),
        OperationSpec::MemoryExtension {
            extension,
            operation,
            ..
        } => {
            format!("memory_extension:{extension}:{operation}")
        }
        OperationSpec::ToolSearch { query, .. } => format!("tool_search:{query}"),
        OperationSpec::ProgrammaticToolCall { caller, .. } => {
            format!("programmatic_tool_call:{caller}")
        }
    }
}

fn operation_approval_kind(operation: &OperationSpec) -> &'static str {
    match operation {
        OperationSpec::Task { .. } => "task",
        OperationSpec::ConnectorLegacy { .. } => "connector_legacy",
        OperationSpec::ConnectorCore { .. } => "connector_core",
        OperationSpec::ConnectorExtension { .. } => "connector_extension",
        OperationSpec::RuntimeCore { .. } => "runtime_core",
        OperationSpec::RuntimeExtension { .. } => "runtime_extension",
        OperationSpec::ToolCore { .. } => "tool_core",
        OperationSpec::ToolExtension { .. } => "tool_extension",
        OperationSpec::MemoryCore { .. } => "memory_core",
        OperationSpec::MemoryExtension { .. } => "memory_extension",
        OperationSpec::ToolSearch { .. } => "tool_search",
        OperationSpec::ProgrammaticToolCall { .. } => "programmatic_tool_call",
    }
}

fn is_operation_in_approval_scope(operation: &OperationSpec, scope: HumanApprovalScope) -> bool {
    match scope {
        HumanApprovalScope::ToolCalls => matches!(
            operation,
            OperationSpec::ToolCore { .. }
                | OperationSpec::ToolExtension { .. }
                | OperationSpec::ProgrammaticToolCall { .. }
        ),
        HumanApprovalScope::AllOperations => true,
    }
}

fn operation_risk_profile(
    operation: &OperationSpec,
    policy: &HumanApprovalSpec,
) -> (ApprovalRiskLevel, Vec<String>, u8) {
    let profile = resolve_approval_risk_profile(policy);
    let keywords = normalize_signal_list(profile.high_risk_keywords);
    let high_risk_tool_names = normalize_signal_list(profile.high_risk_tool_names);
    let high_risk_payload_keys = normalize_signal_list(profile.high_risk_payload_keys);
    let scoring = sanitize_risk_scoring(profile.scoring);

    let haystack = operation_risk_haystack(operation);
    let haystack_lower = haystack.to_ascii_lowercase();

    let matched_keywords: Vec<String> = keywords
        .iter()
        .filter(|keyword| haystack_lower.contains(keyword.as_str()))
        .cloned()
        .collect();

    let matched_tool_name = operation_tool_name(operation)
        .map(|name| name.trim().to_ascii_lowercase())
        .filter(|name| high_risk_tool_names.iter().any(|value| value == name))
        .map(|name| vec![format!("tool:{name}")])
        .unwrap_or_default();

    let payload_keys = operation_payload_keys(operation);
    let matched_payload_keys: Vec<String> = payload_keys
        .iter()
        .map(|key| key.trim().to_ascii_lowercase())
        .filter(|key| high_risk_payload_keys.iter().any(|value| value == key))
        .map(|key| format!("payload_key:{key}"))
        .collect();

    let mut matched = Vec::new();
    matched.extend(matched_keywords.clone());
    matched.extend(matched_tool_name.clone());
    matched.extend(matched_payload_keys.clone());
    matched.sort();
    matched.dedup();

    let keyword_score = (matched_keywords.len().min(scoring.keyword_hit_cap) as u16)
        * u16::from(scoring.keyword_weight);
    let tool_score = if matched_tool_name.is_empty() {
        0
    } else {
        u16::from(scoring.tool_name_weight)
    };
    let payload_key_score = (matched_payload_keys.len().min(scoring.payload_key_hit_cap) as u16)
        * u16::from(scoring.payload_key_weight);
    let risk_score = keyword_score
        .saturating_add(tool_score)
        .saturating_add(payload_key_score)
        .min(100) as u8;

    if matched.is_empty() || risk_score < scoring.high_risk_threshold {
        (ApprovalRiskLevel::Low, Vec::new(), 0)
    } else {
        (ApprovalRiskLevel::High, matched, risk_score)
    }
}

fn resolve_approval_risk_profile(policy: &HumanApprovalSpec) -> ApprovalRiskProfile {
    let mut profile = policy
        .risk_profile_path
        .as_deref()
        .and_then(load_approval_risk_profile_from_path)
        .unwrap_or_else(bundled_approval_risk_profile);

    if !policy.high_risk_keywords.is_empty() {
        profile.high_risk_keywords = policy.high_risk_keywords.clone();
    }
    if !policy.high_risk_tool_names.is_empty() {
        profile.high_risk_tool_names = policy.high_risk_tool_names.clone();
    }
    if !policy.high_risk_payload_keys.is_empty() {
        profile.high_risk_payload_keys = policy.high_risk_payload_keys.clone();
    }

    profile
}

fn load_approval_risk_profile_from_path(path: &str) -> Option<ApprovalRiskProfile> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str::<ApprovalRiskProfile>(&content).ok()
}

fn bundled_approval_risk_profile() -> ApprovalRiskProfile {
    BUNDLED_APPROVAL_RISK_PROFILE
        .get_or_init(|| {
            let raw = include_str!("../config/approval-medium-balanced.json");
            serde_json::from_str(raw).unwrap_or_else(|error| {
                panic!("invalid bundled approval risk profile config: {error}");
            })
        })
        .clone()
}

fn normalize_signal_list(list: Vec<String>) -> Vec<String> {
    let mut normalized: Vec<String> = list
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn sanitize_risk_scoring(mut scoring: ApprovalRiskScoring) -> ApprovalRiskScoring {
    if scoring.keyword_hit_cap == 0 {
        scoring.keyword_hit_cap = 1;
    }
    if scoring.payload_key_hit_cap == 0 {
        scoring.payload_key_hit_cap = 1;
    }
    if scoring.high_risk_threshold == 0 {
        scoring.high_risk_threshold = 20;
    }
    scoring
}

fn operation_tool_name(operation: &OperationSpec) -> Option<&str> {
    match operation {
        OperationSpec::ToolCore { tool_name, .. } => Some(tool_name.as_str()),
        OperationSpec::ToolExtension {
            extension_action, ..
        } => Some(extension_action.as_str()),
        OperationSpec::ProgrammaticToolCall { caller, .. } => Some(caller.as_str()),
        _ => None,
    }
}

fn operation_payload_keys(operation: &OperationSpec) -> Vec<String> {
    match operation {
        OperationSpec::Task { payload, .. }
        | OperationSpec::ConnectorLegacy { payload, .. }
        | OperationSpec::ConnectorCore { payload, .. }
        | OperationSpec::ConnectorExtension { payload, .. }
        | OperationSpec::RuntimeCore { payload, .. }
        | OperationSpec::RuntimeExtension { payload, .. }
        | OperationSpec::ToolCore { payload, .. }
        | OperationSpec::ToolExtension { payload, .. }
        | OperationSpec::MemoryCore { payload, .. }
        | OperationSpec::MemoryExtension { payload, .. } => {
            let mut keys = Vec::new();
            collect_json_keys(payload, &mut keys);
            keys
        }
        OperationSpec::ToolSearch { .. } => {
            let mut keys = Vec::new();
            keys.extend(
                ["query", "limit", "include_deferred", "include_examples"]
                    .iter()
                    .map(|value| (*value).to_owned()),
            );
            keys
        }
        OperationSpec::ProgrammaticToolCall {
            allowed_connectors,
            connector_rate_limits,
            connector_circuit_breakers,
            concurrency,
            steps,
            ..
        } => {
            let mut keys = Vec::new();
            keys.extend(
                [
                    "caller",
                    "max_calls",
                    "include_intermediate",
                    "connector_rate_limits",
                    "connector_circuit_breakers",
                    "concurrency",
                    "return_step",
                    "steps",
                ]
                .iter()
                .map(|value| (*value).to_owned()),
            );
            keys.push("max_in_flight".to_owned());
            keys.push(concurrency.max_in_flight.to_string());
            keys.push("min_in_flight".to_owned());
            keys.push(concurrency.min_in_flight.to_string());
            keys.push("fairness".to_owned());
            keys.push(concurrency.fairness.as_str().to_owned());
            keys.push("adaptive_budget".to_owned());
            keys.push(concurrency.adaptive_budget.to_string());
            keys.push("high_weight".to_owned());
            keys.push(concurrency.high_weight.to_string());
            keys.push("normal_weight".to_owned());
            keys.push(concurrency.normal_weight.to_string());
            keys.push("low_weight".to_owned());
            keys.push(concurrency.low_weight.to_string());
            keys.push("adaptive_recovery_successes".to_owned());
            keys.push(concurrency.adaptive_recovery_successes.to_string());
            keys.push("adaptive_upshift_step".to_owned());
            keys.push(concurrency.adaptive_upshift_step.to_string());
            keys.push("adaptive_downshift_step".to_owned());
            keys.push(concurrency.adaptive_downshift_step.to_string());
            keys.push("adaptive_reduce_on".to_owned());
            for rule in &concurrency.adaptive_reduce_on {
                keys.push(rule.as_str().to_owned());
            }
            keys.extend(allowed_connectors.iter().cloned());
            for (connector_name, limit) in connector_rate_limits {
                keys.push("connector_name".to_owned());
                keys.push(connector_name.clone());
                keys.push("min_interval_ms".to_owned());
                keys.push(limit.min_interval_ms.to_string());
            }
            for (connector_name, policy) in connector_circuit_breakers {
                keys.push("connector_name".to_owned());
                keys.push(connector_name.clone());
                keys.push("failure_threshold".to_owned());
                keys.push(policy.failure_threshold.to_string());
                keys.push("cooldown_ms".to_owned());
                keys.push(policy.cooldown_ms.to_string());
                keys.push("half_open_max_calls".to_owned());
                keys.push(policy.half_open_max_calls.to_string());
                keys.push("success_threshold".to_owned());
                keys.push(policy.success_threshold.to_string());
            }
            for step in steps {
                keys.push("step_id".to_owned());
                match step {
                    ProgrammaticStep::SetLiteral { value, .. } => {
                        collect_json_keys(value, &mut keys);
                    }
                    ProgrammaticStep::JsonPointer { .. } => {
                        keys.push("pointer".to_owned());
                    }
                    ProgrammaticStep::ConnectorCall {
                        connector_name,
                        operation,
                        priority_class,
                        retry,
                        payload,
                        ..
                    } => {
                        keys.push("connector_name".to_owned());
                        keys.push("operation".to_owned());
                        keys.push("priority_class".to_owned());
                        keys.push(connector_name.clone());
                        keys.push(operation.clone());
                        keys.push(priority_class.as_str().to_owned());
                        if let Some(retry) = retry {
                            keys.push("retry".to_owned());
                            keys.push("max_attempts".to_owned());
                            keys.push("initial_backoff_ms".to_owned());
                            keys.push("max_backoff_ms".to_owned());
                            keys.push("jitter_ratio".to_owned());
                            keys.push("adaptive_jitter".to_owned());
                            keys.push(retry.max_attempts.to_string());
                            keys.push(retry.initial_backoff_ms.to_string());
                            keys.push(retry.max_backoff_ms.to_string());
                            keys.push(retry.jitter_ratio.to_string());
                            keys.push(retry.adaptive_jitter.to_string());
                        }
                        collect_json_keys(payload, &mut keys);
                    }
                    ProgrammaticStep::ConnectorBatch {
                        parallel,
                        continue_on_error,
                        calls,
                        ..
                    } => {
                        keys.push("parallel".to_owned());
                        keys.push(parallel.to_string());
                        keys.push("continue_on_error".to_owned());
                        keys.push(continue_on_error.to_string());
                        keys.push("calls".to_owned());
                        for call in calls {
                            keys.push("call_id".to_owned());
                            keys.push(call.call_id.clone());
                            keys.push("connector_name".to_owned());
                            keys.push("operation".to_owned());
                            keys.push("priority_class".to_owned());
                            keys.push(call.connector_name.clone());
                            keys.push(call.operation.clone());
                            keys.push(call.priority_class.as_str().to_owned());
                            if let Some(retry) = &call.retry {
                                keys.push("retry".to_owned());
                                keys.push("max_attempts".to_owned());
                                keys.push("initial_backoff_ms".to_owned());
                                keys.push("max_backoff_ms".to_owned());
                                keys.push("jitter_ratio".to_owned());
                                keys.push("adaptive_jitter".to_owned());
                                keys.push(retry.max_attempts.to_string());
                                keys.push(retry.initial_backoff_ms.to_string());
                                keys.push(retry.max_backoff_ms.to_string());
                                keys.push(retry.jitter_ratio.to_string());
                                keys.push(retry.adaptive_jitter.to_string());
                            }
                            collect_json_keys(&call.payload, &mut keys);
                        }
                    }
                    ProgrammaticStep::Conditional {
                        from_step,
                        pointer,
                        equals,
                        exists,
                        when_true,
                        when_false,
                        ..
                    } => {
                        keys.push("from_step".to_owned());
                        keys.push(from_step.clone());
                        if let Some(pointer) = pointer {
                            keys.push("pointer".to_owned());
                            keys.push(pointer.clone());
                        }
                        if let Some(equals) = equals {
                            keys.push("equals".to_owned());
                            collect_json_keys(equals, &mut keys);
                        }
                        if let Some(exists) = exists {
                            keys.push("exists".to_owned());
                            keys.push(exists.to_string());
                        }
                        keys.push("when_true".to_owned());
                        collect_json_keys(when_true, &mut keys);
                        if let Some(when_false) = when_false {
                            keys.push("when_false".to_owned());
                            collect_json_keys(when_false, &mut keys);
                        }
                    }
                }
            }
            keys
        }
    }
}

fn collect_json_keys(value: &Value, keys: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                keys.push(key.clone());
                collect_json_keys(child, keys);
            }
        }
        Value::Array(list) => {
            for child in list {
                collect_json_keys(child, keys);
            }
        }
        Value::String(_) | Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn operation_risk_haystack(operation: &OperationSpec) -> String {
    let mut text = String::new();
    text.push_str(operation_approval_kind(operation));
    text.push(' ');
    text.push_str(&operation_approval_key(operation));
    text.push(' ');
    for value in operation_payload_strings(operation) {
        text.push_str(&value);
        text.push(' ');
    }
    text
}

fn operation_payload_strings(operation: &OperationSpec) -> Vec<String> {
    match operation {
        OperationSpec::Task { payload, .. }
        | OperationSpec::ConnectorLegacy { payload, .. }
        | OperationSpec::ConnectorCore { payload, .. }
        | OperationSpec::ConnectorExtension { payload, .. }
        | OperationSpec::RuntimeCore { payload, .. }
        | OperationSpec::RuntimeExtension { payload, .. }
        | OperationSpec::ToolCore { payload, .. }
        | OperationSpec::ToolExtension { payload, .. }
        | OperationSpec::MemoryCore { payload, .. }
        | OperationSpec::MemoryExtension { payload, .. } => {
            let mut values = Vec::new();
            collect_json_strings(payload, &mut values);
            values
        }
        OperationSpec::ToolSearch {
            query,
            limit,
            include_deferred,
            include_examples,
        } => {
            let mut values = Vec::new();
            values.push(query.clone());
            values.push(limit.to_string());
            values.push(include_deferred.to_string());
            values.push(include_examples.to_string());
            values
        }
        OperationSpec::ProgrammaticToolCall {
            caller,
            max_calls,
            include_intermediate,
            allowed_connectors,
            connector_rate_limits,
            connector_circuit_breakers,
            concurrency,
            return_step,
            steps,
        } => {
            let mut values = Vec::new();
            values.push(caller.clone());
            values.push(max_calls.to_string());
            values.push(include_intermediate.to_string());
            values.push(concurrency.max_in_flight.to_string());
            values.push(concurrency.min_in_flight.to_string());
            values.push(concurrency.fairness.as_str().to_owned());
            values.push(concurrency.adaptive_budget.to_string());
            values.push(concurrency.high_weight.to_string());
            values.push(concurrency.normal_weight.to_string());
            values.push(concurrency.low_weight.to_string());
            values.push(concurrency.adaptive_recovery_successes.to_string());
            values.push(concurrency.adaptive_upshift_step.to_string());
            values.push(concurrency.adaptive_downshift_step.to_string());
            for rule in &concurrency.adaptive_reduce_on {
                values.push(rule.as_str().to_owned());
            }
            values.extend(allowed_connectors.iter().cloned());
            for (connector_name, limit) in connector_rate_limits {
                values.push(connector_name.clone());
                values.push(limit.min_interval_ms.to_string());
            }
            for (connector_name, policy) in connector_circuit_breakers {
                values.push(connector_name.clone());
                values.push(policy.failure_threshold.to_string());
                values.push(policy.cooldown_ms.to_string());
                values.push(policy.half_open_max_calls.to_string());
                values.push(policy.success_threshold.to_string());
            }
            if let Some(return_step) = return_step {
                values.push(return_step.clone());
            }
            for step in steps {
                match step {
                    ProgrammaticStep::SetLiteral { step_id, value } => {
                        values.push(step_id.clone());
                        collect_json_strings(value, &mut values);
                    }
                    ProgrammaticStep::JsonPointer {
                        step_id,
                        from_step,
                        pointer,
                    } => {
                        values.push(step_id.clone());
                        values.push(from_step.clone());
                        values.push(pointer.clone());
                    }
                    ProgrammaticStep::ConnectorCall {
                        step_id,
                        connector_name,
                        operation,
                        priority_class,
                        retry,
                        payload,
                        ..
                    } => {
                        values.push(step_id.clone());
                        values.push(connector_name.clone());
                        values.push(operation.clone());
                        values.push(priority_class.as_str().to_owned());
                        if let Some(retry) = retry {
                            values.push(retry.max_attempts.to_string());
                            values.push(retry.initial_backoff_ms.to_string());
                            values.push(retry.max_backoff_ms.to_string());
                            values.push(retry.jitter_ratio.to_string());
                            values.push(retry.adaptive_jitter.to_string());
                        }
                        collect_json_strings(payload, &mut values);
                    }
                    ProgrammaticStep::ConnectorBatch {
                        step_id,
                        parallel,
                        continue_on_error,
                        calls,
                    } => {
                        values.push(step_id.clone());
                        values.push(parallel.to_string());
                        values.push(continue_on_error.to_string());
                        for call in calls {
                            values.push(call.call_id.clone());
                            values.push(call.connector_name.clone());
                            values.push(call.operation.clone());
                            values.push(call.priority_class.as_str().to_owned());
                            if let Some(retry) = &call.retry {
                                values.push(retry.max_attempts.to_string());
                                values.push(retry.initial_backoff_ms.to_string());
                                values.push(retry.max_backoff_ms.to_string());
                                values.push(retry.jitter_ratio.to_string());
                                values.push(retry.adaptive_jitter.to_string());
                            }
                            collect_json_strings(&call.payload, &mut values);
                        }
                    }
                    ProgrammaticStep::Conditional {
                        step_id,
                        from_step,
                        pointer,
                        equals,
                        exists,
                        when_true,
                        when_false,
                    } => {
                        values.push(step_id.clone());
                        values.push(from_step.clone());
                        if let Some(pointer) = pointer {
                            values.push(pointer.clone());
                        }
                        if let Some(exists) = exists {
                            values.push(exists.to_string());
                        }
                        if let Some(equals) = equals {
                            collect_json_strings(equals, &mut values);
                        }
                        collect_json_strings(when_true, &mut values);
                        if let Some(when_false) = when_false {
                            collect_json_strings(when_false, &mut values);
                        }
                    }
                }
            }
            values
        }
    }
}

fn collect_json_strings(value: &Value, values: &mut Vec<String>) {
    match value {
        Value::String(string) => values.push(string.clone()),
        Value::Array(array) => {
            for entry in array {
                collect_json_strings(entry, values);
            }
        }
        Value::Object(map) => {
            for (key, entry) in map {
                values.push(key.clone());
                collect_json_strings(entry, values);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn is_operation_preapproved(operation_key: &str, approvals: &[String]) -> bool {
    let operation_key_lower = operation_key.to_ascii_lowercase();
    approvals.iter().any(|raw| {
        let normalized = raw.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return false;
        }
        if normalized == "*" {
            return true;
        }
        if let Some(prefix) = normalized.strip_suffix('*') {
            return operation_key_lower.starts_with(prefix);
        }
        normalized == operation_key_lower
    })
}

fn current_epoch_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn bootstrap_policy(spec: &RunnerSpec) -> Option<BootstrapPolicy> {
    let bootstrap = spec.bootstrap.as_ref()?;
    if !bootstrap.enabled {
        return None;
    }

    let mut policy = BootstrapPolicy::default();
    if let Some(value) = bootstrap.allow_http_json_auto_apply {
        policy.allow_http_json_auto_apply = value;
    }
    if let Some(value) = bootstrap.allow_process_stdio_auto_apply {
        policy.allow_process_stdio_auto_apply = value;
    }
    if let Some(value) = bootstrap.allow_native_ffi_auto_apply {
        policy.allow_native_ffi_auto_apply = value;
    }
    if let Some(value) = bootstrap.allow_wasm_component_auto_apply {
        policy.allow_wasm_component_auto_apply = value;
    }
    if let Some(value) = bootstrap.allow_mcp_server_auto_apply {
        policy.allow_mcp_server_auto_apply = value;
    }
    if let Some(value) = bootstrap.enforce_ready_execution {
        policy.enforce_ready_execution = value;
    }
    if let Some(value) = bootstrap.max_tasks {
        policy.max_tasks = value.max(1);
    }
    Some(policy)
}

fn filter_scan_report_by_activation(
    report: &PluginScanReport,
    activation: &PluginActivationPlan,
) -> PluginScanReport {
    let ready_keys: BTreeSet<(String, String)> = activation
        .candidates
        .iter()
        .filter(|candidate| matches!(candidate.status, PluginActivationStatus::Ready))
        .map(|candidate| (candidate.source_path.clone(), candidate.plugin_id.clone()))
        .collect();

    filter_scan_report_by_keys(report, &ready_keys)
}

fn filter_scan_report_by_keys(
    report: &PluginScanReport,
    allowed_keys: &BTreeSet<(String, String)>,
) -> PluginScanReport {
    let descriptors: Vec<PluginDescriptor> = report
        .descriptors
        .iter()
        .filter(|descriptor| {
            allowed_keys.contains(&(
                descriptor.path.clone(),
                descriptor.manifest.plugin_id.clone(),
            ))
        })
        .cloned()
        .collect();

    PluginScanReport {
        scanned_files: report.scanned_files,
        matched_plugins: descriptors.len(),
        descriptors,
    }
}

fn enrich_scan_report_with_translation(
    report: &PluginScanReport,
    translation: &PluginTranslationReport,
) -> PluginScanReport {
    let mut runtime_by_key: BTreeMap<(String, String), (String, String, String, String)> =
        BTreeMap::new();

    for entry in &translation.entries {
        runtime_by_key.insert(
            (entry.source_path.clone(), entry.plugin_id.clone()),
            (
                entry.runtime.bridge_kind.as_str().to_owned(),
                entry.runtime.adapter_family.clone(),
                entry.runtime.entrypoint_hint.clone(),
                entry.runtime.source_language.clone(),
            ),
        );
    }

    let descriptors: Vec<PluginDescriptor> = report
        .descriptors
        .iter()
        .cloned()
        .map(|mut descriptor| {
            descriptor
                .manifest
                .metadata
                .entry("plugin_source_path".to_owned())
                .or_insert_with(|| descriptor.path.clone());
            descriptor
                .manifest
                .metadata
                .entry("plugin_id".to_owned())
                .or_insert_with(|| descriptor.manifest.plugin_id.clone());
            descriptor
                .manifest
                .metadata
                .entry("defer_loading".to_owned())
                .or_insert_with(|| descriptor.manifest.defer_loading.to_string());
            if let Some(summary) = descriptor.manifest.summary.clone() {
                descriptor
                    .manifest
                    .metadata
                    .entry("summary".to_owned())
                    .or_insert(summary);
            }
            if !descriptor.manifest.tags.is_empty() {
                if let Ok(tags_json) = serde_json::to_string(&descriptor.manifest.tags) {
                    descriptor
                        .manifest
                        .metadata
                        .entry("tags_json".to_owned())
                        .or_insert(tags_json);
                }
            }
            if !descriptor.manifest.input_examples.is_empty() {
                if let Ok(input_examples_json) =
                    serde_json::to_string(&descriptor.manifest.input_examples)
                {
                    descriptor
                        .manifest
                        .metadata
                        .entry("input_examples_json".to_owned())
                        .or_insert(input_examples_json);
                }
            }
            if !descriptor.manifest.output_examples.is_empty() {
                if let Ok(output_examples_json) =
                    serde_json::to_string(&descriptor.manifest.output_examples)
                {
                    descriptor
                        .manifest
                        .metadata
                        .entry("output_examples_json".to_owned())
                        .or_insert(output_examples_json);
                }
            }
            if let Some(component) = descriptor.manifest.metadata.get("component").cloned() {
                let resolved = resolve_plugin_relative_path(&descriptor.path, &component);
                let normalized = normalize_path_for_policy(&resolved);
                descriptor
                    .manifest
                    .metadata
                    .entry("component_resolved_path".to_owned())
                    .or_insert_with(|| normalized.display().to_string());
            }

            if let Some((bridge_kind, adapter_family, entrypoint_hint, source_language)) =
                runtime_by_key.get(&(
                    descriptor.path.clone(),
                    descriptor.manifest.plugin_id.clone(),
                ))
            {
                descriptor
                    .manifest
                    .metadata
                    .entry("bridge_kind".to_owned())
                    .or_insert_with(|| bridge_kind.clone());
                descriptor
                    .manifest
                    .metadata
                    .entry("adapter_family".to_owned())
                    .or_insert_with(|| adapter_family.clone());
                descriptor
                    .manifest
                    .metadata
                    .entry("entrypoint_hint".to_owned())
                    .or_insert_with(|| entrypoint_hint.clone());
                descriptor
                    .manifest
                    .metadata
                    .entry("source_language".to_owned())
                    .or_insert_with(|| source_language.clone());
            }
            descriptor
        })
        .collect();

    PluginScanReport {
        scanned_files: report.scanned_files,
        matched_plugins: descriptors.len(),
        descriptors,
    }
}

fn bridge_support_policy_checksum(bridge: &BridgeSupportSpec) -> String {
    let encoded = bridge_support_policy_canonical_json(bridge);
    fnv1a64_hex(encoded.as_bytes())
}

fn bridge_support_policy_sha256(bridge: &BridgeSupportSpec) -> String {
    let encoded = bridge_support_policy_canonical_json(bridge);
    let digest = Sha256::digest(encoded.as_bytes());
    hex_lower(&digest)
}

fn bridge_support_policy_canonical_json(bridge: &BridgeSupportSpec) -> String {
    let mut bridges = bridge.supported_bridges.clone();
    bridges.sort();

    let mut adapter_families = bridge.supported_adapter_families.clone();
    adapter_families.sort();
    let mut allowed_commands = bridge.allowed_process_commands.clone();
    allowed_commands.sort();
    let security_scan = canonical_security_scan_value(bridge.security_scan.as_ref());

    let canonical = json!({
        "supported_bridges": bridges,
        "supported_adapter_families": adapter_families,
        "enforce_supported": bridge.enforce_supported,
        "execute_process_stdio": bridge.execute_process_stdio,
        "execute_http_json": bridge.execute_http_json,
        "allowed_process_commands": allowed_commands,
        "enforce_execution_success": bridge.enforce_execution_success,
        "security_scan": security_scan,
    });

    serde_json::to_string(&canonical).expect("serialize bridge support checksum payload")
}

fn canonical_security_scan_value(security_scan: Option<&SecurityScanSpec>) -> Value {
    let Some(scan) = security_scan else {
        return Value::Null;
    };

    let mut keywords = scan.high_risk_metadata_keywords.clone();
    keywords.sort();

    let mut blocked_import_prefixes = scan.wasm.blocked_import_prefixes.clone();
    blocked_import_prefixes.sort();

    let mut allowed_path_prefixes = scan.wasm.allowed_path_prefixes.clone();
    allowed_path_prefixes.sort();

    let required_sha256_by_plugin = scan
        .wasm
        .required_sha256_by_plugin
        .iter()
        .map(|(plugin, digest)| (plugin.clone(), digest.clone()))
        .collect::<BTreeMap<_, _>>();
    let profile_signature =
        canonical_security_scan_profile_signature_value(scan.profile_signature.as_ref());
    let siem_export = canonical_security_scan_siem_export_value(scan.siem_export.as_ref());
    let runtime = canonical_security_scan_runtime_value(&scan.runtime);

    json!({
        "enabled": scan.enabled,
        "block_on_high": scan.block_on_high,
        "profile_path": scan.profile_path,
        "profile_sha256": scan.profile_sha256,
        "profile_signature": profile_signature,
        "siem_export": siem_export,
        "runtime": runtime,
        "high_risk_metadata_keywords": keywords,
        "wasm": {
            "enabled": scan.wasm.enabled,
            "max_module_bytes": scan.wasm.max_module_bytes,
            "allow_wasi": scan.wasm.allow_wasi,
            "blocked_import_prefixes": blocked_import_prefixes,
            "allowed_path_prefixes": allowed_path_prefixes,
            "require_hash_pin": scan.wasm.require_hash_pin,
            "required_sha256_by_plugin": required_sha256_by_plugin,
        },
    })
}

fn canonical_security_scan_profile_signature_value(
    signature: Option<&SecurityProfileSignatureSpec>,
) -> Value {
    let Some(signature) = signature else {
        return Value::Null;
    };
    json!({
        "algorithm": signature.algorithm.trim().to_ascii_lowercase(),
        "public_key_base64": signature.public_key_base64,
        "signature_base64": signature.signature_base64,
    })
}

fn canonical_security_scan_siem_export_value(export: Option<&SecuritySiemExportSpec>) -> Value {
    let Some(export) = export else {
        return Value::Null;
    };
    json!({
        "enabled": export.enabled,
        "path": export.path,
        "include_findings": export.include_findings,
        "max_findings_per_record": export.max_findings_per_record,
        "fail_on_error": export.fail_on_error,
    })
}

fn canonical_security_scan_runtime_value(runtime: &SecurityRuntimeExecutionSpec) -> Value {
    let mut allowed_path_prefixes = runtime.allowed_path_prefixes.clone();
    allowed_path_prefixes.sort();

    json!({
        "execute_wasm_component": runtime.execute_wasm_component,
        "allowed_path_prefixes": allowed_path_prefixes,
        "max_component_bytes": runtime.max_component_bytes,
        "fuel_limit": runtime.fuel_limit,
    })
}

fn canonical_security_scan_profile_value(profile: &SecurityScanProfile) -> Value {
    let mut keywords = profile.high_risk_metadata_keywords.clone();
    keywords.sort();

    let mut blocked_import_prefixes = profile.wasm.blocked_import_prefixes.clone();
    blocked_import_prefixes.sort();

    let mut allowed_path_prefixes = profile.wasm.allowed_path_prefixes.clone();
    allowed_path_prefixes.sort();

    let required_sha256_by_plugin = profile
        .wasm
        .required_sha256_by_plugin
        .iter()
        .map(|(plugin, digest)| (plugin.clone(), digest.clone()))
        .collect::<BTreeMap<_, _>>();

    json!({
        "high_risk_metadata_keywords": keywords,
        "wasm": {
            "enabled": profile.wasm.enabled,
            "max_module_bytes": profile.wasm.max_module_bytes,
            "allow_wasi": profile.wasm.allow_wasi,
            "blocked_import_prefixes": blocked_import_prefixes,
            "allowed_path_prefixes": allowed_path_prefixes,
            "require_hash_pin": profile.wasm.require_hash_pin,
            "required_sha256_by_plugin": required_sha256_by_plugin,
        }
    })
}

fn security_scan_profile_sha256(profile: &SecurityScanProfile) -> String {
    let encoded = security_scan_profile_message(profile);
    let digest = Sha256::digest(&encoded);
    hex_lower(&digest)
}

fn security_scan_profile_message(profile: &SecurityScanProfile) -> Vec<u8> {
    let canonical = canonical_security_scan_profile_value(profile);
    serde_json::to_vec(&canonical).expect("serialize security scan profile canonical payload")
}

fn fnv1a64_hex(bytes: &[u8]) -> String {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    let mut hash = OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push_str(&format!("{byte:02x}"));
    }
    encoded
}

fn default_integration_catalog() -> IntegrationCatalog {
    let mut catalog = IntegrationCatalog::new();
    for (provider_id, connector, version, class) in [
        ("openai", "openai", "1.0.0", "llm"),
        ("anthropic", "anthropic", "1.0.0", "llm"),
        ("github", "github", "1.0.0", "devops"),
        ("slack", "slack", "1.0.0", "messaging"),
        ("notion", "notion", "1.0.0", "workspace"),
    ] {
        catalog.register_template(kernel::ProviderTemplate {
            provider_id: provider_id.to_owned(),
            default_connector_name: connector.to_owned(),
            default_version: version.to_owned(),
            metadata: BTreeMap::from([("class".to_owned(), class.to_owned())]),
        });
    }
    catalog
}

fn register_dynamic_catalog_connectors(
    kernel: &mut LoongClawKernel<StaticPolicyEngine>,
    catalog: Arc<Mutex<IntegrationCatalog>>,
    bridge_runtime_policy: BridgeRuntimePolicy,
) {
    let snapshot = {
        let guard = catalog
            .lock()
            .expect("integration catalog mutex poisoned during registration");
        guard.providers()
    };

    for provider in snapshot {
        kernel.register_connector(DynamicCatalogConnector {
            connector_name: provider.connector_name,
            provider_id: provider.provider_id,
            catalog: catalog.clone(),
            bridge_runtime_policy: bridge_runtime_policy.clone(),
        });
    }
}

fn operation_connector_name(operation: &OperationSpec) -> Option<String> {
    match operation {
        OperationSpec::ConnectorLegacy { connector_name, .. }
        | OperationSpec::ConnectorCore { connector_name, .. }
        | OperationSpec::ConnectorExtension { connector_name, .. } => Some(connector_name.clone()),
        OperationSpec::ProgrammaticToolCall { steps, .. } => {
            steps.iter().find_map(|step| match step {
                ProgrammaticStep::ConnectorCall { connector_name, .. } => {
                    Some(connector_name.clone())
                }
                ProgrammaticStep::ConnectorBatch { calls, .. } => {
                    calls.first().map(|call| call.connector_name.clone())
                }
                ProgrammaticStep::SetLiteral { .. }
                | ProgrammaticStep::JsonPointer { .. }
                | ProgrammaticStep::Conditional { .. } => None,
            })
        }
        _ => None,
    }
}

async fn execute_spec_operation(
    kernel: &LoongClawKernel<StaticPolicyEngine>,
    pack_id: &str,
    token: &kernel::CapabilityToken,
    integration_catalog: &IntegrationCatalog,
    plugin_scan_reports: &[PluginScanReport],
    plugin_translation_reports: &[PluginTranslationReport],
    operation: &OperationSpec,
) -> CliResult<(&'static str, Value)> {
    match operation {
        OperationSpec::Task {
            task_id,
            objective,
            required_capabilities,
            payload,
        } => {
            let dispatch = kernel
                .execute_task(
                    pack_id,
                    token,
                    TaskIntent {
                        task_id: task_id.clone(),
                        objective: objective.clone(),
                        required_capabilities: required_capabilities.clone(),
                        payload: payload.clone(),
                    },
                )
                .await
                .map_err(|error| format!("task execution from spec failed: {error}"))?;
            Ok((
                "task",
                json!({
                    "route": dispatch.adapter_route,
                    "outcome": dispatch.outcome,
                }),
            ))
        }
        OperationSpec::ConnectorLegacy {
            connector_name,
            operation,
            required_capabilities,
            payload,
        } => {
            let dispatch = kernel
                .invoke_connector(
                    pack_id,
                    token,
                    ConnectorCommand {
                        connector_name: connector_name.clone(),
                        operation: operation.clone(),
                        required_capabilities: required_capabilities.clone(),
                        payload: payload.clone(),
                    },
                )
                .await
                .map_err(|error| format!("legacy connector execution from spec failed: {error}"))?;
            Ok((
                "connector_legacy",
                json!({
                    "connector_name": dispatch.connector_name,
                    "outcome": dispatch.outcome,
                }),
            ))
        }
        OperationSpec::ConnectorCore {
            connector_name,
            operation,
            required_capabilities,
            payload,
            core,
        } => {
            let dispatch = kernel
                .execute_connector_core(
                    pack_id,
                    token,
                    core.as_deref(),
                    ConnectorCommand {
                        connector_name: connector_name.clone(),
                        operation: operation.clone(),
                        required_capabilities: required_capabilities.clone(),
                        payload: payload.clone(),
                    },
                )
                .await
                .map_err(|error| format!("core connector execution from spec failed: {error}"))?;
            Ok((
                "connector_core",
                json!({
                    "connector_name": dispatch.connector_name,
                    "outcome": dispatch.outcome,
                }),
            ))
        }
        OperationSpec::ConnectorExtension {
            connector_name,
            operation,
            required_capabilities,
            payload,
            extension,
            core,
        } => {
            let dispatch = kernel
                .execute_connector_extension(
                    pack_id,
                    token,
                    extension,
                    core.as_deref(),
                    ConnectorCommand {
                        connector_name: connector_name.clone(),
                        operation: operation.clone(),
                        required_capabilities: required_capabilities.clone(),
                        payload: payload.clone(),
                    },
                )
                .await
                .map_err(|error| {
                    format!("extension connector execution from spec failed: {error}")
                })?;
            Ok((
                "connector_extension",
                json!({
                    "connector_name": dispatch.connector_name,
                    "outcome": dispatch.outcome,
                }),
            ))
        }
        OperationSpec::RuntimeCore {
            action,
            required_capabilities,
            payload,
            core,
        } => {
            let outcome = kernel
                .execute_runtime_core(
                    pack_id,
                    token,
                    required_capabilities,
                    core.as_deref(),
                    RuntimeCoreRequest {
                        action: action.clone(),
                        payload: payload.clone(),
                    },
                )
                .await
                .map_err(|error| format!("runtime core execution from spec failed: {error}"))?;
            Ok(("runtime_core", json!({ "outcome": outcome })))
        }
        OperationSpec::RuntimeExtension {
            action,
            required_capabilities,
            payload,
            extension,
            core,
        } => {
            let outcome = kernel
                .execute_runtime_extension(
                    pack_id,
                    token,
                    required_capabilities,
                    extension,
                    core.as_deref(),
                    RuntimeExtensionRequest {
                        action: action.clone(),
                        payload: payload.clone(),
                    },
                )
                .await
                .map_err(|error| {
                    format!("runtime extension execution from spec failed: {error}")
                })?;
            Ok(("runtime_extension", json!({ "outcome": outcome })))
        }
        OperationSpec::ToolCore {
            tool_name,
            required_capabilities,
            payload,
            core,
        } => {
            let outcome = kernel
                .execute_tool_core(
                    pack_id,
                    token,
                    required_capabilities,
                    core.as_deref(),
                    ToolCoreRequest {
                        tool_name: tool_name.clone(),
                        payload: payload.clone(),
                    },
                )
                .await
                .map_err(|error| format!("tool core execution from spec failed: {error}"))?;
            Ok(("tool_core", json!({ "outcome": outcome })))
        }
        OperationSpec::ToolExtension {
            extension_action,
            required_capabilities,
            payload,
            extension,
            core,
        } => {
            let outcome = kernel
                .execute_tool_extension(
                    pack_id,
                    token,
                    required_capabilities,
                    extension,
                    core.as_deref(),
                    ToolExtensionRequest {
                        extension_action: extension_action.clone(),
                        payload: payload.clone(),
                    },
                )
                .await
                .map_err(|error| format!("tool extension execution from spec failed: {error}"))?;
            Ok(("tool_extension", json!({ "outcome": outcome })))
        }
        OperationSpec::MemoryCore {
            operation,
            required_capabilities,
            payload,
            core,
        } => {
            let outcome = kernel
                .execute_memory_core(
                    pack_id,
                    token,
                    required_capabilities,
                    core.as_deref(),
                    MemoryCoreRequest {
                        operation: operation.clone(),
                        payload: payload.clone(),
                    },
                )
                .await
                .map_err(|error| format!("memory core execution from spec failed: {error}"))?;
            Ok(("memory_core", json!({ "outcome": outcome })))
        }
        OperationSpec::MemoryExtension {
            operation,
            required_capabilities,
            payload,
            extension,
            core,
        } => {
            let outcome = kernel
                .execute_memory_extension(
                    pack_id,
                    token,
                    required_capabilities,
                    extension,
                    core.as_deref(),
                    MemoryExtensionRequest {
                        operation: operation.clone(),
                        payload: payload.clone(),
                    },
                )
                .await
                .map_err(|error| format!("memory extension execution from spec failed: {error}"))?;
            Ok(("memory_extension", json!({ "outcome": outcome })))
        }
        OperationSpec::ToolSearch {
            query,
            limit,
            include_deferred,
            include_examples,
        } => {
            let matches = execute_tool_search(
                integration_catalog,
                plugin_scan_reports,
                plugin_translation_reports,
                query,
                *limit,
                *include_deferred,
                *include_examples,
            );
            Ok((
                "tool_search",
                json!({
                    "query": query,
                    "limit": limit,
                    "include_deferred": include_deferred,
                    "include_examples": include_examples,
                    "returned": matches.len(),
                    "results": matches,
                }),
            ))
        }
        OperationSpec::ProgrammaticToolCall {
            caller,
            max_calls,
            include_intermediate,
            allowed_connectors,
            connector_rate_limits,
            connector_circuit_breakers,
            concurrency,
            return_step,
            steps,
        } => {
            let outcome = execute_programmatic_tool_call(
                kernel,
                pack_id,
                token,
                caller,
                *max_calls,
                *include_intermediate,
                allowed_connectors,
                connector_rate_limits,
                connector_circuit_breakers,
                concurrency,
                return_step.as_deref(),
                steps,
            )
            .await?;
            Ok(("programmatic_tool_call", outcome))
        }
    }
}

fn execute_tool_search(
    integration_catalog: &IntegrationCatalog,
    plugin_scan_reports: &[PluginScanReport],
    plugin_translation_reports: &[PluginTranslationReport],
    query: &str,
    limit: usize,
    include_deferred: bool,
    include_examples: bool,
) -> Vec<ToolSearchResult> {
    let mut entries: BTreeMap<String, ToolSearchEntry> = BTreeMap::new();
    let mut translation_by_key: BTreeMap<
        (String, String),
        (PluginBridgeKind, String, String, String),
    > = BTreeMap::new();

    for report in plugin_translation_reports {
        for entry in &report.entries {
            translation_by_key.insert(
                (entry.source_path.clone(), entry.plugin_id.clone()),
                (
                    entry.runtime.bridge_kind,
                    entry.runtime.adapter_family.clone(),
                    entry.runtime.entrypoint_hint.clone(),
                    entry.runtime.source_language.clone(),
                ),
            );
        }
    }

    for provider in integration_catalog.providers() {
        let channel_endpoint = integration_catalog
            .channels_for_provider(&provider.provider_id)
            .into_iter()
            .find(|channel| channel.enabled)
            .map(|channel| channel.endpoint)
            .unwrap_or_default();
        let bridge_kind = detect_provider_bridge_kind(&provider, &channel_endpoint);
        let tool_id = format!("{}::{}", provider.provider_id, provider.connector_name);
        let summary = provider.metadata.get("summary").cloned();
        let tags = metadata_tags(&provider.metadata);
        let input_examples = metadata_examples(&provider.metadata, "input_examples_json");
        let output_examples = metadata_examples(&provider.metadata, "output_examples_json");
        let deferred = metadata_bool(&provider.metadata, "defer_loading").unwrap_or(false);
        let mut adapter_family = provider.metadata.get("adapter_family").cloned();
        let mut entrypoint_hint = provider
            .metadata
            .get("entrypoint")
            .or_else(|| provider.metadata.get("entrypoint_hint"))
            .cloned();
        let mut source_language = provider.metadata.get("source_language").cloned();
        let mut resolved_bridge_kind = bridge_kind;

        if let (Some(source_path), Some(plugin_id)) = (
            provider.metadata.get("plugin_source_path"),
            provider.metadata.get("plugin_id"),
        ) {
            if let Some((bridge, adapter, entrypoint, language)) =
                translation_by_key.get(&(source_path.clone(), plugin_id.clone()))
            {
                resolved_bridge_kind = *bridge;
                adapter_family = Some(adapter.clone());
                entrypoint_hint = Some(entrypoint.clone());
                source_language = Some(language.clone());
            }
        }

        entries.insert(
            tool_id.clone(),
            ToolSearchEntry {
                tool_id,
                plugin_id: provider.metadata.get("plugin_id").cloned(),
                connector_name: provider.connector_name.clone(),
                provider_id: provider.provider_id.clone(),
                source_path: provider.metadata.get("plugin_source_path").cloned(),
                bridge_kind: resolved_bridge_kind,
                adapter_family,
                entrypoint_hint,
                source_language,
                summary,
                tags,
                input_examples,
                output_examples,
                deferred,
                loaded: true,
            },
        );
    }

    for report in plugin_scan_reports {
        for descriptor in &report.descriptors {
            let manifest = &descriptor.manifest;
            let tool_id = format!("{}::{}", manifest.provider_id, manifest.connector_name);
            let translation =
                translation_by_key.get(&(descriptor.path.clone(), manifest.plugin_id.clone()));
            let bridge_kind = translation
                .map(|(bridge, _, _, _)| *bridge)
                .unwrap_or_else(|| descriptor_bridge_kind(descriptor));
            let adapter_family = translation.map(|(_, adapter, _, _)| adapter.clone());
            let entrypoint_hint = translation.map(|(_, _, entrypoint, _)| entrypoint.clone());
            let source_language = translation.map(|(_, _, _, language)| language.clone());

            let entry = entries
                .entry(tool_id.clone())
                .or_insert_with(|| ToolSearchEntry {
                    tool_id: tool_id.clone(),
                    plugin_id: Some(manifest.plugin_id.clone()),
                    connector_name: manifest.connector_name.clone(),
                    provider_id: manifest.provider_id.clone(),
                    source_path: Some(descriptor.path.clone()),
                    bridge_kind,
                    adapter_family: adapter_family.clone(),
                    entrypoint_hint: entrypoint_hint.clone(),
                    source_language: source_language.clone(),
                    summary: manifest.summary.clone(),
                    tags: manifest.tags.clone(),
                    input_examples: manifest.input_examples.clone(),
                    output_examples: manifest.output_examples.clone(),
                    deferred: manifest.defer_loading,
                    loaded: false,
                });

            if entry.plugin_id.is_none() {
                entry.plugin_id = Some(manifest.plugin_id.clone());
            }
            if entry.source_path.is_none() {
                entry.source_path = Some(descriptor.path.clone());
            }
            if entry.summary.is_none() {
                entry.summary = manifest.summary.clone();
            }
            if entry.adapter_family.is_none() {
                entry.adapter_family = adapter_family.clone();
            }
            if entry.entrypoint_hint.is_none() {
                entry.entrypoint_hint = entrypoint_hint.clone();
            }
            if entry.source_language.is_none() {
                entry.source_language = source_language.clone();
            }
            if entry.input_examples.is_empty() {
                entry.input_examples = manifest.input_examples.clone();
            }
            if entry.output_examples.is_empty() {
                entry.output_examples = manifest.output_examples.clone();
            }
            for tag in &manifest.tags {
                if !entry.tags.iter().any(|existing| existing == tag) {
                    entry.tags.push(tag.clone());
                }
            }
            if !entry.loaded {
                entry.deferred = manifest.defer_loading;
                entry.bridge_kind = bridge_kind;
            }
        }
    }

    let query_normalized = query.trim().to_ascii_lowercase();
    let tokens: Vec<String> = query_normalized
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect();

    let mut ranked: Vec<(u32, ToolSearchEntry)> = entries
        .into_values()
        .filter(|entry| include_deferred || !entry.deferred || entry.loaded)
        .filter_map(|entry| {
            let score = tool_search_score(&entry, &query_normalized, &tokens);
            if query_normalized.is_empty() || score > 0 {
                Some((score, entry))
            } else {
                None
            }
        })
        .collect();

    ranked.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| right.loaded.cmp(&left.loaded))
            .then_with(|| left.tool_id.cmp(&right.tool_id))
    });

    let capped_limit = limit.clamp(1, 50);
    ranked
        .into_iter()
        .take(capped_limit)
        .map(|(score, entry)| ToolSearchResult {
            tool_id: entry.tool_id,
            plugin_id: entry.plugin_id,
            connector_name: entry.connector_name,
            provider_id: entry.provider_id,
            source_path: entry.source_path,
            bridge_kind: entry.bridge_kind.as_str().to_owned(),
            adapter_family: entry.adapter_family,
            entrypoint_hint: entry.entrypoint_hint,
            source_language: entry.source_language,
            score,
            deferred: entry.deferred,
            loaded: entry.loaded,
            summary: entry.summary,
            tags: entry.tags,
            input_examples: if include_examples {
                entry.input_examples
            } else {
                Vec::new()
            },
            output_examples: if include_examples {
                entry.output_examples
            } else {
                Vec::new()
            },
        })
        .collect()
}

fn metadata_tags(metadata: &BTreeMap<String, String>) -> Vec<String> {
    if let Some(raw_json) = metadata.get("tags_json") {
        if let Ok(values) = serde_json::from_str::<Vec<String>>(raw_json) {
            return values;
        }
    }

    metadata
        .get("tags")
        .map(|raw| {
            raw.split([',', ';'])
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn metadata_examples(metadata: &BTreeMap<String, String>, key: &str) -> Vec<Value> {
    metadata
        .get(key)
        .and_then(|raw| serde_json::from_str::<Vec<Value>>(raw).ok())
        .unwrap_or_default()
}

fn metadata_bool(metadata: &BTreeMap<String, String>, key: &str) -> Option<bool> {
    metadata
        .get(key)
        .and_then(|raw| match raw.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "y" | "on" => Some(true),
            "false" | "0" | "no" | "n" | "off" => Some(false),
            _ => None,
        })
}

fn tool_search_score(entry: &ToolSearchEntry, query: &str, tokens: &[String]) -> u32 {
    if query.is_empty() {
        return if entry.loaded { 10 } else { 5 };
    }

    let connector = entry.connector_name.to_ascii_lowercase();
    let provider = entry.provider_id.to_ascii_lowercase();
    let tool_id = entry.tool_id.to_ascii_lowercase();
    let summary = entry
        .summary
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let source_path = entry
        .source_path
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let adapter_family = entry
        .adapter_family
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let entrypoint_hint = entry
        .entrypoint_hint
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let source_language = entry
        .source_language
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let tags: Vec<String> = entry
        .tags
        .iter()
        .map(|tag| tag.to_ascii_lowercase())
        .collect();

    let mut score = 0_u32;
    if connector == query {
        score = score.saturating_add(150);
    } else if connector.contains(query) {
        score = score.saturating_add(110);
    }
    if provider == query {
        score = score.saturating_add(120);
    } else if provider.contains(query) {
        score = score.saturating_add(80);
    }
    if tool_id.contains(query) {
        score = score.saturating_add(60);
    }
    if summary.contains(query) {
        score = score.saturating_add(55);
    }
    if source_path.contains(query) {
        score = score.saturating_add(35);
    }
    if adapter_family.contains(query) {
        score = score.saturating_add(18);
    }
    if entrypoint_hint.contains(query) {
        score = score.saturating_add(12);
    }
    if source_language.contains(query) {
        score = score.saturating_add(10);
    }
    if tags.iter().any(|tag| tag == query) {
        score = score.saturating_add(45);
    } else if tags.iter().any(|tag| tag.contains(query)) {
        score = score.saturating_add(25);
    }

    let haystack = format!(
        "{} {} {} {} {} {} {} {}",
        connector,
        provider,
        tool_id,
        summary,
        adapter_family,
        entrypoint_hint,
        source_language,
        tags.join(" ")
    );
    for token in tokens {
        if haystack.contains(token) {
            score = score.saturating_add(8);
        }
    }

    if entry.loaded {
        score = score.saturating_add(4);
    }
    score
}

fn apply_default_selection(
    kernel: &mut LoongClawKernel<StaticPolicyEngine>,
    defaults: Option<&DefaultCoreSelection>,
) -> CliResult<()> {
    if let Some(defaults) = defaults {
        if let Some(connector) = defaults.connector.as_deref() {
            kernel
                .set_default_core_connector_adapter(connector)
                .map_err(|error| {
                    format!("invalid default connector core adapter ({connector}): {error}")
                })?;
        }
        if let Some(runtime) = defaults.runtime.as_deref() {
            kernel
                .set_default_core_runtime_adapter(runtime)
                .map_err(|error| {
                    format!("invalid default runtime core adapter ({runtime}): {error}")
                })?;
        }
        if let Some(tool) = defaults.tool.as_deref() {
            kernel
                .set_default_core_tool_adapter(tool)
                .map_err(|error| format!("invalid default tool core adapter ({tool}): {error}"))?;
        }
        if let Some(memory) = defaults.memory.as_deref() {
            kernel
                .set_default_core_memory_adapter(memory)
                .map_err(|error| {
                    format!("invalid default memory core adapter ({memory}): {error}")
                })?;
        }
    }
    Ok(())
}

fn parse_json_payload(raw: &str, context: &str) -> CliResult<Value> {
    serde_json::from_str(raw).map_err(|error| format!("invalid JSON for {context}: {error}"))
}

fn read_spec_file(path: &str) -> CliResult<RunnerSpec> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read spec file {path}: {error}"))?;
    serde_json::from_str(&raw).map_err(|error| format!("failed to parse spec file {path}: {error}"))
}

fn write_json_file<T: Serialize>(path: &str, value: &T) -> CliResult<()> {
    let serialized = serde_json::to_string_pretty(value)
        .map_err(|error| format!("serialize JSON value for output file failed: {error}"))?;
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("create output directory failed: {error}"))?;
        }
    }
    fs::write(path, serialized)
        .map_err(|error| format!("write JSON output file failed: {error}"))?;
    Ok(())
}

fn bootstrap_kernel_default() -> LoongClawKernel<StaticPolicyEngine> {
    let mut kernel = LoongClawKernel::new(StaticPolicyEngine::default());
    register_builtin_adapters(&mut kernel);
    kernel
        .register_pack(default_pack_manifest())
        .expect("pack registration failed");
    kernel
}

fn bootstrap_kernel_with_runtime(
    clock: Arc<dyn Clock>,
    audit: Arc<dyn AuditSink>,
) -> LoongClawKernel<StaticPolicyEngine> {
    let mut kernel = LoongClawKernel::with_runtime(StaticPolicyEngine::default(), clock, audit);
    register_builtin_adapters(&mut kernel);
    kernel
        .register_pack(default_pack_manifest())
        .expect("pack registration failed");
    kernel
}

fn register_builtin_adapters(kernel: &mut LoongClawKernel<StaticPolicyEngine>) {
    kernel.register_harness_adapter(EmbeddedPiHarness {
        seen: Mutex::new(Vec::new()),
    });
    kernel.register_connector(WebhookConnector);
    kernel.register_core_connector_adapter(CrmCoreConnector);
    kernel.register_core_connector_adapter(CrmGrpcCoreConnector);
    kernel.register_connector_extension_adapter(ShieldedConnectorExtension);

    kernel.register_core_runtime_adapter(NativeCoreRuntime);
    kernel.register_core_runtime_adapter(FallbackCoreRuntime);
    kernel.register_runtime_extension_adapter(AcpBridgeRuntimeExtension);

    kernel.register_core_tool_adapter(CoreToolRuntime);
    kernel.register_tool_extension_adapter(SqlAnalyticsToolExtension);

    kernel.register_core_memory_adapter(KvCoreMemory);
    kernel.register_memory_extension_adapter(VectorIndexMemoryExtension);
}

fn default_pack_manifest() -> VerticalPackManifest {
    VerticalPackManifest {
        pack_id: DEFAULT_PACK_ID.to_owned(),
        domain: "engineering".to_owned(),
        version: "0.1.0".to_owned(),
        default_route: ExecutionRoute {
            harness_kind: HarnessKind::EmbeddedPi,
            adapter: Some("pi-local".to_owned()),
        },
        allowed_connectors: BTreeSet::from([
            "webhook".to_owned(),
            "crm".to_owned(),
            "erp".to_owned(),
        ]),
        granted_capabilities: BTreeSet::from([
            Capability::InvokeTool,
            Capability::InvokeConnector,
            Capability::MemoryRead,
            Capability::MemoryWrite,
            Capability::ObserveTelemetry,
        ]),
        metadata: BTreeMap::from([
            ("owner".to_owned(), "platform-team".to_owned()),
            ("stage".to_owned(), "prototype".to_owned()),
        ]),
    }
}
