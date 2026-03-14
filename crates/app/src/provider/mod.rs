use std::time::Duration;

use serde_json::{Value, json};
use tokio::time::sleep;

use crate::{CliResult, KernelContext};

use super::config::{
    LoongClawConfig, ProviderConfig, ProviderKind, ProviderWireApi, ReasoningEffort,
};
#[cfg(feature = "memory-sqlite")]
use super::memory;

mod policy;
mod shape;
mod transport;

pub use shape::extract_provider_turn;

pub fn build_system_message(
    config: &LoongClawConfig,
    include_system_prompt: bool,
) -> Option<Value> {
    if !include_system_prompt {
        return None;
    }
    let system = config.cli.system_prompt.trim();
    let snapshot = super::tools::capability_snapshot();
    let content = if system.is_empty() {
        snapshot
    } else {
        format!("{system}\n\n{snapshot}")
    };
    Some(json!({
        "role": "system",
        "content": content,
    }))
}

pub(crate) fn build_base_messages(
    config: &LoongClawConfig,
    include_system_prompt: bool,
) -> Vec<Value> {
    build_system_message(config, include_system_prompt)
        .into_iter()
        .collect()
}

pub(crate) fn push_history_message(messages: &mut Vec<Value>, role: &str, content: &str) {
    messages.push(json!({
        "role": role,
        "content": content,
    }));
}

pub fn build_messages_for_session(
    config: &LoongClawConfig,
    session_id: &str,
    include_system_prompt: bool,
) -> CliResult<Vec<Value>> {
    let mut messages = Vec::new();
    if include_system_prompt {
        let system = config.cli.system_prompt.trim();
        let snapshot = super::tools::capability_snapshot();
        let content = if system.is_empty() {
            snapshot
        } else {
            format!("{system}\n\n{snapshot}")
        };
        messages.push(json!({
            "role": "system",
            "content": content,
        }));
    }

    #[cfg(feature = "memory-sqlite")]
    {
        let mem_config =
            super::memory::runtime_config::MemoryRuntimeConfig::from_memory_config(&config.memory);
        let turns = memory::window_direct(session_id, config.memory.sliding_window, &mem_config)
            .map_err(|error| format!("load memory window failed: {error}"))?;
        for turn in turns {
            messages.push(json!({
                "role": turn.role,
                "content": turn.content,
            }));
        }
    }
    #[cfg(not(feature = "memory-sqlite"))]
    {
        let _ = session_id;
    }
    Ok(messages)
}

pub fn build_messages_for_session_in_view(
    config: &LoongClawConfig,
    session_id: &str,
    include_system_prompt: bool,
    tool_view: &crate::tools::ToolView,
) -> CliResult<Vec<Value>> {
    request_message_runtime::build_messages_for_session_in_view(
        config,
        session_id,
        include_system_prompt,
        tool_view,
    )
}

pub async fn request_completion(
    config: &LoongClawConfig,
    messages: &[Value],
    kernel_ctx: Option<&KernelContext>,
) -> CliResult<String> {
    let session = prepare_provider_request_session(config).await?;
    request_across_model_candidates(
        &config.provider,
        kernel_ctx,
        &session.auth_profiles,
        session.profile_state_policy.as_ref(),
        &session.model_candidates,
        session.auto_model_mode,
        session.model_candidate_cooldown_policy.as_ref(),
        |model, auto_model_mode, auth_profile| {
            request_completion_with_model(
                config,
                messages,
                model,
                session.runtime_contract,
                &session.capability_profile,
                auto_model_mode,
                auth_profile,
                &session.endpoint,
                &session.headers,
                &session.request_policy,
                &session.client,
                &session.auth_context,
            )
        },
    )
    .await
}

pub async fn request_turn(
    config: &LoongClawConfig,
    messages: &[Value],
    kernel_ctx: Option<&KernelContext>,
) -> CliResult<crate::conversation::turn_engine::ProviderTurn> {
    request_turn_in_view(
        config,
        messages,
        &crate::tools::runtime_tool_view(),
        kernel_ctx,
    )
    .await
}

pub async fn request_turn_in_view(
    config: &LoongClawConfig,
    messages: &[Value],
    tool_view: &crate::tools::ToolView,
    kernel_ctx: Option<&KernelContext>,
) -> CliResult<crate::conversation::turn_engine::ProviderTurn> {
    let session = prepare_provider_request_session(config).await?;
    let tool_definitions = crate::tools::try_provider_tool_definitions_for_view(tool_view)?;
    request_across_model_candidates(
        &config.provider,
        kernel_ctx,
        &session.auth_profiles,
        session.profile_state_policy.as_ref(),
        &session.model_candidates,
        session.auto_model_mode,
        session.model_candidate_cooldown_policy.as_ref(),
        |model, auto_model_mode, auth_profile| {
            request_turn_with_model(
                config,
                messages,
                model,
                session.runtime_contract,
                &session.capability_profile,
                auto_model_mode,
                tool_definitions.as_slice(),
                auth_profile,
                &session.endpoint,
                &session.headers,
                &session.request_policy,
                &session.client,
                &session.auth_context,
            )
        },
    )
    .await
}

pub async fn fetch_available_models(config: &LoongClawConfig) -> CliResult<Vec<String>> {
    validate_provider_configuration(config)?;
    validate_provider_feature_gate(config)?;
    let headers = transport::build_request_headers(&config.provider)?;
    let request_policy = policy::ProviderRequestPolicy::from_config(&config.provider);
    fetch_available_models_with_policy(config, &headers, &request_policy).await
}

fn build_http_client(request_policy: &policy::ProviderRequestPolicy) -> CliResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(request_policy.timeout_ms))
        .build()
        .map_err(|error| format!("build provider http client failed: {error}"))
}

fn build_completion_request_body(
    config: &LoongClawConfig,
    messages: &[Value],
    model: &str,
    payload_mode: CompletionPayloadMode,
) -> Value {
    let transport_mode = ProviderTransportMode::for_provider(&config.provider);
    match transport_mode {
        ProviderTransportMode::Responses => {
            build_responses_request_body(config, messages, model, payload_mode, false, &[])
        }
        ProviderTransportMode::OpenAiChatCompletions | ProviderTransportMode::KimiApi => {
            build_chat_completions_request_body(
                config,
                messages,
                model,
                payload_mode,
                transport_mode,
            )
        }
    }
}

