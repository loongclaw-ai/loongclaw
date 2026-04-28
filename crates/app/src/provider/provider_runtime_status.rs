use crate::CliResult;

use super::capability_profile_runtime::ProviderCapabilityProfile;
use super::catalog_query_runtime::fetch_available_models_with_profiles;
use super::contracts::{ProviderToolSchemaMode, provider_runtime_contract};
use super::failover::ProviderFailoverReason;
use super::failover_telemetry_runtime::ProviderFailoverMetricsSnapshot;
use super::http_client_runtime::ProviderHttpClientRuntimeMetricsSnapshot;
use super::profile_health_policy::classify_profile_failure_reason_from_message;
use super::transport;
use crate::config::{LoongConfig, ProviderKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderToolSchemaReadiness {
    pub active_model: String,
    pub structured_tool_schema_enabled: bool,
    pub effective_tool_schema_mode: String,
}

pub fn provider_tool_schema_readiness(config: &LoongConfig) -> ProviderToolSchemaReadiness {
    let provider = &config.provider;
    let runtime_contract = provider_runtime_contract(provider);
    let capability_profile = ProviderCapabilityProfile::from_provider(provider, runtime_contract);
    let active_model = provider.model.clone();
    let capability = capability_profile.resolve_for_model(active_model.as_str());
    let effective_tool_schema_mode = match capability.tool_schema_mode {
        ProviderToolSchemaMode::Disabled => "disabled",
        ProviderToolSchemaMode::EnabledStrict => "enabled_strict",
        ProviderToolSchemaMode::EnabledWithDowngradeOnUnsupported => "enabled_with_downgrade",
    };
    let structured_tool_schema_enabled = capability.turn_tool_schema_enabled();

    ProviderToolSchemaReadiness {
        active_model,
        structured_tool_schema_enabled,
        effective_tool_schema_mode: effective_tool_schema_mode.to_owned(),
    }
}

pub fn provider_http_client_runtime_metrics_snapshot() -> ProviderHttpClientRuntimeMetricsSnapshot {
    super::http_client_runtime::provider_http_client_runtime_metrics_snapshot()
}

pub fn provider_failover_metrics_snapshot() -> ProviderFailoverMetricsSnapshot {
    super::failover_telemetry_runtime::provider_failover_metrics_snapshot()
}

pub fn is_auth_style_failure_message(message: &str) -> bool {
    matches!(
        classify_profile_failure_reason_from_message(message),
        ProviderFailoverReason::AuthRejected
    )
}

pub fn supports_turn_streaming_events(config: &LoongConfig) -> bool {
    let runtime_contract = provider_runtime_contract(&config.provider);
    runtime_contract.supports_turn_streaming_events()
}

pub async fn fetch_available_models(config: &LoongConfig) -> CliResult<Vec<String>> {
    fetch_available_models_with_profiles(config).await
}

pub async fn provider_auth_ready(config: &LoongConfig) -> bool {
    if config.provider.resolved_auth_secret().is_some() {
        return true;
    }

    for header_name in ["authorization", "x-api-key"] {
        if config
            .provider
            .header_value(header_name)
            .is_some_and(|value| !value.trim().is_empty())
        {
            return true;
        }
    }

    if config.provider.kind == ProviderKind::Bedrock
        && let Ok(auth_context) = transport::resolve_request_auth_context(&config.provider).await
    {
        return auth_context.has_bedrock_sigv4_fallback();
    }

    false
}
