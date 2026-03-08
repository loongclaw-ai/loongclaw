#[derive(Debug, Clone)]
struct ConnectorProtocolContext {
    request_method: String,
    request_id: Option<String>,
    route_method: Option<String>,
    required_capability: Option<String>,
    capabilities: BTreeSet<String>,
}

impl ConnectorProtocolContext {
    fn from_connector_command(
        provider: &kernel::ProviderConfig,
        channel: &kernel::ChannelConfig,
        command: &ConnectorCommand,
    ) -> Self {
        Self {
            request_method: "tools/call".to_owned(),
            request_id: Some(format!(
                "{}:{}:{}",
                provider.provider_id, channel.channel_id, command.operation
            )),
            route_method: None,
            required_capability: None,
            capabilities: protocol_capabilities_for_connector_command(command),
        }
    }

    fn capabilities_vec(&self) -> Vec<String> {
        self.capabilities.iter().cloned().collect::<Vec<_>>()
    }

    fn outbound_frame(&self, payload: Value) -> OutboundFrame {
        OutboundFrame {
            method: self.request_method.clone(),
            id: self.request_id.clone(),
            payload,
        }
    }
}

fn authorize_connector_protocol_context(context: &mut ConnectorProtocolContext) -> Result<(), String> {
    let router = ProtocolRouter::default();
    let resolved_route = router
        .resolve(&context.request_method)
        .map_err(|error| {
            format!(
                "protocol method {} is invalid: {error}",
                context.request_method
            )
        })?;
    context.route_method = Some(resolved_route.method().to_owned());
    context.required_capability = resolved_route.policy.required_capability.clone();
    router
        .authorize(
            &resolved_route,
            &RouteAuthorizationRequest {
                authenticated: true,
                capabilities: context.capabilities.clone(),
            },
        )
        .map_err(|error| format!("protocol route authorization failed: {error}"))?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct BridgeProtocolRuntimeContext {
    request_method: String,
    request_id: Option<String>,
    protocol_route: Option<String>,
    protocol_required_capability: Option<String>,
    protocol_capabilities: Vec<String>,
}

impl BridgeProtocolRuntimeContext {
    fn from_connector_context(context: &ConnectorProtocolContext) -> Self {
        Self {
            request_method: context.request_method.clone(),
            request_id: context.request_id.clone(),
            protocol_route: context.route_method.clone(),
            protocol_required_capability: context.required_capability.clone(),
            protocol_capabilities: context.capabilities_vec(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct HttpJsonRuntimeBase {
    executor: &'static str,
    method: String,
    url: String,
    timeout_ms: u64,
    enforce_protocol_contract: bool,
    #[serde(flatten)]
    protocol: BridgeProtocolRuntimeContext,
}

#[derive(Debug)]
enum HttpJsonRuntimeEvidenceKind {
    BaseOnly,
    RequestOnly {
        request: Value,
    },
    Response {
        status_code: u16,
        request: Value,
        response_text: String,
        response_json: Value,
        response_method: Option<String>,
        response_id: Option<String>,
    },
}

#[derive(Debug, Serialize)]
struct HttpJsonRuntimeRequestOnly {
    #[serde(flatten)]
    base: HttpJsonRuntimeBase,
    request: Value,
}

#[derive(Debug, Serialize)]
struct HttpJsonRuntimeResponse {
    #[serde(flatten)]
    base: HttpJsonRuntimeBase,
    status_code: u16,
    request: Value,
    response_text: String,
    response_json: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_id: Option<String>,
}

fn http_json_runtime_evidence(
    context: &ConnectorProtocolContext,
    method: &str,
    url: &str,
    timeout_ms: u64,
    enforce_protocol_contract: bool,
    evidence_kind: HttpJsonRuntimeEvidenceKind,
) -> Value {
    const EXECUTOR: &str = "http_json_reqwest";
    let base = HttpJsonRuntimeBase {
        executor: EXECUTOR,
        method: method.to_owned(),
        url: url.to_owned(),
        timeout_ms,
        enforce_protocol_contract,
        protocol: BridgeProtocolRuntimeContext::from_connector_context(context),
    };
    match evidence_kind {
        HttpJsonRuntimeEvidenceKind::BaseOnly => serialize_runtime_evidence(EXECUTOR, &base),
        HttpJsonRuntimeEvidenceKind::RequestOnly { request } => serialize_runtime_evidence(
            EXECUTOR,
            &HttpJsonRuntimeRequestOnly { base, request },
        ),
        HttpJsonRuntimeEvidenceKind::Response {
            status_code,
            request,
            response_text,
            response_json,
            response_method,
            response_id,
        } => serialize_runtime_evidence(
            EXECUTOR,
            &HttpJsonRuntimeResponse {
                base,
                status_code,
                request,
                response_text,
                response_json,
                response_method,
                response_id,
            },
        ),
    }
}

#[derive(Debug, Clone, Serialize)]
struct ProcessStdioRuntimeBase {
    executor: &'static str,
    transport_kind: &'static str,
    command: String,
    args: Vec<String>,
    timeout_ms: u64,
    #[serde(flatten)]
    protocol: BridgeProtocolRuntimeContext,
}

#[derive(Debug)]
enum ProcessStdioRuntimeEvidenceKind {
    BaseOnly,
    Execution {
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
        stdout_json: Value,
        response_method: String,
        response_id: Option<String>,
    },
}

#[derive(Debug, Serialize)]
struct ProcessStdioRuntimeExecution {
    #[serde(flatten)]
    base: ProcessStdioRuntimeBase,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    stdout_json: Value,
    response_method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_id: Option<String>,
}

fn process_stdio_runtime_evidence(
    context: &ConnectorProtocolContext,
    command: &str,
    args: &[String],
    timeout_ms: u64,
    evidence_kind: ProcessStdioRuntimeEvidenceKind,
) -> Value {
    const EXECUTOR: &str = "process_stdio_local";
    let base = ProcessStdioRuntimeBase {
        executor: EXECUTOR,
        transport_kind: "json_line",
        command: command.to_owned(),
        args: args.to_vec(),
        timeout_ms,
        protocol: BridgeProtocolRuntimeContext::from_connector_context(context),
    };
    match evidence_kind {
        ProcessStdioRuntimeEvidenceKind::BaseOnly => serialize_runtime_evidence(EXECUTOR, &base),
        ProcessStdioRuntimeEvidenceKind::Execution {
            exit_code,
            stdout,
            stderr,
            stdout_json,
            response_method,
            response_id,
        } => serialize_runtime_evidence(
            EXECUTOR,
            &ProcessStdioRuntimeExecution {
                base,
                exit_code,
                stdout,
                stderr,
                stdout_json,
                response_method,
                response_id,
            },
        ),
    }
}

fn serialize_runtime_evidence<T: Serialize>(executor: &str, runtime: &T) -> Value {
    serde_json::to_value(runtime).unwrap_or_else(|error| {
        json!({
            "executor": executor,
            "serialization_error": error.to_string(),
        })
    })
}

fn parse_http_timeout_ms(provider: &kernel::ProviderConfig) -> u64 {
    parse_clamped_timeout_ms(provider, "http_timeout_ms", 8_000, 300_000)
}

fn parse_http_enforce_protocol_contract(provider: &kernel::ProviderConfig) -> bool {
    parse_bool_flag(
        provider
            .metadata
            .get("http_enforce_protocol_contract")
            .map(String::as_str),
    )
}

fn parse_process_timeout_ms(provider: &kernel::ProviderConfig) -> u64 {
    parse_clamped_timeout_ms(provider, "process_timeout_ms", 5_000, 300_000)
}

fn parse_bool_flag(raw: Option<&str>) -> bool {
    raw.map(|value| value.trim().to_ascii_lowercase())
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
}

fn parse_clamped_timeout_ms(
    provider: &kernel::ProviderConfig,
    metadata_key: &str,
    default_ms: u64,
    max_ms: u64,
) -> u64 {
    provider
        .metadata
        .get(metadata_key)
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(|value| value.min(max_ms))
        .unwrap_or(default_ms)
}

fn protocol_capabilities_for_connector_command(command: &ConnectorCommand) -> BTreeSet<String> {
    let mut capabilities = BTreeSet::new();
    for capability in &command.required_capabilities {
        match capability {
            Capability::MemoryRead
            | Capability::FilesystemRead
            | Capability::ObserveTelemetry => {
                capabilities.insert("discover".to_owned());
            }
            _ => {
                capabilities.insert("invoke".to_owned());
            }
        }
    }
    if capabilities.is_empty() {
        capabilities.insert("invoke".to_owned());
    }
    capabilities
}