fn build_chat_completions_request_body(
    config: &LoongClawConfig,
    messages: &[Value],
    model: &str,
    payload_mode: CompletionPayloadMode,
    transport_mode: ProviderTransportMode,
) -> Value {
    let mut body = serde_json::Map::new();
    body.insert("model".to_owned(), json!(model));
    body.insert("messages".to_owned(), Value::Array(messages.to_vec()));
    body.insert("stream".to_owned(), Value::Bool(false));
    if payload_mode.temperature_field == TemperatureField::Include {
        body.insert("temperature".to_owned(), json!(config.provider.temperature));
    }

    if let Some(limit) = config.provider.max_tokens {
        match payload_mode.token_field {
            TokenLimitField::MaxCompletionTokens => {
                body.insert("max_completion_tokens".to_owned(), json!(limit));
            }
            TokenLimitField::MaxTokens => {
                body.insert("max_tokens".to_owned(), json!(limit));
            }
            TokenLimitField::MaxOutputTokens => {
                body.insert("max_output_tokens".to_owned(), json!(limit));
            }
            TokenLimitField::Omit => {}
        }
    }

    if let Some(reasoning_effort) = config.provider.reasoning_effort {
        match payload_mode.reasoning_field {
            ReasoningField::ReasoningEffort => {
                body.insert(
                    "reasoning_effort".to_owned(),
                    json!(reasoning_effort.as_str()),
                );
            }
            ReasoningField::ReasoningObject => {
                body.insert(
                    "reasoning".to_owned(),
                    json!({
                        "effort": reasoning_effort.as_str()
                    }),
                );
            }
            ReasoningField::Omit => {}
        }
    }

    if transport_mode == ProviderTransportMode::KimiApi {
        if let Some(extra_body) = kimi_extra_body(config.provider.reasoning_effort) {
            body.insert("extra_body".to_owned(), extra_body);
        }
    }

    Value::Object(body)
}

fn build_turn_request_body(
    config: &LoongClawConfig,
    messages: &[Value],
    model: &str,
    payload_mode: CompletionPayloadMode,
    include_tool_schema: bool,
    tool_definitions: &[Value],
) -> Value {
    let transport_mode = ProviderTransportMode::for_provider(&config.provider);
    match transport_mode {
        ProviderTransportMode::Responses => build_responses_request_body(
            config,
            messages,
            model,
            payload_mode,
            include_tool_schema,
            tool_definitions,
        ),
        ProviderTransportMode::OpenAiChatCompletions | ProviderTransportMode::KimiApi => {
            let mut body = build_chat_completions_request_body(
                config,
                messages,
                model,
                payload_mode,
                transport_mode,
            );
            if include_tool_schema && !tool_definitions.is_empty() {
                if let Some(object) = body.as_object_mut() {
                    object.insert("tools".to_owned(), Value::Array(tool_definitions.to_vec()));
                    object.insert("tool_choice".to_owned(), json!("auto"));
                }
            }
            body
        }
    }
}

fn build_responses_request_body(
    config: &LoongClawConfig,
    messages: &[Value],
    model: &str,
    payload_mode: CompletionPayloadMode,
    include_tool_schema: bool,
    tool_definitions: &[Value],
) -> Value {
    let mut body = serde_json::Map::new();
    body.insert("model".to_owned(), json!(model));
    body.insert("stream".to_owned(), Value::Bool(false));

    let (instructions, input_items) = build_responses_input_items(messages);
    if let Some(instructions) = instructions {
        body.insert("instructions".to_owned(), json!(instructions));
    }
    body.insert("input".to_owned(), Value::Array(input_items));

    if payload_mode.temperature_field == TemperatureField::Include {
        body.insert("temperature".to_owned(), json!(config.provider.temperature));
    }

    if let Some(limit) = config.provider.max_tokens {
        match payload_mode.token_field {
            TokenLimitField::MaxOutputTokens => {
                body.insert("max_output_tokens".to_owned(), json!(limit));
            }
            TokenLimitField::MaxTokens => {
                body.insert("max_tokens".to_owned(), json!(limit));
            }
            TokenLimitField::MaxCompletionTokens => {
                body.insert("max_completion_tokens".to_owned(), json!(limit));
            }
            TokenLimitField::Omit => {}
        }
    }

    if let Some(reasoning_effort) = config.provider.reasoning_effort {
        match payload_mode.reasoning_field {
            ReasoningField::ReasoningObject => {
                body.insert(
                    "reasoning".to_owned(),
                    json!({
                        "effort": reasoning_effort.as_str()
                    }),
                );
            }
            ReasoningField::ReasoningEffort => {
                body.insert(
                    "reasoning_effort".to_owned(),
                    json!(reasoning_effort.as_str()),
                );
            }
            ReasoningField::Omit => {}
        }
    }

    if include_tool_schema && !tool_definitions.is_empty() {
        body.insert("tools".to_owned(), Value::Array(tool_definitions.to_vec()));
        body.insert("tool_choice".to_owned(), json!("auto"));
    }

    Value::Object(body)
}

fn build_responses_input_items(messages: &[Value]) -> (Option<String>, Vec<Value>) {
    let mut instructions = Vec::new();
    let mut input_items = Vec::new();
    let mut seen_non_system_message = false;

    for message in messages {
        if let Some(native_item) = normalize_responses_native_input_item(message) {
            seen_non_system_message = true;
            input_items.push(native_item);
            continue;
        }

        let Some(role) = message.get("role").and_then(Value::as_str) else {
            continue;
        };
        let Some(text) = extract_request_message_text(message.get("content")) else {
            continue;
        };
        if role == "system" && !seen_non_system_message {
            instructions.push(text);
            continue;
        }
        seen_non_system_message = true;
        input_items.push(json!({
            "role": role,
            "content": [{
                "type": "input_text",
                "text": text,
            }],
        }));
    }

    let merged_instructions = if instructions.is_empty() {
        None
    } else {
        Some(instructions.join("\n\n"))
    };

    (merged_instructions, input_items)
}

fn normalize_responses_native_input_item(message: &Value) -> Option<Value> {
    let item_type = message.get("type").and_then(Value::as_str)?;
    match item_type {
        "function_call" | "function_call_output" | "reasoning" => Some(message.clone()),
        _ => None,
    }
}

fn extract_request_message_text(content: Option<&Value>) -> Option<String> {
    let content = content?;
    if let Some(text) = content.as_str() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
        return None;
    }

    let parts = content.as_array()?;
    let mut merged = Vec::new();
    for part in parts {
        if let Some(text) = part.get("text").and_then(Value::as_str) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                merged.push(trimmed.to_owned());
            }
            continue;
        }
        if let Some(text) = part
            .get("text")
            .and_then(|value| value.get("value"))
            .and_then(Value::as_str)
        {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                merged.push(trimmed.to_owned());
            }
        }
    }

    if merged.is_empty() {
        return None;
    }
    Some(merged.join("\n"))
}

async fn resolve_request_models(
    config: &LoongClawConfig,
    headers: &reqwest::header::HeaderMap,
    request_policy: &policy::ProviderRequestPolicy,
) -> CliResult<Vec<String>> {
    if let Some(model) = config.provider.resolved_model() {
        return Ok(vec![model]);
    }
    let available = fetch_available_models_with_policy(config, headers, request_policy).await?;
    let ordered = rank_model_candidates(&config.provider, &available);
    if ordered.is_empty() {
        return Err("provider model-list is empty; set provider.model explicitly".to_owned());
    }
    Ok(ordered)
}

