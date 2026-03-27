use serde_json::{Value, json};

use crate::{CliResult, config::ResolvedZaloChannelConfig};

use super::{
    ChannelOutboundTargetKind,
    http::{build_outbound_http_client, read_json_or_text_response, response_body_detail},
};

pub(super) async fn run_zalo_send(
    resolved: &ResolvedZaloChannelConfig,
    target_kind: ChannelOutboundTargetKind,
    target_id: &str,
    text: &str,
) -> CliResult<()> {
    if target_kind != ChannelOutboundTargetKind::Address {
        return Err(format!(
            "zalo send requires address target kind, got {}",
            target_kind.as_str()
        ));
    }

    let _ = resolved
        .app_id()
        .ok_or_else(|| "zalo app_id missing (set zalo.app_id or env)".to_owned())?;
    let oa_access_token = resolved.oa_access_token().ok_or_else(|| {
        "zalo oa_access_token missing (set zalo.oa_access_token or env)".to_owned()
    })?;
    let recipient = target_id.trim();
    if recipient.is_empty() {
        return Err("zalo outbound target id is empty".to_owned());
    }

    let api_base_url = resolved.resolved_api_base_url();
    let trimmed_api_base_url = api_base_url.trim_end_matches('/');
    let request_url = format!("{trimmed_api_base_url}/message/cs");
    let request_body = json!({
        "recipient": {
            "user_id": recipient,
        },
        "message": {
            "text": text,
        },
    });

    let client = build_outbound_http_client("zalo send")?;
    let request = client
        .post(request_url.as_str())
        .header("access_token", oa_access_token)
        .json(&request_body);
    let response = request
        .send()
        .await
        .map_err(|error| format!("zalo send failed: {error}"))?;

    ensure_zalo_send_success(response).await
}

async fn ensure_zalo_send_success(response: reqwest::Response) -> CliResult<()> {
    let (status, body, payload) = read_json_or_text_response(response, "zalo send").await?;

    if !status.is_success() {
        let detail = zalo_response_detail(&payload, body.as_str());
        return Err(format!(
            "zalo send failed with status {}: {detail}",
            status.as_u16()
        ));
    }

    if !payload.is_object() {
        let detail = response_body_detail(body.as_str());
        return Err(format!(
            "zalo send returned a non-json success payload: {detail}"
        ));
    }

    let error_code = payload
        .get("error")
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("zalo send success payload did not include error: {payload}"))?;
    if error_code != 0 {
        let detail = zalo_response_detail(&payload, body.as_str());
        return Err(format!("zalo send did not succeed: {detail}"));
    }

    let message_id = payload
        .get("data")
        .and_then(|data| data.get("message_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if message_id.is_none() {
        return Err(format!("zalo send did not return a message id: {payload}"));
    }

    Ok(())
}

