use async_trait::async_trait;

use crate::channel::feishu::api::client::FeishuClient;
use crate::channel::traits::{
    ApiError, ApiResult, Document, DocumentContent, DocumentsApi, Pagination,
};

#[async_trait]
impl DocumentsApi for FeishuClient {
    async fn create_document(
        &self,
        title: &str,
        _content: Option<&str>,
        folder_id: Option<&str>,
    ) -> ApiResult<Document> {
        let token = self
            .get_tenant_access_token()
            .await
            .map_err(|e| ApiError::Auth {
                message: e,
                retry_after: None,
            })?;

        let result =
            crate::feishu::resources::docs::create_document(self, &token, Some(title), folder_id)
                .await
                .map_err(|e| ApiError::Platform {
                    platform: "feishu".to_string(),
                    code: "doc_create_failed".to_string(),
                    message: e,
                    raw: None,
                })?;

        Ok(Document {
            id: result.document_id,
            title: title.to_string(),
            url: result.url,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            owner_id: None,
            platform_metadata: serde_json::Value::Null,
        })
    }

    async fn read_document(&self, doc_id: &str) -> ApiResult<DocumentContent> {
        let token = self
            .get_tenant_access_token()
            .await
            .map_err(|e| ApiError::Auth {
                message: e,
                retry_after: None,
            })?;

        let content: crate::feishu::resources::types::FeishuDocumentContent =
            crate::feishu::resources::docs::fetch_document_content(self, &token, doc_id, None)
                .await
                .map_err(|e| ApiError::Platform {
                    platform: "feishu".to_string(),
                    code: "doc_read_failed".to_string(),
                    message: e,
                    raw: None,
                })?;

        Ok(DocumentContent {
            doc_id: content.document_id,
            title: "".to_string(),
            text: Some(content.content),
            markdown: None,
            html: None,
            blocks: None,
        })
    }

    async fn append_to_document(&self, _doc_id: &str, _content: &str) -> ApiResult<()> {
        Err(ApiError::NotSupported {
            operation: "append_to_document".to_string(),
            platform: "feishu".to_string(),
        })
    }

    async fn update_document(&self, _doc_id: &str, _content: &str) -> ApiResult<()> {
        Err(ApiError::NotSupported {
            operation: "update_document".to_string(),
            platform: "feishu".to_string(),
        })
    }

    async fn delete_document(&self, _doc_id: &str) -> ApiResult<()> {
        Err(ApiError::NotSupported {
            operation: "delete_document".to_string(),
            platform: "feishu".to_string(),
        })
    }

    async fn search_documents(
        &self,
        _query: &str,
        _pagination: &Pagination,
    ) -> ApiResult<Vec<Document>> {
        Err(ApiError::NotSupported {
            operation: "search_documents".to_string(),
            platform: "feishu".to_string(),
        })
    }
}