async fn fetch_available_models_with_policy(
    config: &LoongClawConfig,
    headers: &reqwest::header::HeaderMap,
    request_policy: &policy::ProviderRequestPolicy,
) -> CliResult<Vec<String>> {
    let endpoint = config.provider.models_endpoint();
    let client = build_http_client(request_policy)?;

    let mut attempt = 0usize;
    let mut backoff_ms = request_policy.initial_backoff_ms;
    loop {
        attempt += 1;
        let mut req = client.get(endpoint.clone()).headers(headers.clone());
        if let Some(auth_header) = config.provider.authorization_header() {
            req = req.header(reqwest::header::AUTHORIZATION, auth_header);
        }

        match req.send().await {
            Ok(response) => {
                let status = response.status();
                let response_body = transport::decode_response_body(response)
                    .await
                    .map_err(|error| {
                        format!(
                            "provider model-list decode failed on attempt {attempt}/{max_attempts}: {error}",
                            max_attempts = request_policy.max_attempts
                        )
                    })?;

                if status.is_success() {
                    let models = shape::extract_model_ids(&response_body);
                    if models.is_empty() {
                        return Err(format!(
                            "provider model-list returned no models from endpoint `{endpoint}`"
                        ));
                    }
                    return Ok(models);
                }

                let status_code = status.as_u16();
                if attempt < request_policy.max_attempts && policy::should_retry_status(status_code)
                {
                    sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = policy::next_backoff_ms(backoff_ms, request_policy.max_backoff_ms);
                    continue;
                }

                return Err(format!(
                    "provider model-list returned status {status_code} on attempt {attempt}/{max_attempts}: {response_body}",
                    max_attempts = request_policy.max_attempts
                ));
            }
            Err(error) => {
                if attempt < request_policy.max_attempts && policy::should_retry_error(&error) {
                    sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = policy::next_backoff_ms(backoff_ms, request_policy.max_backoff_ms);
                    continue;
                }
                return Err(format!(
                    "provider model-list request failed on attempt {attempt}/{max_attempts}: {error}",
                    max_attempts = request_policy.max_attempts
                ));
            }
        }
    }
}

fn rank_model_candidates(provider: &ProviderConfig, available: &[String]) -> Vec<String> {
    let mut ordered = Vec::new();
    for raw in &provider.preferred_models {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(matched) = available.iter().find(|model| *model == trimmed) {
            push_unique_model(&mut ordered, matched);
            continue;
        }
        if let Some(matched) = available
            .iter()
            .find(|model| model.eq_ignore_ascii_case(trimmed))
        {
            push_unique_model(&mut ordered, matched);
        }
    }

    for model in available {
        push_unique_model(&mut ordered, model);
    }
    ordered
}

fn push_unique_model(out: &mut Vec<String>, model: &str) {
    if out.iter().any(|existing| existing == model) {
        return;
    }
    out.push(model.to_owned());
}

