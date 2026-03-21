use crate::channel::feishu::FeishuClient;
use crate::channel::feishu::resources::media::{upload_message_file, upload_message_image};
use crate::channel::feishu::resources::messages::{
    FeishuMessageDetail, FeishuMessageHistoryPage, FeishuMessageWriteReceipt,
};
use crate::channel::traits::messaging::{
    ApiResult, MediaType, MessageContent, MessagingApi, Pagination,
};

pub(super) struct FeishuMessagingImpl {
    client: FeishuClient,
}

impl FeishuMessagingImpl {
    pub(super) fn new(client: FeishuClient) -> Self {
        Self { client }
    }
}

#[derive(Clone, Debug)]
pub struct FeishuMessagingReceipt {
    pub message_id: String,
}

#[derive(Clone, Debug)]
pub struct FeishuMessagingMessage {
    pub message_id: String,
    pub chat_id: Option<String>,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct FeishuMessagingPage {
    pub messages: Vec<FeishuMessagingMessage>,
    pub has_more: bool,
    pub page_token: Option<String>,
}

#[derive(Clone, Debug)]
pub struct FeishuMediaUploadResult {
    pub file_key: String,
}

#[async_trait::async_trait]
impl MessagingApi for FeishuMessagingImpl {
    type Receipt = FeishuMessagingReceipt;
    type Message = FeishuMessagingMessage;
    type MessagePage = FeishuMessagingPage;
    type MediaUploadResult = FeishuMediaUploadResult;

    async fn send_message(
        &self,
        target: &str,
        receive_id_type: Option<&str>,
        content: &MessageContent,
        _idempotency_key: Option<&str>,
    ) -> ApiResult<Self::Receipt> {
        let tenant_token = self
            .client
            .get_tenant_access_token()
            .await
            .map_err(|e| crate::channel::traits::messaging::ApiError::Auth(e.to_string()))?;

        let receive_id_type = receive_id_type.unwrap_or("chat_id");

        let body = if let Some(text) = &content.text {
            serde_json::json!({"text": text})
        } else if let Some(card) = &content.card {
            serde_json::json!({"card": card})
        } else if let Some(image_key) = &content.image_key {
            serde_json::json!({"image_key": image_key})
        } else if let Some(file_key) = &content.file_key {
            serde_json::json!({"file_key": file_key})
        } else {
            return Err(crate::channel::traits::messaging::ApiError::InvalidRequest(
                "no content provided".to_string(),
            ));
        };

        let msg_type = if content.text.is_some() {
            "text"
        } else if content.card.is_some() {
            "interactive"
        } else if content.image_key.is_some() {
            "image"
        } else {
            "file"
        };

        let receipt = crate::channel::feishu::resources::messages::send_message(
            &self.client,
            &tenant_token,
            target,
            receive_id_type,
            msg_type,
            body,
            None,
        )
        .await
        .map_err(|e| crate::channel::traits::messaging::ApiError::InvalidRequest(e.to_string()))?;

        Ok(FeishuMessagingReceipt {
            message_id: receipt.message_id,
        })
    }

    async fn reply(&self, parent_id: &str, content: &MessageContent) -> ApiResult<Self::Receipt> {
        let tenant_token = self
            .client
            .get_tenant_access_token()
            .await
            .map_err(|e| crate::channel::traits::messaging::ApiError::Auth(e.to_string()))?;

        let body = if let Some(text) = &content.text {
            serde_json::json!({"text": text})
        } else if let Some(card) = &content.card {
            serde_json::json!({"card": card})
        } else {
            return Err(crate::channel::traits::messaging::ApiError::InvalidRequest(
                "no content provided".to_string(),
            ));
        };

        let msg_type = if content.text.is_some() {
            "text"
        } else {
            "interactive"
        };

        let receipt = crate::channel::feishu::resources::messages::reply_message(
            &self.client,
            &tenant_token,
            parent_id,
            msg_type,
            body,
            None,
        )
        .await
        .map_err(|e| crate::channel::traits::messaging::ApiError::InvalidRequest(e.to_string()))?;

        Ok(FeishuMessagingReceipt {
            message_id: receipt.message_id,
        })
    }

    async fn get_message(&self, message_id: &str) -> ApiResult<Self::Message> {
        let tenant_token = self
            .client
            .get_tenant_access_token()
            .await
            .map_err(|e| crate::channel::traits::messaging::ApiError::Auth(e.to_string()))?;

        let detail = crate::channel::feishu::resources::messages::fetch_message_detail(
            &self.client,
            &tenant_token,
            message_id,
        )
        .await
        .map_err(|e| crate::channel::traits::messaging::ApiError::InvalidRequest(e.to_string()))?;

        Ok(FeishuMessagingMessage {
            message_id: detail.message_id,
            chat_id: detail.chat_id,
            content: detail.body.to_string(),
        })
    }

    async fn list_messages(
        &self,
        chat_id: &str,
        pagination: &Pagination,
    ) -> ApiResult<Self::MessagePage> {
        let tenant_token = self
            .client
            .get_tenant_access_token()
            .await
            .map_err(|e| crate::channel::traits::messaging::ApiError::Auth(e.to_string()))?;

        let query = crate::channel::feishu::resources::messages::FeishuMessageHistoryQuery {
            container_id_type: "chat_id".to_string(),
            container_id: chat_id.to_string(),
            start_time: None,
            end_time: None,
            sort_type: None,
            page_size: pagination.page_size.map(|s| s as usize),
            page_token: pagination.cursor.clone(),
        };

        let history = crate::channel::feishu::resources::messages::fetch_message_history(
            &self.client,
            &tenant_token,
            &query,
        )
        .await
        .map_err(|e| crate::channel::traits::messaging::ApiError::InvalidRequest(e.to_string()))?;

        let messages = history
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|m| FeishuMessagingMessage {
                message_id: m.message_id,
                chat_id: m.chat_id,
                content: m.body.map(|b| b.to_string()).unwrap_or_default(),
            })
            .collect();

        Ok(FeishuMessagingPage {
            messages,
            has_more: history.has_more.unwrap_or(false),
            page_token: history.page_token,
        })
    }

    async fn upload_media(
        &self,
        file_path: Option<&str>,
        _file_key: Option<&str>,
        media_type: MediaType,
    ) -> ApiResult<Self::MediaUploadResult> {
        let tenant_token = self
            .client
            .get_tenant_access_token()
            .await
            .map_err(|e| crate::channel::traits::messaging::ApiError::Auth(e.to_string()))?;

        match (file_path, media_type) {
            (Some(path), MediaType::Image) => {
                let result = upload_message_image(&self.client, &tenant_token, Some(path))
                    .await
                    .map_err(|e| {
                        crate::channel::traits::messaging::ApiError::InvalidRequest(e.to_string())
                    })?;
                Ok(FeishuMediaUploadResult {
                    file_key: result.image_key,
                })
            }
            (Some(path), _) => {
                let file_type = match media_type {
                    MediaType::File => "stream",
                    MediaType::Audio => "audio",
                    MediaType::Video => "video",
                    _ => "stream",
                };
                let result =
                    upload_message_file(&self.client, &tenant_token, Some(path), Some(file_type))
                        .await
                        .map_err(|e| {
                            crate::channel::traits::messaging::ApiError::InvalidRequest(
                                e.to_string(),
                            )
                        })?;
                Ok(FeishuMediaUploadResult {
                    file_key: result.file_key,
                })
            }
            _ => Err(crate::channel::traits::messaging::ApiError::InvalidRequest(
                "file_path is required".to_string(),
            )),
        }
    }
}
