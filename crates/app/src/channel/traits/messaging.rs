use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::error::{ApiResult, PlatformApi};

#[async_trait]
pub trait MessagingApi: PlatformApi {
    /// Send a message to a target (chat, channel, room)
    /// Returns the platform-specific message ID
    async fn send_message(&self, target: &str, content: &MessageContent) -> ApiResult<String>;

    /// Reply to an existing message
    async fn reply(&self, parent_id: &str, content: &MessageContent) -> ApiResult<String>;

    /// Get a message by ID
    async fn get_message(&self, message_id: &str) -> ApiResult<Message>;

    /// List messages in a chat with pagination
    async fn list_messages(
        &self,
        chat_id: &str,
        pagination: &Pagination,
    ) -> ApiResult<Vec<Message>>;

    /// Upload media (image, file, etc.)
    async fn upload_media(
        &self,
        file_path: &std::path::Path,
        media_type: MediaType,
    ) -> ApiResult<MediaUploadResult>;
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MessageContent {
    pub text: Option<String>,
    pub html: Option<String>,
    pub markdown: Option<String>,
    pub image_key: Option<String>,
    pub file_key: Option<String>,
    pub file_type: Option<String>,
    /// Platform-specific card/interactive message content (JSON)
    pub card: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub chat_id: String,
    pub sender_id: String,
    pub content: MessageContent,
    #[serde(
        serialize_with = "serialize_timestamp",
        deserialize_with = "deserialize_timestamp"
    )]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub reply_to: Option<String>,
    pub platform_metadata: serde_json::Value,
}

pub fn serialize_timestamp<S>(
    timestamp: &chrono::DateTime<chrono::Utc>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_i64(timestamp.timestamp())
}

pub fn deserialize_timestamp<'de, D>(
    deserializer: D,
) -> Result<chrono::DateTime<chrono::Utc>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let timestamp = i64::deserialize(deserializer)?;
    chrono::DateTime::from_timestamp(timestamp, 0)
        .ok_or_else(|| serde::de::Error::custom("invalid timestamp"))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Pagination {
    pub page_size: u32,
    pub cursor: Option<String>,
    /// Some platforms use page numbers instead of cursors
    pub page: Option<u32>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    Image,
    File,
    Audio,
    Video,
    Sticker,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaUploadResult {
    pub file_key: String,
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
    pub mime_type: Option<String>,
}
