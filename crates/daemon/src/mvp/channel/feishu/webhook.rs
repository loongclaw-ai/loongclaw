use std::{
    collections::{BTreeSet, VecDeque},
    sync::Arc,
};

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::mvp::channel::{process_inbound_with_provider, ChannelInboundMessage};
use crate::mvp::config::LoongClawConfig;

use super::adapter::FeishuAdapter;
use super::payload::FeishuWebhookAction;

#[derive(Clone)]
pub(super) struct FeishuWebhookState {
    config: LoongClawConfig,
    adapter: Arc<Mutex<FeishuAdapter>>,
    verification_token: Option<String>,
    allowed_chat_ids: BTreeSet<String>,
    ignore_bot_messages: bool,
    seen_events: Arc<Mutex<RecentIdCache>>,
}

impl FeishuWebhookState {
    pub(super) fn new(config: LoongClawConfig, adapter: FeishuAdapter) -> Self {
        Self {
            verification_token: config.feishu.verification_token(),
            allowed_chat_ids: config
                .feishu
                .allowed_chat_ids
                .iter()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .collect(),
            ignore_bot_messages: config.feishu.ignore_bot_messages,
            config,
            adapter: Arc::new(Mutex::new(adapter)),
            seen_events: Arc::new(Mutex::new(RecentIdCache::new(2_048))),
        }
    }
}

struct RecentIdCache {
    max_len: usize,
    queue: VecDeque<String>,
    set: BTreeSet<String>,
}

impl RecentIdCache {
    fn new(max_len: usize) -> Self {
        Self {
            max_len: max_len.max(1),
            queue: VecDeque::new(),
            set: BTreeSet::new(),
        }
    }

    fn insert_if_new(&mut self, id: &str) -> bool {
        let id = id.trim();
        if id.is_empty() {
            return false;
        }
        if self.set.contains(id) {
            return false;
        }

        self.queue.push_back(id.to_owned());
        self.set.insert(id.to_owned());
        while self.queue.len() > self.max_len {
            if let Some(removed) = self.queue.pop_front() {
                self.set.remove(&removed);
            }
        }
        true
    }
}

pub(super) async fn feishu_webhook_handler(
    State(state): State<FeishuWebhookState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match handle_feishu_webhook_payload(state, payload).await {
        Ok(reply) => (StatusCode::OK, Json(reply)),
        Err((status, message)) => (
            status,
            Json(json!({
                "code": status.as_u16(),
                "msg": message,
            })),
        ),
    }
}

async fn handle_feishu_webhook_payload(
    state: FeishuWebhookState,
    payload: Value,
) -> Result<Value, (StatusCode, String)> {
    let parsed = super::payload::parse_feishu_webhook_payload(
        &payload,
        state.verification_token.as_deref(),
        &state.allowed_chat_ids,
        state.ignore_bot_messages,
    )
    .map_err(map_feishu_parse_error)?;

    match parsed {
        FeishuWebhookAction::UrlVerification { challenge } => Ok(json!({ "challenge": challenge })),
        FeishuWebhookAction::Ignore => Ok(json!({"code": 0, "msg": "ignored"})),
        FeishuWebhookAction::Inbound(event) => {
            {
                let mut dedupe = state.seen_events.lock().await;
                if !dedupe.insert_if_new(&event.event_id) {
                    return Ok(json!({"code": 0, "msg": "duplicate_event"}));
                }
            }

            let channel_message = ChannelInboundMessage {
                session_id: event.session_id,
                reply_target: event.message_id.clone(),
                text: event.text,
            };
            let reply = process_inbound_with_provider(&state.config, &channel_message)
                .await
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("provider processing failed: {error}"),
                    )
                })?;

            let mut adapter = state.adapter.lock().await;
            if let Err(first_error) = adapter.send_reply(&event.message_id, &reply).await {
                adapter.refresh_tenant_token().await.map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!(
                            "feishu token refresh failed after send error `{first_error}`: {error}"
                        ),
                    )
                })?;
                adapter
                    .send_reply(&event.message_id, &reply)
                    .await
                    .map_err(|error| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("feishu reply failed after token refresh: {error}"),
                        )
                    })?;
            }

            Ok(json!({"code": 0, "msg": "ok"}))
        }
    }
}

fn map_feishu_parse_error(error: String) -> (StatusCode, String) {
    if let Some(message) = error.strip_prefix("unauthorized:") {
        return (StatusCode::UNAUTHORIZED, message.trim().to_owned());
    }
    (StatusCode::BAD_REQUEST, error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_cache_deduplicates_and_rolls_window() {
        let mut cache = RecentIdCache::new(2);
        assert!(cache.insert_if_new("a"));
        assert!(!cache.insert_if_new("a"));
        assert!(cache.insert_if_new("b"));
        assert!(cache.insert_if_new("c"));
        assert!(cache.insert_if_new("a"));
    }
}
