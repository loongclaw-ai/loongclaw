use async_trait::async_trait;
use serde_json::Value;

use crate::channel::telegram::TelegramAdapter;
use crate::channel::traits::{
    ApiError, ApiResult, MediaType, MediaUploadResult, Message, MessageContent, MessagingApi,
    Pagination, PlatformApi,
};

impl PlatformApi for TelegramAdapter {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait]
impl MessagingApi for TelegramAdapter {
    async fn send_message(&self, target: &str, content: &MessageContent) -> ApiResult<String> {
        let chat_id: i64 = target.parse().map_err(|e| ApiError::InvalidRequest {
            message: format!("Invalid chat_id: {}", e),
            field: Some("target".to_string()),
        })?;

        let url = self.api_url("sendMessage");
        let body = build_send_message_body(chat_id, content)?;

        let response = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        let status = response.status();
        let payload: Value = response
            .json()
            .await
            .map_err(|e| ApiError::Serialization(e.to_string()))?;

        if !status.is_success() || !payload.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            let error_msg = payload
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("Unknown error");
            return Err(ApiError::Platform {
                platform: "telegram".to_string(),
                code: status.as_u16().to_string(),
                message: error_msg.to_string(),
                raw: Some(payload),
            });
        }

        extract_message_id(&payload)
    }

    async fn reply(&self, parent_id: &str, content: &MessageContent) -> ApiResult<String> {
        let (chat_id, reply_to_message_id) = parse_parent_id(parent_id)?;

        let url = self.api_url("sendMessage");
        let body = build_reply_message_body(chat_id, reply_to_message_id, content)?;

        let response = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        let status = response.status();
        let payload: Value = response
            .json()
            .await
            .map_err(|e| ApiError::Serialization(e.to_string()))?;

        if !status.is_success() || !payload.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            let error_msg = payload
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("Unknown error");
            return Err(ApiError::Platform {
                platform: "telegram".to_string(),
                code: status.as_u16().to_string(),
                message: error_msg.to_string(),
                raw: Some(payload),
            });
        }

        extract_message_id(&payload)
    }

    async fn get_message(&self, _message_id: &str) -> ApiResult<Message> {
        Err(ApiError::NotSupported {
            operation: "get_message".to_string(),
            platform: "telegram".to_string(),
        })
    }

    async fn list_messages(
        &self,
        _chat_id: &str,
        _pagination: &Pagination,
    ) -> ApiResult<Vec<Message>> {
        Err(ApiError::NotSupported {
            operation: "list_messages".to_string(),
            platform: "telegram".to_string(),
        })
    }

    async fn upload_media(
        &self,
        _file_path: &std::path::Path,
        _media_type: MediaType,
    ) -> ApiResult<MediaUploadResult> {
        Err(ApiError::NotSupported {
            operation: "upload_media (use platform-specific send methods instead)".to_string(),
            platform: "telegram".to_string(),
        })
    }
}

fn build_send_message_body(chat_id: i64, content: &MessageContent) -> ApiResult<Value> {
    let body = if let Some(text) = &content.text {
        serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "disable_web_page_preview": true,
        })
    } else if let Some(markdown) = &content.markdown {
        serde_json::json!({
            "chat_id": chat_id,
            "text": markdown,
            "parse_mode": "MarkdownV2",
            "disable_web_page_preview": true,
        })
    } else if let Some(html) = &content.html {
        serde_json::json!({
            "chat_id": chat_id,
            "text": html,
            "parse_mode": "HTML",
            "disable_web_page_preview": true,
        })
    } else {
        return Err(ApiError::InvalidRequest {
            message: "Message content must have text, markdown, or html".to_string(),
            field: Some("content".to_string()),
        });
    };
    Ok(body)
}

fn build_reply_message_body(
    chat_id: i64,
    reply_to_message_id: i64,
    content: &MessageContent,
) -> ApiResult<Value> {
    let body = if let Some(text) = &content.text {
        serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "reply_to_message_id": reply_to_message_id,
            "disable_web_page_preview": true,
        })
    } else if let Some(markdown) = &content.markdown {
        serde_json::json!({
            "chat_id": chat_id,
            "text": markdown,
            "parse_mode": "MarkdownV2",
            "reply_to_message_id": reply_to_message_id,
            "disable_web_page_preview": true,
        })
    } else if let Some(html) = &content.html {
        serde_json::json!({
            "chat_id": chat_id,
            "text": html,
            "parse_mode": "HTML",
            "reply_to_message_id": reply_to_message_id,
            "disable_web_page_preview": true,
        })
    } else {
        return Err(ApiError::InvalidRequest {
            message: "Message content must have text, markdown, or html".to_string(),
            field: Some("content".to_string()),
        });
    };
    Ok(body)
}

fn parse_parent_id(parent_id: &str) -> ApiResult<(i64, i64)> {
    let parts: Vec<&str> = parent_id.split(':').collect();
    if parts.len() != 2 {
        return Err(ApiError::InvalidRequest {
            message: "parent_id must be in format 'chat_id:message_id'".to_string(),
            field: Some("parent_id".to_string()),
        });
    }

    let chat_id: i64 = parts
        .first()
        .ok_or_else(|| ApiError::InvalidRequest {
            message: "Missing chat_id in parent_id".to_string(),
            field: Some("parent_id".to_string()),
        })?
        .parse()
        .map_err(|e| ApiError::InvalidRequest {
            message: format!("Invalid chat_id in parent_id: {}", e),
            field: Some("parent_id".to_string()),
        })?;

    let message_id: i64 = parts
        .get(1)
        .ok_or_else(|| ApiError::InvalidRequest {
            message: "Missing message_id in parent_id".to_string(),
            field: Some("parent_id".to_string()),
        })?
        .parse()
        .map_err(|e| ApiError::InvalidRequest {
            message: format!("Invalid message_id in parent_id: {}", e),
            field: Some("parent_id".to_string()),
        })?;

    Ok((chat_id, message_id))
}

fn extract_message_id(payload: &Value) -> ApiResult<String> {
    payload
        .get("result")
        .and_then(|r| r.get("message_id"))
        .and_then(Value::as_i64)
        .map(|id| id.to_string())
        .ok_or_else(|| ApiError::Platform {
            platform: "telegram".to_string(),
            code: "parse_error".to_string(),
            message: "Failed to extract message_id from response".to_string(),
            raw: Some(payload.clone()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_parent_id_valid() {
        let result = parse_parent_id("123456:789").unwrap();
        assert_eq!(result, (123456, 789));
    }

    #[test]
    fn test_parse_parent_id_invalid_format() {
        let result = parse_parent_id("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_build_send_message_body_with_text() {
        let content = MessageContent {
            text: Some("Hello".to_string()),
            ..Default::default()
        };
        let body = build_send_message_body(123456, &content).unwrap();
        assert_eq!(body["chat_id"], 123456);
        assert_eq!(body["text"], "Hello");
    }

    #[test]
    fn test_build_send_message_body_with_markdown() {
        let content = MessageContent {
            markdown: Some("*bold*".to_string()),
            ..Default::default()
        };
        let body = build_send_message_body(123456, &content).unwrap();
        assert_eq!(body["parse_mode"], "MarkdownV2");
    }
}
