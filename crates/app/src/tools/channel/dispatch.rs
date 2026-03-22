// This module provides infrastructure for generic platform tool dispatch.
// The types are public API that will be used in future integration phases.
#![allow(dead_code)]

use std::sync::Arc;

use serde_json::Value;

use crate::channel::traits::{ApiResult, DocumentsApi, MessageContent, MessagingApi, PlatformApi};

pub type PlatformApiRef = Arc<dyn PlatformApi>;

pub fn as_messaging_api(api: &PlatformApiRef) -> Option<&dyn MessagingApi> {
    api.as_any().downcast_ref::<&dyn MessagingApi>().copied()
}

pub fn as_documents_api(api: &PlatformApiRef) -> Option<&dyn DocumentsApi> {
    api.as_any().downcast_ref::<&dyn DocumentsApi>().copied()
}

pub async fn dispatch_send_message(api: &dyn MessagingApi, params: &Value) -> ApiResult<Value> {
    let target = params
        .get("target")
        .and_then(Value::as_str)
        .ok_or_else(|| crate::channel::traits::ApiError::InvalidRequest {
            message: "Missing required parameter: target".to_string(),
            field: Some("target".to_string()),
        })?;

    let content = parse_message_content(params)?;
    let message_id = api.send_message(target, &content).await?;

    Ok(serde_json::json!({
        "message_id": message_id,
        "status": "sent"
    }))
}

pub async fn dispatch_reply(api: &dyn MessagingApi, params: &Value) -> ApiResult<Value> {
    let parent_id = params
        .get("parent_id")
        .and_then(Value::as_str)
        .ok_or_else(|| crate::channel::traits::ApiError::InvalidRequest {
            message: "Missing required parameter: parent_id".to_string(),
            field: Some("parent_id".to_string()),
        })?;

    let content = parse_message_content(params)?;
    let message_id = api.reply(parent_id, &content).await?;

    Ok(serde_json::json!({
        "message_id": message_id,
        "status": "replied"
    }))
}

pub async fn dispatch_create_document(api: &dyn DocumentsApi, params: &Value) -> ApiResult<Value> {
    let title = params.get("title").and_then(Value::as_str).ok_or_else(|| {
        crate::channel::traits::ApiError::InvalidRequest {
            message: "Missing required parameter: title".to_string(),
            field: Some("title".to_string()),
        }
    })?;

    let content = params.get("content").and_then(Value::as_str);
    let folder_id = params.get("folder_id").and_then(Value::as_str);

    let doc = api.create_document(title, content, folder_id).await?;

    Ok(serde_json::json!({
        "document_id": doc.id,
        "title": doc.title,
        "url": doc.url,
    }))
}

pub async fn dispatch_read_document(api: &dyn DocumentsApi, params: &Value) -> ApiResult<Value> {
    let doc_id = params
        .get("document_id")
        .and_then(Value::as_str)
        .ok_or_else(|| crate::channel::traits::ApiError::InvalidRequest {
            message: "Missing required parameter: document_id".to_string(),
            field: Some("document_id".to_string()),
        })?;

    let doc_content = api.read_document(doc_id).await?;

    Ok(serde_json::json!({
        "document_id": doc_content.doc_id,
        "title": doc_content.title,
        "text": doc_content.text,
        "markdown": doc_content.markdown,
    }))
}

fn parse_message_content(params: &Value) -> ApiResult<MessageContent> {
    let mut content = MessageContent::default();

    if let Some(text) = params.get("text").and_then(Value::as_str) {
        content.text = Some(text.to_string());
    }

    if let Some(html) = params.get("html").and_then(Value::as_str) {
        content.html = Some(html.to_string());
    }

    if let Some(markdown) = params.get("markdown").and_then(Value::as_str) {
        content.markdown = Some(markdown.to_string());
    }

    if content.text.is_none() && content.html.is_none() && content.markdown.is_none() {
        return Err(crate::channel::traits::ApiError::InvalidRequest {
            message: "Message must have text, html, or markdown".to_string(),
            field: None,
        });
    }

    Ok(content)
}

pub struct PlatformToolDispatcher {
    platform: String,
    api: PlatformApiRef,
}

impl PlatformToolDispatcher {
    pub fn new(platform: impl Into<String>, api: PlatformApiRef) -> Self {
        Self {
            platform: platform.into(),
            api,
        }
    }

    pub fn platform(&self) -> &str {
        &self.platform
    }

    pub async fn dispatch(&self, tool_name: &str, params: &Value) -> crate::CliResult<Value> {
        let result = match tool_name {
            "messages.send" => {
                let api = as_messaging_api(&self.api).ok_or_else(|| {
                    format!("Platform {} does not support messaging", self.platform)
                })?;
                dispatch_send_message(api, params).await
            }
            "messages.reply" => {
                let api = as_messaging_api(&self.api).ok_or_else(|| {
                    format!("Platform {} does not support messaging", self.platform)
                })?;
                dispatch_reply(api, params).await
            }
            "doc.create" => {
                let api = as_documents_api(&self.api).ok_or_else(|| {
                    format!("Platform {} does not support documents", self.platform)
                })?;
                dispatch_create_document(api, params).await
            }
            "doc.read" => {
                let api = as_documents_api(&self.api).ok_or_else(|| {
                    format!("Platform {} does not support documents", self.platform)
                })?;
                dispatch_read_document(api, params).await
            }
            _ => {
                return Err(format!("Unknown tool: {}", tool_name));
            }
        };

        result.map_err(|e| format!("API error: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_content_with_text() {
        let params = serde_json::json!({
            "target": "123",
            "text": "Hello"
        });
        let content = parse_message_content(&params).unwrap();
        assert_eq!(content.text, Some("Hello".to_string()));
    }

    #[test]
    fn test_parse_message_content_missing_content() {
        let params = serde_json::json!({
            "target": "123"
        });
        let result = parse_message_content(&params);
        assert!(result.is_err());
    }
}