fn zalo_response_detail(payload: &Value, body: &str) -> String {
    let message = payload
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    if let Some(message) = message {
        return message;
    }

    response_body_detail(body)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        Json, Router,
        extract::State,
        http::{HeaderMap, StatusCode},
        response::IntoResponse,
        routing::post,
    };
    use loongclaw_contracts::SecretRef;
    use tokio::net::TcpListener;

    use super::*;

    #[derive(Debug, Clone)]
    struct MockZaloState {
        access_tokens: Arc<Mutex<Vec<String>>>,
        request_bodies: Arc<Mutex<Vec<Value>>>,
        response_status: StatusCode,
        response_payload: Value,
    }

    impl Default for MockZaloState {
        fn default() -> Self {
            Self {
                access_tokens: Arc::new(Mutex::new(Vec::new())),
                request_bodies: Arc::new(Mutex::new(Vec::new())),
                response_status: StatusCode::OK,
                response_payload: json!({
                    "error": 0,
                    "message": "Success",
                    "data": {
                        "message_id": "msg-123",
                        "user_id": "user-456",
                    },
                }),
            }
        }
    }

    #[tokio::test]
    async fn run_zalo_send_posts_expected_request_and_accepts_success_payload() {
        let state = MockZaloState::default();
        let router = build_mock_zalo_router(state.clone());
        let (base_url, server) = spawn_mock_zalo_server(router).await;
        let resolved = build_resolved_zalo_config(Some(format!("{base_url}/v3.0/oa")));

        let send_result = run_zalo_send(
            &resolved,
            ChannelOutboundTargetKind::Address,
            "user-456",
            "hello from loongclaw",
        )
        .await;

        send_result.expect("zalo send should succeed");

        let access_tokens = state.access_tokens.lock().expect("access token log");
        assert_eq!(access_tokens.as_slice(), &[String::from("oa-access-token")]);

        let request_bodies = state.request_bodies.lock().expect("request body log");
        assert_eq!(request_bodies.len(), 1);
        assert_eq!(
            request_bodies[0],
            json!({
                "recipient": {
                    "user_id": "user-456",
                },
                "message": {
                    "text": "hello from loongclaw",
                },
            })
        );

        server.abort();
    }

    #[tokio::test]
    async fn run_zalo_send_reports_business_error_payloads() {
        let state = MockZaloState {
            response_payload: json!({
                "error": 216,
                "message": "User not found",
            }),
            ..MockZaloState::default()
        };
        let router = build_mock_zalo_router(state);
        let (base_url, server) = spawn_mock_zalo_server(router).await;
        let resolved = build_resolved_zalo_config(Some(format!("{base_url}/v3.0/oa")));

        let error = run_zalo_send(
            &resolved,
            ChannelOutboundTargetKind::Address,
            "missing-user",
            "hello from loongclaw",
        )
        .await
        .expect_err("business error payload should fail");

        assert_eq!(error, "zalo send did not succeed: User not found");

        server.abort();
    }

    #[tokio::test]
    async fn run_zalo_send_requires_address_target_kind() {
        let resolved = build_resolved_zalo_config(None);

        let error = run_zalo_send(
            &resolved,
            ChannelOutboundTargetKind::Conversation,
            "user-456",
            "hello from loongclaw",
        )
        .await
        .expect_err("conversation target kind should fail");

        assert_eq!(
            error,
            "zalo send requires address target kind, got conversation"
        );
    }

    #[tokio::test]
    async fn run_zalo_send_requires_message_id_in_success_payload() {
        let state = MockZaloState {
            response_payload: json!({
                "error": 0,
                "message": "Success",
                "data": {
                    "user_id": "user-456",
                },
            }),
            ..MockZaloState::default()
        };
        let router = build_mock_zalo_router(state);
        let (base_url, server) = spawn_mock_zalo_server(router).await;
        let resolved = build_resolved_zalo_config(Some(format!("{base_url}/v3.0/oa")));

        let error = run_zalo_send(
            &resolved,
            ChannelOutboundTargetKind::Address,
            "user-456",
            "hello from loongclaw",
        )
        .await
        .expect_err("missing message id should fail");

        assert!(error.contains("zalo send did not return a message id"));

        server.abort();
    }

    fn build_resolved_zalo_config(api_base_url: Option<String>) -> ResolvedZaloChannelConfig {
        ResolvedZaloChannelConfig {
            configured_account_id: "default".to_owned(),
            configured_account_label: "default".to_owned(),
            account: crate::config::ChannelAccountIdentity {
                id: "default".to_owned(),
                label: "default".to_owned(),
                source: crate::config::ChannelAccountIdentitySource::Default,
            },
            enabled: true,
            app_id: Some(SecretRef::Inline("zalo-app-id".to_owned())),
            app_id_env: None,
            oa_access_token: Some(SecretRef::Inline("oa-access-token".to_owned())),
            oa_access_token_env: None,
            app_secret: Some(SecretRef::Inline("app-secret".to_owned())),
            app_secret_env: None,
            api_base_url,
        }
    }

    fn build_mock_zalo_router(state: MockZaloState) -> Router {
        Router::new()
            .route("/v3.0/oa/message/cs", post(mock_zalo_send))
            .with_state(state)
    }

    async fn spawn_mock_zalo_server(router: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock zalo server");
        let address = listener.local_addr().expect("mock zalo server addr");
        let handle = tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("serve mock zalo server");
        });
        let base_url = format!("http://{}", address);
        (base_url, handle)
    }

    async fn mock_zalo_send(
        State(state): State<MockZaloState>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> impl IntoResponse {
        let access_token = header_text(headers.get("access_token"));
        {
            let mut access_tokens = state.access_tokens.lock().expect("access token log");
            access_tokens.push(access_token);
        }
        {
            let mut request_bodies = state.request_bodies.lock().expect("request body log");
            request_bodies.push(body);
        }

        (state.response_status, Json(state.response_payload))
    }

    fn header_text(value: Option<&axum::http::HeaderValue>) -> String {
        let Some(value) = value else {
            return String::new();
        };

        value.to_str().unwrap_or_default().to_owned()
    }
}
