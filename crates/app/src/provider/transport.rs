use reqwest::header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use serde_json::{Value, json};

use crate::CliResult;
use crate::config::ProviderConfig;

pub(super) fn build_request_headers(provider: &ProviderConfig) -> CliResult<HeaderMap> {
    let mut headers = HeaderMap::new();
    for (key, value) in &provider.headers {
        let name = HeaderName::from_bytes(key.as_bytes())
            .map_err(|error| format!("invalid provider header name `{key}`: {error}"))?;
        let header_value = HeaderValue::from_str(value)
            .map_err(|error| format!("invalid provider header value for `{key}`: {error}"))?;
        headers.insert(name, header_value);
    }
    if !headers.contains_key(USER_AGENT)
        && let Some(default_user_agent) = provider.kind.default_user_agent()
    {
        let header_value = HeaderValue::from_str(default_user_agent).map_err(|error| {
            format!("invalid default provider user-agent `{default_user_agent}`: {error}")
        })?;
        headers.insert(USER_AGENT, header_value);
    }
    Ok(headers)
}

pub(super) async fn decode_response_body(response: reqwest::Response) -> CliResult<Value> {
    let status = response.status().as_u16();
    let content_encoding = response
        .headers()
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("none")
        .to_owned();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("none")
        .to_owned();
    let bytes = response.bytes().await.map_err(|error| {
        format!(
            "read response body failed: {error} [status={status}, content-encoding={content_encoding}, content-type={content_type}]"
        )
    })?;
    if bytes.is_empty() {
        return Ok(json!({}));
    }
    let text = String::from_utf8_lossy(&bytes);
    Ok(serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({"raw_body": text.as_ref()})))
}
