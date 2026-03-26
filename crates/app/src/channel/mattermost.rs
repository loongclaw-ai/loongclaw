use serde_json::{Value, json};

use crate::{CliResult, config::ResolvedMattermostChannelConfig};

use super::ChannelOutboundTargetKind;

pub(super) async fn run_mattermost_send(
    resolved: &ResolvedMattermostChannelConfig,
    target_kind: ChannelOutboundTargetKind,
    target_id: &str,
    text: &str,
) -> CliResult<()> {
    if target_kind != ChannelOutboundTargetKind::Conversation {
        return Err(format!(
            "mattermost send requires conversation target kind, got {}",
            target_kind.as_str()
        ));
    }

    let server_url = resolved.server_url().ok_or_else(|| {
        "mattermost server_url missing (set mattermost.server_url or env)".to_owned()
    })?;
    let bot_token = resolved.bot_token().ok_or_else(|| {
        "mattermost bot_token missing (set mattermost.bot_token or env)".to_owned()
    })?;
    let channel_id = target_id.trim();
    if channel_id.is_empty() {
        return Err("mattermost outbound target id is empty".to_owned());
    }

    let trimmed_server_url = server_url.trim_end_matches('/');
    let request_url = format!("{trimmed_server_url}/api/v4/posts");
    let request_body = json!({
        "channel_id": channel_id,
        "message": text,
    });

    let client = reqwest::Client::new();
    let request = client
        .post(request_url.as_str())
        .bearer_auth(bot_token)
        .json(&request_body);
    let response = request
        .send()
        .await
        .map_err(|error| format!("mattermost send failed: {error}"))?;
    let payload = read_mattermost_json_response(response).await?;

    let message_id = payload
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if message_id.is_none() {
        return Err(format!(
            "mattermost send did not return a post id: {payload}"
        ));
    }

    Ok(())
}

async fn read_mattermost_json_response(response: reqwest::Response) -> CliResult<Value> {
    let status = response.status();
    let payload = response
        .json::<Value>()
        .await
        .map_err(|error| format!("decode mattermost send response failed: {error}"))?;

    if status.is_success() {
        return Ok(payload);
    }

    let detail = payload
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| payload.to_string());

    Err(format!(
        "mattermost send failed with status {}: {detail}",
        status.as_u16()
    ))
}
