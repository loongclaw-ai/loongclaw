use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::error::{ApiResult, PlatformApi};
use super::messaging::Pagination;

#[async_trait]
pub trait DocumentsApi: PlatformApi {
    /// Create a new document
    async fn create_document(
        &self,
        title: &str,
        content: Option<&str>,
        folder_id: Option<&str>,
    ) -> ApiResult<Document>;

    /// Read document content
    async fn read_document(&self, doc_id: &str) -> ApiResult<DocumentContent>;

    /// Append content to document
    async fn append_to_document(&self, doc_id: &str, content: &str) -> ApiResult<()>;

    /// Update document (replace content)
    async fn update_document(&self, doc_id: &str, content: &str) -> ApiResult<()>;

    /// Delete a document
    async fn delete_document(&self, doc_id: &str) -> ApiResult<()>;

    /// Search documents
    async fn search_documents(
        &self,
        query: &str,
        pagination: &Pagination,
    ) -> ApiResult<Vec<Document>>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    #[serde(
        serialize_with = "super::messaging::serialize_timestamp",
        deserialize_with = "super::messaging::deserialize_timestamp"
    )]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(
        serialize_with = "super::messaging::serialize_timestamp",
        deserialize_with = "super::messaging::deserialize_timestamp"
    )]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub owner_id: Option<String>,
    pub platform_metadata: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentContent {
    pub doc_id: String,
    pub title: String,
    pub text: Option<String>,
    pub markdown: Option<String>,
    pub html: Option<String>,
    pub blocks: Option<serde_json::Value>,
}
