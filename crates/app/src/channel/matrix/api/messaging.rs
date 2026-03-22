use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::Value;

use crate::channel::matrix::MatrixAdapter;
use crate::channel::traits::{
    ApiError, ApiResult, MediaType, MediaUploadResult, Message, MessageContent, MessagingApi,
    Pagination, PlatformApi,
};

impl PlatformApi for MatrixAdapter {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait]
impl MessagingApi for MatrixAdapter {
    async fn send_message(
        &self,
        target: &str,
        content: &MessageContent,
    ) -> ApiResult<String> {
        let room_id = target;
        let text = content
            .text
            .as_ref()
            .or(content.markdown.as_ref())
            .or(content.html.as_ref())
            .ok_or_else(|| ApiError::InvalidRequest {
                message: "Message content must have text, markdown, or html".to_string(),
                field: Some("content".to_string()),
            })?;

        let txn_id = format!("loongclaw-{}", next_transaction_id());
        let body = serde_json::json!({
            "msgtype": "m.text",
            "body": text,
        });

        let client = build_matrix_http_client().map_err(ApiError::Internal)?;
        let url = self
            .send_event_url(room_id, &txn_id)
            .map_err(|e| ApiError::InvalidRequest {
                message: e,
                field: Some("target".to_string()),
            })?;

        let response = client
            .put(url)
            .bearer_auth(self.access_token.as_str())
            .json(&body)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        let status = response.status();
        let payload: Value = response
            .json()
            .await
            .map_err(|e| ApiError::Serialization(e.to_string()))?;

        if !status.is_success() {
            return Err(ApiError::Http {
                status: status.as_u16(),
                body: payload.to_string(),
            });
        }

        payload
            .get("event_id")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .ok_or_else(|| ApiError::Platform {
                platform: "matrix".to_string(),
                code: "missing_event_id".to_string(),
                message: "Response missing event_id".to_string(),
                raw: Some(payload),
            })
    }

    async fn reply(
        &self,
        parent_id: &str,
        content: &MessageContent,
    ) -> ApiResult<String> {
        let text = content
            .text
            .as_ref()
            .or(content.markdown.as_ref())
            .or(content.html.as_ref())
            .ok_or_else(|| ApiError::InvalidRequest {
                message: "Message content must have text, markdown, or html".to_string(),
                field: Some("content".to_string()),
            })?;

        let parts: Vec<&str> = parent_id.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(ApiError::InvalidRequest {
                message: "parent_id must include room_id and event_id separated by ':'".to_string(),
                field: Some("parent_id".to_string()),
            });
        }

        let room_id = parts
            .first()
            .ok_or_else(|| ApiError::InvalidRequest {
                message: "Missing room_id".to_string(),
                field: Some("parent_id".to_string()),
            })?;
        let event_id = parts.get(1).ok_or_else(|| ApiError::InvalidRequest {
            message: "Missing event_id".to_string(),
            field: Some("parent_id".to_string()),
        })?;

        let txn_id = format!("loongclaw-reply-{}", next_transaction_id());
        let body = serde_json::json!({
            "msgtype": "m.text",
            "body": text,
            "m.relates_to": {
                "m.in_reply_to": {
                    "event_id": event_id
                }
            }
        });

        let client = build_matrix_http_client().map_err(ApiError::Internal)?;
        let url = self
            .send_event_url(room_id, &txn_id)
            .map_err(|e| ApiError::InvalidRequest {
                message: e,
                field: Some("target".to_string()),
            })?;

        let response = client
            .put(url)
            .bearer_auth(self.access_token.as_str())
            .json(&body)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        let status = response.status();
        let payload: Value = response
            .json()
            .await
            .map_err(|e| ApiError::Serialization(e.to_string()))?;

        if !status.is_success() {
            return Err(ApiError::Http {
                status: status.as_u16(),
                body: payload.to_string(),
            });
        }

        payload
            .get("event_id")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .ok_or_else(|| ApiError::Platform {
                platform: "matrix".to_string(),
                code: "missing_event_id".to_string(),
                message: "Response missing event_id".to_string(),
                raw: Some(payload),
            })
    }

    async fn get_message(&self,
        _message_id: &str,
    ) -> ApiResult<Message> {
        Err(ApiError::NotSupported {
            operation: "get_message".to_string(),
            platform: "matrix".to_string(),
        })
    }

    async fn list_messages(
        &self,
        _chat_id: &str,
        _pagination: &Pagination,
    ) -> ApiResult<Vec<Message>> {
        Err(ApiError::NotSupported {
            operation: "list_messages".to_string(),
            platform: "matrix".to_string(),
        })
    }

    async fn upload_media(
        &self,
        _file_path: &std::path::Path,
        _media_type: MediaType,
    ) -> ApiResult<MediaUploadResult> {
        Err(ApiError::NotSupported {
            operation: "upload_media".to_string(),
            platform: "matrix".to_string(),
        })
    }
}

fn next_transaction_id() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

fn build_matrix_http_client() -> crate::CliResult<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|error| format!("build matrix http client failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_next_transaction_id_is_monotonic() {
        let id1 = next_transaction_id();
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        let id2 = next_transaction_id();
        assert!(id2 > id1);
    }
}
