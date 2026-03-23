//! Feishu channel tools using PlatformApi traits
//!
//! These tools use the trait abstractions from channel/traits/ instead of
//! directly depending on FeishuClient, enabling better testability and
//! cross-platform compatibility.

#![allow(dead_code)]

use std::sync::Arc;

use crate::channel::feishu::api::FeishuClient;
use crate::channel::traits::{
    ApiResult, CalendarApi, DocumentContent, DocumentsApi, MessageContent, MessagingApi, Pagination,
};

/// Feishu messaging tool using trait abstractions
pub struct FeishuMessagingTool {
    client: Arc<FeishuClient>,
}

impl FeishuMessagingTool {
    pub fn new(client: Arc<FeishuClient>) -> Self {
        Self { client }
    }

    pub async fn send_message(&self, target: &str, content: &MessageContent) -> ApiResult<String> {
        self.client.send_message(target, content).await
    }

    pub async fn reply(&self, parent_id: &str, content: &MessageContent) -> ApiResult<String> {
        self.client.reply(parent_id, content).await
    }

    pub async fn get_message(
        &self,
        message_id: &str,
    ) -> ApiResult<crate::channel::traits::Message> {
        self.client.get_message(message_id).await
    }

    pub async fn list_messages(
        &self,
        chat_id: &str,
        pagination: &Pagination,
    ) -> ApiResult<Vec<crate::channel::traits::Message>> {
        self.client.list_messages(chat_id, pagination).await
    }
}

/// Feishu documents tool using trait abstractions
pub struct FeishuDocumentsTool {
    client: Arc<FeishuClient>,
}

impl FeishuDocumentsTool {
    pub fn new(client: Arc<FeishuClient>) -> Self {
        Self { client }
    }

    pub async fn create_document(
        &self,
        title: &str,
        content: Option<&str>,
    ) -> ApiResult<crate::channel::traits::Document> {
        self.client.create_document(title, content, None).await
    }

    pub async fn read_document(&self, doc_id: &str) -> ApiResult<DocumentContent> {
        self.client.read_document(doc_id).await
    }

    pub async fn append_to_document(&self, doc_id: &str, content: &str) -> ApiResult<()> {
        self.client.append_to_document(doc_id, content).await
    }
}

/// Feishu calendar tool using trait abstractions
pub struct FeishuCalendarTool {
    client: Arc<FeishuClient>,
}

impl FeishuCalendarTool {
    pub fn new(client: Arc<FeishuClient>) -> Self {
        Self { client }
    }

    pub async fn list_calendars(&self) -> ApiResult<Vec<crate::channel::traits::Calendar>> {
        self.client.list_calendars().await
    }

    pub async fn get_primary_calendar(&self) -> ApiResult<crate::channel::traits::Calendar> {
        self.client.get_primary_calendar().await
    }

    pub async fn query_freebusy(
        &self,
        range: &crate::channel::traits::TimeRange,
        participants: &[String],
    ) -> ApiResult<Vec<crate::channel::traits::FreeBusyResult>> {
        self.client.query_freebusy(range, participants).await
    }
}
