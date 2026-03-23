//! Feishu Platform API Implementation
//!
//! Implements the PlatformApi traits (MessagingApi, DocumentsApi, CalendarApi)
//! for the FeishuClient.

use async_trait::async_trait;

use crate::channel::feishu::api::client::FeishuClient;
use crate::channel::traits::{
    ApiError, ApiResult, MediaType, MediaUploadResult, Message, MessageContent, MessagingApi,
    Pagination, PlatformApi,
};

impl PlatformApi for FeishuClient {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait]
impl MessagingApi for FeishuClient {
    async fn send_message(&self, target: &str, content: &MessageContent) -> ApiResult<String> {
        let token = self
            .get_tenant_access_token()
            .await
            .map_err(|e| ApiError::Auth {
                message: e,
                retry_after: None,
            })?;

        let body = build_message_body(content)?;
        let result = crate::feishu::resources::messages::send_outbound_message(
            self, &token, "chat_id", target, &body, None,
        )
        .await
        .map_err(map_feishu_error)?;

        Ok(result.message_id)
    }

    async fn reply(&self, parent_id: &str, content: &MessageContent) -> ApiResult<String> {
        let token = self
            .get_tenant_access_token()
            .await
            .map_err(|e| ApiError::Auth {
                message: e,
                retry_after: None,
            })?;

        let body = build_message_body(content)?;
        let result = crate::feishu::resources::messages::reply_outbound_message(
            self, &token, parent_id, &body, false, None,
        )
        .await
        .map_err(map_feishu_error)?;

        Ok(result.message_id)
    }

    async fn get_message(&self, message_id: &str) -> ApiResult<Message> {
        let token = self
            .get_tenant_access_token()
            .await
            .map_err(|e| ApiError::Auth {
                message: e,
                retry_after: None,
            })?;

        let detail: crate::feishu::resources::types::FeishuMessageDetail =
            crate::feishu::resources::messages::fetch_message_detail(self, &token, message_id)
                .await
                .map_err(map_feishu_error)?;

        Ok(convert_to_message(detail))
    }

    async fn list_messages(
        &self,
        chat_id: &str,
        pagination: &Pagination,
    ) -> ApiResult<Vec<Message>> {
        let token = self
            .get_tenant_access_token()
            .await
            .map_err(|e| ApiError::Auth {
                message: e,
                retry_after: None,
            })?;

        let query = crate::feishu::resources::messages::FeishuMessageHistoryQuery {
            container_id_type: "chat".to_string(),
            container_id: chat_id.to_string(),
            start_time: None,
            end_time: None,
            sort_type: None,
            page_size: Some(pagination.page_size as usize),
            page_token: pagination.cursor.clone(),
        };

        let page: crate::feishu::resources::types::FeishuMessageHistoryPage =
            crate::feishu::resources::messages::fetch_message_history(self, &token, &query)
                .await
                .map_err(map_feishu_error)?;

        let messages: Vec<Message> = page
            .items
            .into_iter()
            .map(convert_summary_to_message)
            .collect();
        Ok(messages)
    }

    async fn upload_media(
        &self,
        _file_path: &std::path::Path,
        media_type: MediaType,
    ) -> ApiResult<MediaUploadResult> {
        match media_type {
            MediaType::Image
            | MediaType::File
            | MediaType::Audio
            | MediaType::Video
            | MediaType::Sticker => Err(ApiError::NotSupported {
                operation: "upload_media".to_string(),
                platform: "feishu".to_string(),
            }),
        }
    }
}

fn build_message_body(
    content: &MessageContent,
) -> ApiResult<crate::feishu::resources::messages::FeishuOutboundMessageBody> {
    if let Some(text) = &content.text {
        Ok(crate::feishu::resources::messages::FeishuOutboundMessageBody::Text(text.clone()))
    } else if let Some(markdown) = &content.markdown {
        Ok(
            crate::feishu::resources::messages::FeishuOutboundMessageBody::MarkdownCard(
                markdown.clone(),
            ),
        )
    } else if let Some(card) = &content.card {
        Ok(crate::feishu::resources::messages::FeishuOutboundMessageBody::Post(card.clone()))
    } else {
        Err(ApiError::InvalidRequest {
            message: "Message must have text, markdown, or card content".to_string(),
            field: Some("content".to_string()),
        })
    }
}

fn map_feishu_error(e: String) -> ApiError {
    if e.contains("code") && e.contains("99991663") {
        ApiError::RateLimited {
            retry_after_secs: Some(1),
        }
    } else if e.contains("token") || e.contains("auth") || e.contains("unauthorized") {
        ApiError::Auth {
            message: e,
            retry_after: None,
        }
    } else if e.contains("not found") || e.contains("99991400") {
        ApiError::NotFound {
            resource: "feishu_resource".to_string(),
            id: None,
        }
    } else if e.contains("permission") || e.contains("403") {
        ApiError::PermissionDenied {
            action: "api_call".to_string(),
            resource: "feishu".to_string(),
        }
    } else {
        ApiError::Platform {
            platform: "feishu".to_string(),
            code: "unknown".to_string(),
            message: e,
            raw: None,
        }
    }
}

fn convert_to_message(detail: crate::feishu::resources::types::FeishuMessageDetail) -> Message {
    Message {
        id: detail.message_id,
        chat_id: detail.chat_id.unwrap_or_default(),
        sender_id: detail.sender_id.unwrap_or_default(),
        content: MessageContent {
            text: extract_text_from_body(&detail.body),
            html: None,
            markdown: None,
            image_key: None,
            file_key: None,
            file_type: None,
            card: None,
        },
        timestamp: chrono::Utc::now(),
        reply_to: detail.parent_id,
        platform_metadata: detail.body,
    }
}

fn convert_summary_to_message(
    summary: crate::feishu::resources::types::FeishuMessageSummary,
) -> Message {
    Message {
        id: summary.message_id,
        chat_id: summary.chat_id.unwrap_or_default(),
        sender_id: "".to_string(),
        content: MessageContent::default(),
        timestamp: chrono::Utc::now(),
        reply_to: summary.parent_id,
        platform_metadata: serde_json::Value::Null,
    }
}

fn extract_text_from_body(body: &serde_json::Value) -> Option<String> {
    body.get("content")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
}
