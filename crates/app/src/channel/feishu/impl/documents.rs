use crate::channel::feishu::FeishuClient;
use crate::channel::feishu::resources::docs::{
    self as docs, FeishuDocumentContent, FeishuDocumentMetadata,
};
use crate::channel::traits::documents::{ApiResult, DocumentsApi};

pub(super) struct FeishuDocumentsImpl {
    client: FeishuClient,
}

impl FeishuDocumentsImpl {
    pub(super) fn new(client: FeishuClient) -> Self {
        Self { client }
    }
}

#[derive(Clone, Debug)]
pub struct FeishuDocument {
    pub document_id: String,
    pub title: Option<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug)]
pub struct FeishuDocContent {
    pub document_id: String,
    pub content: String,
}

#[async_trait::async_trait]
impl DocumentsApi for FeishuDocumentsImpl {
    type Document = FeishuDocument;
    type DocumentContent = FeishuDocContent;

    async fn create_document(
        &self,
        title: &str,
        content: Option<&str>,
    ) -> ApiResult<Self::Document> {
        let token = self
            .client
            .get_tenant_access_token()
            .await
            .map_err(|e| crate::channel::traits::documents::ApiError::Auth(e.to_string()))?;

        let doc: FeishuDocumentMetadata = docs::create_document(&self.client, &token, title)
            .await
            .map_err(|e| {
                crate::channel::traits::documents::ApiError::InvalidRequest(e.to_string())
            })?;

        if let Some(content) = content {
            let _ = docs::append_document(&self.client, &token, &doc.document_id, content)
                .await
                .map_err(|e| {
                    crate::channel::traits::documents::ApiError::InvalidRequest(e.to_string())
                })?;
        }

        Ok(FeishuDocument {
            document_id: doc.document_id,
            title: doc.title,
            url: doc.url,
        })
    }

    async fn read_document(&self, doc_id: &str) -> ApiResult<Self::DocumentContent> {
        let token = self
            .client
            .get_tenant_access_token()
            .await
            .map_err(|e| crate::channel::traits::documents::ApiError::Auth(e.to_string()))?;

        let content: FeishuDocumentContent = docs::read_document(&self.client, &token, doc_id)
            .await
            .map_err(|e| {
                crate::channel::traits::documents::ApiError::InvalidRequest(e.to_string())
            })?;

        Ok(FeishuDocContent {
            document_id: content.document_id,
            content: content.content,
        })
    }

    async fn append_to_document(&self, doc_id: &str, content: &str) -> ApiResult<()> {
        let token = self
            .client
            .get_tenant_access_token()
            .await
            .map_err(|e| crate::channel::traits::documents::ApiError::Auth(e.to_string()))?;

        docs::append_document(&self.client, &token, doc_id, content)
            .await
            .map_err(|e| {
                crate::channel::traits::documents::ApiError::InvalidRequest(e.to_string())
            })?;

        Ok(())
    }
}
