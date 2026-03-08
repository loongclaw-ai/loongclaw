use std::time::Duration;

use serde_json::{json, Value};
use tokio::time::sleep;

use crate::CliResult;

use super::config::{LoongClawConfig, ProviderKind};
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

    let body = json!({
        "model": config.provider.model,
        "messages": messages,
        "temperature": config.provider.temperature,
        "max_tokens": config.provider.max_tokens,
        "stream": false,
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(request_policy.timeout_ms))
        .build()
        .map_err(|error| format!("build provider http client failed: {error}"))?;

    let mut attempt = 0usize;
    let mut backoff_ms = request_policy.initial_backoff_ms;
    loop {
        attempt += 1;
        let mut req = client
            .post(endpoint.clone())
            .headers(headers.clone())
            .json(&body);
        if let Some(api_key) = config.provider.api_key() {
            req = req.bearer_auth(api_key);
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

fn validate_provider_feature_gate(config: &LoongClawConfig) -> CliResult<()> {
    match config.provider.kind {
        ProviderKind::OpenaiCompatible => {
            if !cfg!(feature = "provider-openai") {
                return Err(
                    "openai-compatible provider is disabled (enable feature `provider-openai`)"
                        .to_owned(),
                );
            }
        }
        ProviderKind::VolcengineCustom => {
            if !cfg!(feature = "provider-volcengine") {
                return Err(
                    "volcengine custom provider is disabled (enable feature `provider-volcengine`)"
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
    use crate::mvp::config::{FeishuChannelConfig, MemoryConfig, ProviderConfig, ToolConfig};

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
}
