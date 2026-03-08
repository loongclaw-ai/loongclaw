use std::time::Duration;

use serde_json::{json, Value};
use tokio::time::sleep;

use crate::CliResult;

use super::config::{LoongClawConfig, ProviderConfig, ProviderKind};
#[cfg(feature = "memory-sqlite")]
use super::memory;

mod policy;
mod shape;
mod transport;

pub fn build_messages_for_session(
    config: &LoongClawConfig,
    session_id: &str,
    include_system_prompt: bool,
) -> CliResult<Vec<Value>> {
    let mut messages = Vec::new();
    if include_system_prompt {
        let system = config.cli.system_prompt.trim();
        if !system.is_empty() {
            messages.push(json!({
                "role": "system",
                "content": system,
            }));
        }
    }

    #[cfg(feature = "memory-sqlite")]
    {
        let turns = memory::window_direct(session_id, config.memory.sliding_window)
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

pub async fn request_completion(config: &LoongClawConfig, messages: &[Value]) -> CliResult<String> {
    validate_provider_feature_gate(config)?;

    let endpoint = config.provider.endpoint();
    let headers = transport::build_request_headers(&config.provider.headers)?;
    let request_policy = policy::ProviderRequestPolicy::from_config(&config.provider);
    let resolved_model = resolve_request_model(config, &headers, &request_policy).await?;

    let body = build_completion_request_body(config, messages, &resolved_model);
    let client = build_http_client(&request_policy)?;

    let mut attempt = 0usize;
    let mut backoff_ms = request_policy.initial_backoff_ms;
    loop {
        attempt += 1;
        let mut req = client
            .post(endpoint.clone())
            .headers(headers.clone())
            .json(&body);
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
                            "provider response decode failed on attempt {attempt}/{max_attempts}: {error}",
                            max_attempts = request_policy.max_attempts
                        )
                    })?;

                if status.is_success() {
                    let content = shape::extract_message_content(&response_body).ok_or_else(|| {
                        format!(
                            "provider response missing choices[0].message.content on attempt {attempt}/{max_attempts}: {response_body}",
                            max_attempts = request_policy.max_attempts
                        )
                    })?;
                    return Ok(content);
                }

                let status_code = status.as_u16();
                if attempt < request_policy.max_attempts && policy::should_retry_status(status_code)
                {
                    sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = policy::next_backoff_ms(backoff_ms, request_policy.max_backoff_ms);
                    continue;
                }

                return Err(format!(
                    "provider returned status {status_code} on attempt {attempt}/{max_attempts}: {response_body}",
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
                    "provider request failed on attempt {attempt}/{max_attempts}: {error}",
                    max_attempts = request_policy.max_attempts
                ));
            }
        }
    }
}

pub async fn fetch_available_models(config: &LoongClawConfig) -> CliResult<Vec<String>> {
    validate_provider_feature_gate(config)?;
    let headers = transport::build_request_headers(&config.provider.headers)?;
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
) -> Value {
    let mut body = json!({
        "model": model,
        "messages": messages,
        "temperature": config.provider.temperature,
        "max_tokens": config.provider.max_tokens,
        "stream": false,
    });

    if let Some(reasoning_effort) = config.provider.reasoning_effort {
        if let Some(map) = body.as_object_mut() {
            map.insert(
                "reasoning".to_owned(),
                json!({
                    "effort": reasoning_effort.as_str()
                }),
            );
        }
    }

    body
}

async fn resolve_request_model(
    config: &LoongClawConfig,
    headers: &reqwest::header::HeaderMap,
    request_policy: &policy::ProviderRequestPolicy,
) -> CliResult<String> {
    if !config.provider.model_selection_requires_fetch() {
        return Ok(config.provider.model.trim().to_owned());
    }
    let available = fetch_available_models_with_policy(config, headers, request_policy).await?;
    select_model_from_catalog(&config.provider, &available)
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

fn select_model_from_catalog(provider: &ProviderConfig, available: &[String]) -> CliResult<String> {
    if available.is_empty() {
        return Err("provider model-list is empty; set provider.model explicitly".to_owned());
    }

    let mut preferred = Vec::new();
    for raw in &provider.preferred_models {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if preferred
            .iter()
            .any(|existing: &String| existing == trimmed)
        {
            continue;
        }
        preferred.push(trimmed.to_owned());
    }

    for candidate in &preferred {
        if let Some(matched) = available.iter().find(|model| *model == candidate) {
            return Ok(matched.clone());
        }
    }
    for candidate in &preferred {
        if let Some(matched) = available
            .iter()
            .find(|model| model.eq_ignore_ascii_case(candidate))
        {
            return Ok(matched.clone());
        }
    }

    Ok(available[0].clone())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mvp::config::{
        FeishuChannelConfig, MemoryConfig, ProviderConfig, ReasoningEffort, ToolConfig,
    };

    #[test]
    fn message_builder_includes_system_prompt() {
        let config = LoongClawConfig {
            provider: ProviderConfig::default(),
            cli: crate::mvp::config::CliChannelConfig::default(),
            telegram: crate::mvp::config::TelegramChannelConfig::default(),
            feishu: FeishuChannelConfig::default(),
            tools: ToolConfig::default(),
            memory: MemoryConfig::default(),
        };

        let messages =
            build_messages_for_session(&config, "noop-session", true).expect("build messages");
        assert!(!messages.is_empty());
        assert_eq!(messages[0]["role"], "system");
    }

    #[test]
    fn completion_body_includes_reasoning_effort_when_configured() {
        let mut config = LoongClawConfig {
            provider: ProviderConfig::default(),
            cli: crate::mvp::config::CliChannelConfig::default(),
            telegram: crate::mvp::config::TelegramChannelConfig::default(),
            feishu: FeishuChannelConfig::default(),
            tools: ToolConfig::default(),
            memory: MemoryConfig::default(),
        };
        config.provider.reasoning_effort = Some(ReasoningEffort::High);

        let body = build_completion_request_body(&config, &[], "model-latest");
        assert_eq!(body["reasoning"]["effort"], "high");
    }

    #[test]
    fn model_catalog_selection_prefers_user_preferences() {
        let config = ProviderConfig {
            model: "auto".to_owned(),
            preferred_models: vec!["model-latest".to_owned(), "model-fallback".to_owned()],
            ..ProviderConfig::default()
        };
        let selected = select_model_from_catalog(
            &config,
            &["model-fallback".to_owned(), "model-latest".to_owned()],
        )
        .expect("model selected");
        assert_eq!(selected, "model-latest");
    }
}