#[derive(Debug)]
struct ModelRequestError {
    message: String,
    try_next_model: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderTransportMode {
    OpenAiChatCompletions,
    Responses,
    KimiApi,
}

impl ProviderTransportMode {
    fn for_provider(provider: &ProviderConfig) -> Self {
        match provider.kind {
            ProviderKind::KimiCoding => Self::KimiApi,
            _ => match provider.wire_api {
                ProviderWireApi::ChatCompletions => Self::OpenAiChatCompletions,
                ProviderWireApi::Responses => Self::Responses,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenLimitField {
    MaxOutputTokens,
    MaxCompletionTokens,
    MaxTokens,
    Omit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReasoningField {
    ReasoningEffort,
    ReasoningObject,
    Omit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemperatureField {
    Include,
    Omit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompletionPayloadMode {
    token_field: TokenLimitField,
    reasoning_field: ReasoningField,
    temperature_field: TemperatureField,
}

impl CompletionPayloadMode {
    fn default_for(provider: &ProviderConfig) -> Self {
        let transport_mode = ProviderTransportMode::for_provider(provider);
        let token_field = if provider.max_tokens.is_some() {
            match transport_mode {
                ProviderTransportMode::Responses => TokenLimitField::MaxOutputTokens,
                ProviderTransportMode::OpenAiChatCompletions
                    if provider.kind == ProviderKind::Openai =>
                {
                    TokenLimitField::MaxCompletionTokens
                }
                ProviderTransportMode::OpenAiChatCompletions | ProviderTransportMode::KimiApi => {
                    TokenLimitField::MaxTokens
                }
            }
        } else {
            TokenLimitField::Omit
        };

        let reasoning_field = if provider.reasoning_effort.is_some() {
            match transport_mode {
                ProviderTransportMode::Responses => ReasoningField::ReasoningObject,
                ProviderTransportMode::OpenAiChatCompletions | ProviderTransportMode::KimiApi => {
                    ReasoningField::ReasoningEffort
                }
            }
        } else {
            ReasoningField::Omit
        };

        Self {
            token_field,
            reasoning_field,
            temperature_field: TemperatureField::Include,
        }
    }
}

#[derive(Debug, Default)]
struct ProviderApiError {
    code: Option<String>,
    param: Option<String>,
    message: Option<String>,
}

fn parse_provider_api_error(body: &Value) -> ProviderApiError {
    let error = body.get("error").unwrap_or(body);
    let raw_body_message = body.get("raw_body").and_then(Value::as_str);
    ProviderApiError {
        code: error
            .get("code")
            .and_then(Value::as_str)
            .map(str::to_lowercase),
        param: error
            .get("param")
            .and_then(Value::as_str)
            .map(str::to_lowercase),
        message: error
            .get("message")
            .and_then(Value::as_str)
            .or(raw_body_message)
            .map(str::to_lowercase),
    }
}

fn is_parameter_unsupported(error: &ProviderApiError, parameter: &str) -> bool {
    let param = parameter.to_lowercase();
    if error.param.as_deref() == Some(param.as_str()) {
        return true;
    }

    let message = error.message.as_deref().unwrap_or_default();
    if !(message.contains("unknown parameter")
        || message.contains("unsupported parameter")
        || message.contains("not supported"))
    {
        return false;
    }
    message.contains(param.as_str())
}

fn should_disable_tool_schema_for_error(error: &ProviderApiError) -> bool {
    if is_parameter_unsupported(error, "tools") || is_parameter_unsupported(error, "tool_choice") {
        return true;
    }

    let message = error.message.as_deref().unwrap_or_default();
    message.contains("tools are not supported")
        || message.contains("tool use is not supported")
        || message.contains("function calling is not supported")
}

fn should_try_next_model_on_error(error: &ProviderApiError) -> bool {
    if let Some(code) = error.code.as_deref() {
        if matches!(
            code,
            "model_not_found" | "unsupported_model" | "invalid_model" | "not_found_error"
        ) {
            return true;
        }
    }
    if error.param.as_deref() == Some("model") {
        return true;
    }

    let message = error.message.as_deref().unwrap_or_default();
    message.contains("only supports")
        || message.contains("only available")
        || message.contains("this endpoint")
        || message.contains("/v1/responses")
        || message.contains("model does not exist")
}

fn should_fallback_responses_to_chat_completions(
    provider: &ProviderConfig,
    status_code: u16,
    error: &ProviderApiError,
) -> bool {
    if provider.responses_fallback_provider().is_none()
        || !matches!(status_code, 400 | 404 | 405 | 415 | 422)
    {
        return false;
    }

    let message = error.message.as_deref().unwrap_or_default();
    if message.is_empty()
        || message.contains("unauthorized")
        || message.contains("forbidden")
        || message.contains("invalid api key")
        || message.contains("rate limit")
        || message.contains("insufficient quota")
    {
        return false;
    }

    let mentions_chat_endpoint =
        message.contains("/v1/chat/completions") || message.contains("chat/completions");
    let rejects_responses_input = matches!(error.param.as_deref(), Some("input" | "instructions"))
        && (message.contains("unknown parameter")
            || message.contains("unsupported parameter")
            || message.contains("expects")
            || message.contains("not supported"));
    let requires_messages = error.param.as_deref() == Some("messages")
        && (message.contains("required")
            || message.contains("missing")
            || message.contains("expects"));
    let textual_messages_hint = message.contains("expects `messages`")
        || message.contains("expects messages")
        || message.contains("use `messages`")
        || message.contains("use 'messages'")
        || message.contains("missing required parameter: `messages`")
        || message.contains("requires `messages`")
        || message.contains("requires messages")
        || message.contains("expected `messages`")
        || message.contains("expected messages")
        || message.contains("unknown parameter `input`")
        || message.contains("unknown parameter: `input`")
        || message.contains("unsupported parameter `input`")
        || message.contains("unsupported parameter: `input`")
        || message.contains("unknown parameter `instructions`")
        || message.contains("unknown parameter: `instructions`")
        || message.contains("unsupported parameter `instructions`")
        || message.contains("unsupported parameter: `instructions`");

    mentions_chat_endpoint || rejects_responses_input || requires_messages || textual_messages_hint
}

fn adapt_payload_mode_for_error(
    current: CompletionPayloadMode,
    provider: &ProviderConfig,
    error: &ProviderApiError,
) -> Option<CompletionPayloadMode> {
    if provider.max_tokens.is_some() {
        if is_parameter_unsupported(error, "max_output_tokens") {
            return Some(match current.token_field {
                TokenLimitField::MaxOutputTokens => CompletionPayloadMode {
                    token_field: TokenLimitField::MaxTokens,
                    ..current
                },
                TokenLimitField::MaxTokens => CompletionPayloadMode {
                    token_field: TokenLimitField::MaxCompletionTokens,
                    ..current
                },
                TokenLimitField::MaxCompletionTokens => CompletionPayloadMode {
                    token_field: TokenLimitField::Omit,
                    ..current
                },
                TokenLimitField::Omit => current,
            });
        }

        if is_parameter_unsupported(error, "max_tokens") {
            return Some(match current.token_field {
                TokenLimitField::MaxTokens => CompletionPayloadMode {
                    token_field: TokenLimitField::MaxCompletionTokens,
                    ..current
                },
                TokenLimitField::MaxOutputTokens => CompletionPayloadMode {
                    token_field: TokenLimitField::MaxCompletionTokens,
                    ..current
                },
                TokenLimitField::MaxCompletionTokens => CompletionPayloadMode {
                    token_field: TokenLimitField::Omit,
                    ..current
                },
                TokenLimitField::Omit => current,
            });
        }

        if is_parameter_unsupported(error, "max_completion_tokens") {
            return Some(match current.token_field {
                TokenLimitField::MaxCompletionTokens => CompletionPayloadMode {
                    token_field: TokenLimitField::MaxTokens,
                    ..current
                },
                TokenLimitField::MaxOutputTokens => CompletionPayloadMode {
                    token_field: TokenLimitField::MaxTokens,
                    ..current
                },
                TokenLimitField::MaxTokens => CompletionPayloadMode {
                    token_field: TokenLimitField::Omit,
                    ..current
                },
                TokenLimitField::Omit => current,
            });
        }
    }

    if provider.reasoning_effort.is_some() {
        if is_parameter_unsupported(error, "reasoning_effort") {
            return Some(match current.reasoning_field {
                ReasoningField::ReasoningEffort => CompletionPayloadMode {
                    reasoning_field: ReasoningField::ReasoningObject,
                    ..current
                },
                ReasoningField::ReasoningObject => CompletionPayloadMode {
                    reasoning_field: ReasoningField::Omit,
                    ..current
                },
                ReasoningField::Omit => current,
            });
        }

        if is_parameter_unsupported(error, "reasoning") {
            return Some(match current.reasoning_field {
                ReasoningField::ReasoningObject => CompletionPayloadMode {
                    reasoning_field: ReasoningField::ReasoningEffort,
                    ..current
                },
                ReasoningField::ReasoningEffort => CompletionPayloadMode {
                    reasoning_field: ReasoningField::Omit,
                    ..current
                },
                ReasoningField::Omit => current,
            });
        }
    }

    let temperature_rejected = is_parameter_unsupported(error, "temperature")
        || (error.param.as_deref() == Some("temperature")
            && error
                .message
                .as_deref()
                .unwrap_or_default()
                .contains("only the default"));
    if temperature_rejected {
        return Some(match current.temperature_field {
            TemperatureField::Include => CompletionPayloadMode {
                temperature_field: TemperatureField::Omit,
                ..current
            },
            TemperatureField::Omit => current,
        });
    }

    None
}

#[allow(clippy::too_many_arguments)]
async fn request_completion_with_model(
    config: &LoongClawConfig,
    messages: &[Value],
    model: &str,
    auto_model_mode: bool,
    headers: &reqwest::header::HeaderMap,
    request_policy: &policy::ProviderRequestPolicy,
    client: &reqwest::Client,
) -> Result<String, ModelRequestError> {
    let mut attempt = 0usize;
    let mut backoff_ms = request_policy.initial_backoff_ms;
    let mut request_provider = config.provider.clone();
    let mut payload_mode = CompletionPayloadMode::default_for(&request_provider);
    let mut tried_payload_modes = vec![payload_mode];

    loop {
        attempt += 1;
        let endpoint = request_provider.endpoint();
        let mut request_config = config.clone();
        request_config.provider = request_provider.clone();
        let body = build_completion_request_body(&request_config, messages, model, payload_mode);
        let mut req = client.post(endpoint).headers(headers.clone()).json(&body);
        if let Some(auth_header) = request_provider.authorization_header() {
            req = req.header(reqwest::header::AUTHORIZATION, auth_header);
        }

        match req.send().await {
            Ok(response) => {
                let status = response.status();
                let response_body = transport::decode_response_body(response)
                    .await
                    .map_err(|error| ModelRequestError {
                        message: format!(
                            "provider response decode failed for model `{model}` on attempt {attempt}/{max_attempts}: {error}",
                            max_attempts = request_policy.max_attempts
                        ),
                        try_next_model: false,
                    })?;

                if status.is_success() {
                    let content = shape::extract_message_content(&response_body).ok_or_else(|| {
                        ModelRequestError {
                            message: format!(
                                "provider response missing choices[0].message.content for model `{model}` on attempt {attempt}/{max_attempts}: {response_body}",
                                max_attempts = request_policy.max_attempts
                            ),
                            try_next_model: false,
                        }
                    })?;
                    return Ok(content);
                }

                let api_error = parse_provider_api_error(&response_body);
                if let Some(next_mode) =
                    adapt_payload_mode_for_error(payload_mode, &request_provider, &api_error)
                {
                    if !tried_payload_modes.contains(&next_mode) {
                        payload_mode = next_mode;
                        tried_payload_modes.push(next_mode);
                        continue;
                    }
                }

                let status_code = status.as_u16();
                if should_fallback_responses_to_chat_completions(
                    &request_provider,
                    status_code,
                    &api_error,
                ) {
                    if let Some(fallback_provider) = request_provider.responses_fallback_provider()
                    {
                        request_provider = fallback_provider;
                        payload_mode = CompletionPayloadMode::default_for(&request_provider);
                        tried_payload_modes = vec![payload_mode];
                        continue;
                    }
                }
                if attempt < request_policy.max_attempts && policy::should_retry_status(status_code)
                {
                    sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = policy::next_backoff_ms(backoff_ms, request_policy.max_backoff_ms);
                    continue;
                }

                if auto_model_mode && should_try_next_model_on_error(&api_error) {
                    return Err(ModelRequestError {
                        message: format!(
                            "model `{model}` rejected by provider endpoint; trying next candidate. status {status_code}: {response_body}"
                        ),
                        try_next_model: true,
                    });
                }

                return Err(ModelRequestError {
                    message: format!(
                        "provider returned status {status_code} for model `{model}` on attempt {attempt}/{max_attempts}: {response_body}",
                        max_attempts = request_policy.max_attempts
                    ),
                    try_next_model: false,
                });
            }
            Err(error) => {
                if attempt < request_policy.max_attempts && policy::should_retry_error(&error) {
                    sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = policy::next_backoff_ms(backoff_ms, request_policy.max_backoff_ms);
                    continue;
                }
                return Err(ModelRequestError {
                    message: format!(
                        "provider request failed for model `{model}` on attempt {attempt}/{max_attempts}: {error}",
                        max_attempts = request_policy.max_attempts
                    ),
                    try_next_model: false,
                });
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn request_turn_with_model(
    config: &LoongClawConfig,
    messages: &[Value],
    model: &str,
    auto_model_mode: bool,
    headers: &reqwest::header::HeaderMap,
    request_policy: &policy::ProviderRequestPolicy,
    client: &reqwest::Client,
) -> Result<crate::conversation::turn_engine::ProviderTurn, ModelRequestError> {
    let mut attempt = 0usize;
    let mut backoff_ms = request_policy.initial_backoff_ms;
    let mut request_provider = config.provider.clone();
    let mut payload_mode = CompletionPayloadMode::default_for(&request_provider);
    let mut tried_payload_modes = vec![payload_mode];
    let tool_definitions = super::tools::provider_tool_definitions();
    let mut include_tool_schema = !tool_definitions.is_empty();

    loop {
        attempt += 1;
        let endpoint = request_provider.endpoint();
        let mut request_config = config.clone();
        request_config.provider = request_provider.clone();
        let body = build_turn_request_body(
            &request_config,
            messages,
            model,
            payload_mode,
            include_tool_schema,
            &tool_definitions,
        );
        let mut req = client.post(endpoint).headers(headers.clone()).json(&body);
        if let Some(auth_header) = request_provider.authorization_header() {
            req = req.header(reqwest::header::AUTHORIZATION, auth_header);
        }

        match req.send().await {
            Ok(response) => {
                let status = response.status();
                let response_body = transport::decode_response_body(response)
                    .await
                    .map_err(|error| ModelRequestError {
                        message: format!(
                            "provider response decode failed for model `{model}` on attempt {attempt}/{max_attempts}: {error}",
                            max_attempts = request_policy.max_attempts
                        ),
                        try_next_model: false,
                    })?;

                if status.is_success() {
                    let turn = shape::extract_provider_turn(&response_body).ok_or_else(|| {
                        ModelRequestError {
                            message: format!(
                                "provider response missing choices[0].message for model `{model}` on attempt {attempt}/{max_attempts}: {response_body}",
                                max_attempts = request_policy.max_attempts
                            ),
                            try_next_model: false,
                        }
                    })?;
                    return Ok(turn);
                }

                let api_error = parse_provider_api_error(&response_body);
                if include_tool_schema && should_disable_tool_schema_for_error(&api_error) {
                    include_tool_schema = false;
                    continue;
                }
                if let Some(next_mode) =
                    adapt_payload_mode_for_error(payload_mode, &request_provider, &api_error)
                {
                    if !tried_payload_modes.contains(&next_mode) {
                        payload_mode = next_mode;
                        tried_payload_modes.push(next_mode);
                        continue;
                    }
                }

                let status_code = status.as_u16();
                if should_fallback_responses_to_chat_completions(
                    &request_provider,
                    status_code,
                    &api_error,
                ) {
                    if let Some(fallback_provider) = request_provider.responses_fallback_provider()
                    {
                        request_provider = fallback_provider;
                        payload_mode = CompletionPayloadMode::default_for(&request_provider);
                        tried_payload_modes = vec![payload_mode];
                        include_tool_schema = !tool_definitions.is_empty();
                        continue;
                    }
                }
                if attempt < request_policy.max_attempts && policy::should_retry_status(status_code)
                {
                    sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = policy::next_backoff_ms(backoff_ms, request_policy.max_backoff_ms);
                    continue;
                }

                if auto_model_mode && should_try_next_model_on_error(&api_error) {
                    return Err(ModelRequestError {
                        message: format!(
                            "model `{model}` rejected by provider endpoint; trying next candidate. status {status_code}: {response_body}"
                        ),
                        try_next_model: true,
                    });
                }

                return Err(ModelRequestError {
                    message: format!(
                        "provider returned status {status_code} for model `{model}` on attempt {attempt}/{max_attempts}: {response_body}",
                        max_attempts = request_policy.max_attempts
                    ),
                    try_next_model: false,
                });
            }
            Err(error) => {
                if attempt < request_policy.max_attempts && policy::should_retry_error(&error) {
                    sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = policy::next_backoff_ms(backoff_ms, request_policy.max_backoff_ms);
                    continue;
                }
                return Err(ModelRequestError {
                    message: format!(
                        "provider request failed for model `{model}` on attempt {attempt}/{max_attempts}: {error}",
                        max_attempts = request_policy.max_attempts
                    ),
                    try_next_model: false,
                });
            }
        }
    }
}

fn validate_provider_feature_gate(config: &LoongClawConfig) -> CliResult<()> {
    match config.provider.kind {
        ProviderKind::Volcengine => {
            if !cfg!(feature = "provider-volcengine") {
                return Err(
                    "volcengine provider is disabled (enable feature `provider-volcengine`)"
                        .to_owned(),
                );
            }
        }
        _ => {
            if !cfg!(feature = "provider-openai") {
                return Err(
                    "openai-compatible provider family is disabled (enable feature `provider-openai`)"
                        .to_owned(),
                );
            }
        }
    }
    Ok(())
}

fn validate_provider_configuration(config: &LoongClawConfig) -> CliResult<()> {
    if config.provider.kind == ProviderKind::Kimi
        && provider_uses_kimi_coding_endpoint(&config.provider)
    {
        return Err(
            "kimi provider cannot target Kimi Coding endpoints; use `kind = \"kimi_coding\"`"
                .to_owned(),
        );
    }

    if config.provider.kind == ProviderKind::KimiCoding {
        if config.provider.wire_api == ProviderWireApi::Responses {
            return Err(
                "kimi_coding provider currently supports only `wire_api = \"chat_completions\"`"
                    .to_owned(),
            );
        }
        if let Some(user_agent) = config.provider.header_value("user-agent") {
            if !is_kimi_cli_user_agent(user_agent) {
                return Err(format!(
                    "kimi_coding provider requires a `User-Agent` header starting with `KimiCLI/`; got `{user_agent}`"
                ));
            }
        }
    }

    Ok(())
}

fn provider_uses_kimi_coding_endpoint(provider: &ProviderConfig) -> bool {
    is_kimi_coding_endpoint(provider.endpoint().as_str())
        || provider
            .endpoint
            .as_deref()
            .is_some_and(is_kimi_coding_endpoint)
}

fn is_kimi_coding_endpoint(endpoint: &str) -> bool {
    endpoint
        .trim()
        .to_ascii_lowercase()
        .contains("://api.kimi.com/coding/")
}

fn is_kimi_cli_user_agent(user_agent: &str) -> bool {
    user_agent.trim().starts_with("KimiCLI/")
}

fn kimi_extra_body(reasoning_effort: Option<ReasoningEffort>) -> Option<Value> {
    let reasoning_effort = reasoning_effort?;
    let thinking_type = match reasoning_effort {
        ReasoningEffort::None => "disabled",
        ReasoningEffort::Minimal
        | ReasoningEffort::Low
        | ReasoningEffort::Medium
        | ReasoningEffort::High
        | ReasoningEffort::Xhigh => "enabled",
    };
    Some(json!({
        "thinking": {
            "type": thinking_type
        }
    }))
}

pub async fn provider_auth_ready(config: &LoongClawConfig) -> bool {
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

    if config.provider.kind == crate::config::ProviderKind::Bedrock
        && let Ok(auth_context) = transport::resolve_request_auth_context(&config.provider).await
    {
        return auth_context.has_bedrock_sigv4_fallback();
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LoongClawConfig, ProviderConfig, ReasoningEffort};
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn test_config(provider: ProviderConfig) -> LoongClawConfig {
        LoongClawConfig {
            provider,
            ..LoongClawConfig::default()
        }
    }

    #[test]
    fn message_builder_includes_system_prompt() {
        let config = test_config(ProviderConfig::default());

        let messages =
            build_messages_for_session(&config, "noop-session", true).expect("build messages");
        assert!(!messages.is_empty());
        assert_eq!(messages[0]["role"], "system");
    }

    #[test]
    fn build_messages_includes_capability_snapshot_block() {
        let config = test_config(ProviderConfig::default());

        let messages =
            build_messages_for_session(&config, "noop-session", true).expect("build messages");
        assert!(!messages.is_empty());
        let system_content = messages[0]["content"].as_str().expect("system content");
        assert!(
            system_content.contains("[available_tools]"),
            "system prompt should contain capability snapshot marker, got: {system_content}"
        );
        assert!(
            system_content.contains("- shell.exec: Execute shell commands"),
            "system prompt should list shell.exec tool"
        );
        assert!(
            system_content.contains("- file.read: Read file contents"),
            "system prompt should list file.read tool"
        );
        assert!(
            system_content.contains("- file.write: Write file contents"),
            "system prompt should list file.write tool"
        );
    }

    #[test]
    fn completion_body_includes_reasoning_effort_when_configured() {
        let mut config = test_config(ProviderConfig::default());
        config.provider.reasoning_effort = Some(ReasoningEffort::High);

        let body = build_completion_request_body(
            &config,
            &[],
            "model-latest",
            CompletionPayloadMode::default_for(&config.provider),
        );
        assert_eq!(body["reasoning_effort"], "high");
    }

    #[test]
    fn responses_completion_body_uses_input_shape_and_responses_specific_fields() {
        let mut config = test_config(ProviderConfig {
            wire_api: crate::config::ProviderWireApi::Responses,
            max_tokens: Some(512),
            ..ProviderConfig::default()
        });
        config.provider.reasoning_effort = Some(ReasoningEffort::High);

        let body = build_completion_request_body(
            &config,
            &[
                json!({
                    "role": "system",
                    "content": "You are concise."
                }),
                json!({
                    "role": "user",
                    "content": "ping"
                }),
            ],
            "gpt-5.1-mini",
            CompletionPayloadMode::default_for(&config.provider),
        );
        assert_eq!(body["model"], "gpt-5.1-mini");
        assert_eq!(body["instructions"], "You are concise.");
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(body["input"][0]["content"][0]["text"], "ping");
        assert_eq!(body["max_output_tokens"], 512);
        assert_eq!(body["reasoning"]["effort"], "high");
        assert!(body.get("messages").is_none());
        assert!(body.get("max_completion_tokens").is_none());
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn kimi_coding_completion_body_adds_extra_body_thinking() {
        let mut config = test_config(ProviderConfig {
            kind: ProviderKind::KimiCoding,
            ..ProviderConfig::default()
        });
        config.provider.reasoning_effort = Some(ReasoningEffort::High);

        let body = build_completion_request_body(
            &config,
            &[],
            "kimi-for-coding",
            CompletionPayloadMode::default_for(&config.provider),
        );
        assert_eq!(body["reasoning_effort"], "high");
        assert_eq!(body["extra_body"]["thinking"]["type"], "enabled");
    }

    #[test]
    fn model_catalog_selection_prefers_user_preferences() {
        let config = ProviderConfig {
            model: "auto".to_owned(),
            preferred_models: vec!["model-latest".to_owned(), "model-fallback".to_owned()],
            ..ProviderConfig::default()
        };
        let ranked = rank_model_candidates(
            &config,
            &["model-fallback".to_owned(), "model-latest".to_owned()],
        );
        let selected = ranked.first().expect("model selected");
        assert_eq!(selected, "model-latest");
    }

    #[test]
    fn completion_body_omits_optional_fields_when_not_configured() {
        let config = test_config(ProviderConfig::default());

        let body = build_completion_request_body(
            &config,
            &[],
            "model-latest",
            CompletionPayloadMode::default_for(&config.provider),
        );
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("max_completion_tokens").is_none());
        assert!(body.get("reasoning").is_none());
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn kimi_coding_request_headers_include_default_user_agent() {
        let provider = ProviderConfig {
            kind: ProviderKind::KimiCoding,
            ..ProviderConfig::default()
        };
        let headers = transport::build_request_headers(&provider).expect("headers");
        let user_agent = headers
            .get(reqwest::header::USER_AGENT)
            .expect("default user-agent")
            .to_str()
            .expect("user-agent value");
        assert_eq!(user_agent, "KimiCLI/LoongClaw");
    }

    #[test]
    fn kimi_coding_keeps_explicit_compatible_user_agent() {
        let provider = ProviderConfig {
            kind: ProviderKind::KimiCoding,
            headers: [("User-Agent".to_owned(), "KimiCLI/custom".to_owned())]
                .into_iter()
                .collect(),
            ..ProviderConfig::default()
        };
        let headers = transport::build_request_headers(&provider).expect("headers");
        let user_agent = headers
            .get(reqwest::header::USER_AGENT)
            .expect("explicit user-agent")
            .to_str()
            .expect("user-agent value");
        assert_eq!(user_agent, "KimiCLI/custom");
    }

    #[cfg(any(feature = "tool-file", feature = "tool-shell"))]
    #[test]
    fn turn_body_includes_tool_schema_and_auto_choice() {
        let config = test_config(ProviderConfig::default());

        let body = build_turn_request_body(
            &config,
            &[],
            "model-latest",
            CompletionPayloadMode::default_for(&config.provider),
            true,
            &crate::tools::provider_tool_definitions(),
        );
        let tools = body
            .get("tools")
            .and_then(|value| value.as_array())
            .expect("tools array in turn body");
        assert!(!tools.is_empty());
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|item| item.get("function"))
            .filter_map(|function| function.get("name"))
            .filter_map(Value::as_str)
            .collect();
        let expected: Vec<String> = crate::tools::provider_tool_definitions()
            .into_iter()
            .filter_map(|item| item.get("function").cloned())
            .filter_map(|function| function.get("name").cloned())
            .filter_map(|name| name.as_str().map(str::to_owned))
            .collect();

        assert_eq!(names, expected);
        assert_eq!(body["tool_choice"], "auto");
    }

    #[cfg(any(feature = "tool-file", feature = "tool-shell"))]
    #[test]
    fn responses_turn_body_keeps_tool_schema_with_responses_input_shape() {
        let config = test_config(ProviderConfig {
            wire_api: crate::config::ProviderWireApi::Responses,
            ..ProviderConfig::default()
        });

        let body = build_turn_request_body(
            &config,
            &[json!({
                "role": "user",
                "content": "read README"
            })],
            "gpt-5.1-mini",
            CompletionPayloadMode::default_for(&config.provider),
            true,
            &crate::tools::provider_tool_definitions(),
        );

        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"][0]["text"], "read README");
        assert!(body.get("messages").is_none());
        assert_eq!(body["tool_choice"], "auto");
        assert!(
            body.get("tools")
                .and_then(Value::as_array)
                .is_some_and(|tools| !tools.is_empty()),
            "responses requests should still carry tool definitions"
        );
    }

    #[test]
    fn responses_turn_body_preserves_native_function_call_roundtrip_items() {
        let config = test_config(ProviderConfig {
            wire_api: crate::config::ProviderWireApi::Responses,
            ..ProviderConfig::default()
        });

        let body = build_turn_request_body(
            &config,
            &[
                json!({
                    "role": "assistant",
                    "content": "Reading the file now."
                }),
                json!({
                    "type": "function_call",
                    "name": "file_read",
                    "call_id": "call_resp_1",
                    "arguments": "{\"path\":\"README.md\"}"
                }),
                json!({
                    "type": "function_call_output",
                    "call_id": "call_resp_1",
                    "output": "[ok] {\"path\":\"README.md\"}"
                }),
                json!({
                    "role": "user",
                    "content": "Use the tool result above to answer the original request."
                }),
            ],
            "gpt-5.1-mini",
            CompletionPayloadMode::default_for(&config.provider),
            true,
            &crate::tools::provider_tool_definitions(),
        );

        let input = body["input"].as_array().expect("responses input array");
        assert!(
            input.iter().any(|item| {
                item.get("type").and_then(Value::as_str) == Some("function_call")
                    && item.get("call_id").and_then(Value::as_str) == Some("call_resp_1")
            }),
            "responses input should preserve function_call items, got: {input:?}"
        );
        assert!(
            input.iter().any(|item| {
                item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    && item.get("call_id").and_then(Value::as_str) == Some("call_resp_1")
                    && item.get("output").and_then(Value::as_str)
                        == Some("[ok] {\"path\":\"README.md\"}")
            }),
            "responses input should preserve function_call_output items, got: {input:?}"
        );
    }

    #[test]
    fn tool_schema_fallback_detects_unsupported_error_shapes() {
        let unsupported_tools = json!({
            "error": {
                "code": "unsupported_parameter",
                "param": "tools",
                "message": "Unsupported parameter: tools"
            }
        });
        let unsupported_tool_choice = json!({
            "error": {
                "message": "Function calling is not supported for this model."
            }
        });

        assert!(should_disable_tool_schema_for_error(
            &parse_provider_api_error(&unsupported_tools)
        ));
        assert!(should_disable_tool_schema_for_error(
            &parse_provider_api_error(&unsupported_tool_choice)
        ));
    }

    #[test]
    fn completion_body_uses_provider_token_field_default() {
        let openai = ProviderConfig {
            kind: ProviderKind::Openai,
            max_tokens: Some(512),
            ..ProviderConfig::default()
        };
        let openai_mode = CompletionPayloadMode::default_for(&openai);
        assert_eq!(
            openai_mode.token_field,
            TokenLimitField::MaxCompletionTokens
        );

        let openrouter = ProviderConfig {
            kind: ProviderKind::Openrouter,
            max_tokens: Some(512),
            ..ProviderConfig::default()
        };
        let openrouter_mode = CompletionPayloadMode::default_for(&openrouter);
        assert_eq!(openrouter_mode.token_field, TokenLimitField::MaxTokens);
    }

    #[test]
    fn payload_mode_adapts_for_parameter_incompatibility() {
        let provider = ProviderConfig {
            max_tokens: Some(1024),
            reasoning_effort: Some(ReasoningEffort::Medium),
            ..ProviderConfig::default()
        };

        let max_tokens_error = json!({
            "error": {
                "code": "unsupported_parameter",
                "param": "max_tokens",
                "message": "Unsupported parameter: 'max_tokens'. Use 'max_completion_tokens' instead."
            }
        });
        let reasoning_effort_error = json!({
            "error": {
                "code": "unknown_parameter",
                "param": "reasoning_effort",
                "message": "Unknown parameter: 'reasoning_effort'."
            }
        });

        let mut mode = CompletionPayloadMode {
            token_field: TokenLimitField::MaxTokens,
            reasoning_field: ReasoningField::ReasoningEffort,
            temperature_field: TemperatureField::Include,
        };

        mode = adapt_payload_mode_for_error(
            mode,
            &provider,
            &parse_provider_api_error(&max_tokens_error),
        )
        .expect("max_tokens adapt");
        assert_eq!(mode.token_field, TokenLimitField::MaxCompletionTokens);

        mode = adapt_payload_mode_for_error(
            mode,
            &provider,
            &parse_provider_api_error(&reasoning_effort_error),
        )
        .expect("reasoning adapt");
        assert_eq!(mode.reasoning_field, ReasoningField::ReasoningObject);
    }

    #[test]
    fn payload_mode_can_drop_temperature_when_model_rejects_it() {
        let provider = ProviderConfig::default();
        let unsupported_temperature = json!({
            "error": {
                "code": "unsupported_value",
                "param": "temperature",
                "message": "Only the default (1) value is supported."
            }
        });

        let mode = CompletionPayloadMode::default_for(&provider);
        let adapted = adapt_payload_mode_for_error(
            mode,
            &provider,
            &parse_provider_api_error(&unsupported_temperature),
        )
        .expect("temperature adaptation");
        assert_eq!(adapted.temperature_field, TemperatureField::Omit);
    }

    #[test]
    fn ranking_model_candidates_keeps_preferences_then_catalog() {
        let config = ProviderConfig {
            model: "auto".to_owned(),
            preferred_models: vec![
                "model-z".to_owned(),
                "MODEL-A".to_owned(),
                "model-z".to_owned(),
            ],
            ..ProviderConfig::default()
        };

        let ranked = rank_model_candidates(
            &config,
            &[
                "model-a".to_owned(),
                "model-b".to_owned(),
                "model-z".to_owned(),
            ],
        );
        assert_eq!(ranked, vec!["model-z", "model-a", "model-b"]);
    }

    #[test]
    fn model_error_parser_detects_endpoint_mismatch() {
        let body = json!({
            "error": {
                "message": "The model `gpt-5.4-pro` only supports /v1/responses and not this endpoint."
            }
        });
        let parsed = parse_provider_api_error(&body);
        assert!(should_try_next_model_on_error(&parsed));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn responses_completion_falls_back_to_chat_completions_for_compatible_endpoints() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider listener");
        let addr = listener.local_addr().expect("local addr");
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().expect("accept local provider request");
                let mut request_buf = [0_u8; 8192];
                let len = stream.read(&mut request_buf).expect("read request");
                let request = String::from_utf8_lossy(&request_buf[..len]).to_string();
                requests.push(request.clone());

                let (status_line, body) = if request.starts_with("POST /v1/responses ") {
                    (
                        "HTTP/1.1 400 Bad Request",
                        r#"{"error":{"code":"unsupported_parameter","param":"input","message":"This compatibility endpoint expects `messages`; unknown parameter `input`. Retry with /v1/chat/completions."}}"#.to_owned(),
                    )
                } else if request.starts_with("POST /v1/chat/completions ") {
                    (
                        "HTTP/1.1 200 OK",
                        r#"{"choices":[{"message":{"role":"assistant","content":"fallback ok"}}]}"#
                            .to_owned(),
                    )
                } else {
                    (
                        "HTTP/1.1 404 Not Found",
                        r#"{"error":{"message":"unexpected request"}}"#.to_owned(),
                    )
                };

                let response = format!(
                    "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write response");
            }
            requests
        });

        let config = test_config(ProviderConfig {
            kind: ProviderKind::Deepseek,
            base_url: format!("http://{addr}"),
            model: "deepseek-chat".to_owned(),
            wire_api: ProviderWireApi::Responses,
            api_key: Some("deepseek-test-key".to_owned()),
            ..ProviderConfig::default()
        });

        let completion = request_completion(
            &config,
            &[json!({
                "role": "user",
                "content": "ping"
            })],
            None,
        )
        .await
        .expect("compatible responses transport should retry chat-completions automatically");
        assert_eq!(completion, "fallback ok");

        let requests = server.join().expect("join local provider server");
        assert!(
            requests.iter().any(|request| {
                request.starts_with("POST /v1/responses ")
                    && request.contains("\"input\"")
                    && !request.contains("\"messages\"")
            }),
            "first attempt should use Responses input shape: {requests:#?}"
        );
        assert!(
            requests.iter().any(|request| {
                request.starts_with("POST /v1/chat/completions ")
                    && request.contains("\"messages\"")
                    && !request.contains("\"input\"")
            }),
            "fallback attempt should switch to chat-completions payload shape: {requests:#?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn responses_turn_falls_back_to_chat_completions_for_compatible_endpoints() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider listener");
        let addr = listener.local_addr().expect("local addr");
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().expect("accept local provider request");
                let mut request_buf = [0_u8; 8192];
                let len = stream.read(&mut request_buf).expect("read request");
                let request = String::from_utf8_lossy(&request_buf[..len]).to_string();
                requests.push(request.clone());

                let (status_line, body) = if request.starts_with("POST /v1/responses ") {
                    (
                        "HTTP/1.1 422 Unprocessable Entity",
                        r#"{"error":{"code":"invalid_request_error","param":"input","message":"Missing required parameter: `messages`. This provider expects /v1/chat/completions instead of Responses input."}}"#.to_owned(),
                    )
                } else if request.starts_with("POST /v1/chat/completions ") {
                    (
                        "HTTP/1.1 200 OK",
                        r#"{"choices":[{"message":{"role":"assistant","content":"turn fallback ok"}}]}"#
                            .to_owned(),
                    )
                } else {
                    (
                        "HTTP/1.1 404 Not Found",
                        r#"{"error":{"message":"unexpected request"}}"#.to_owned(),
                    )
                };

                let response = format!(
                    "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write response");
            }
            requests
        });

        let config = test_config(ProviderConfig {
            kind: ProviderKind::Deepseek,
            base_url: format!("http://{addr}"),
            model: "deepseek-chat".to_owned(),
            wire_api: ProviderWireApi::Responses,
            api_key: Some("deepseek-test-key".to_owned()),
            ..ProviderConfig::default()
        });

        let turn = request_turn(
            &config,
            &[json!({
                "role": "user",
                "content": "turn ping"
            })],
            None,
        )
        .await
        .expect("turn requests should retry chat-completions when Responses is rejected");
        assert_eq!(turn.assistant_text, "turn fallback ok");

        let requests = server.join().expect("join local provider server");
        assert!(
            requests.iter().any(|request| {
                request.starts_with("POST /v1/responses ")
                    && request.contains("\"input\"")
                    && request.contains("\"tools\"")
            }),
            "turn flow should first attempt Responses with tool schema: {requests:#?}"
        );
        assert!(
            requests.iter().any(|request| {
                request.starts_with("POST /v1/chat/completions ")
                    && request.contains("\"messages\"")
                    && request.contains("\"tools\"")
            }),
            "turn flow fallback should preserve tool schema on chat-completions: {requests:#?}"
        );
    }

    #[test]
    fn validate_provider_configuration_rejects_plain_kimi_on_coding_endpoint() {
        let config = test_config(ProviderConfig {
            kind: ProviderKind::Kimi,
            base_url: "https://api.kimi.com/coding".to_owned(),
            chat_completions_path: "/v1/chat/completions".to_owned(),
            ..ProviderConfig::default()
        });
        let error = validate_provider_configuration(&config).expect_err("misconfig should fail");
        assert!(error.contains("use `kind = \"kimi_coding\"`"));
    }

    #[test]
    fn validate_provider_configuration_rejects_incompatible_kimi_coding_user_agent() {
        let config = test_config(ProviderConfig {
            kind: ProviderKind::KimiCoding,
            headers: [("User-Agent".to_owned(), "LoongClaw/0.1".to_owned())]
                .into_iter()
                .collect(),
            ..ProviderConfig::default()
        });
        let error = validate_provider_configuration(&config).expect_err("invalid ua");
        assert!(error.contains("KimiCLI/"));
    }

    #[test]
    fn validate_provider_configuration_rejects_kimi_coding_responses_wire_api() {
        let config = test_config(ProviderConfig {
            kind: ProviderKind::KimiCoding,
            wire_api: ProviderWireApi::Responses,
            ..ProviderConfig::default()
        });
        let error = validate_provider_configuration(&config).expect_err("invalid wire api");
        assert!(error.contains("chat_completions"));
    }
}
