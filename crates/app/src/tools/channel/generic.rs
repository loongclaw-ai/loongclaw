#![allow(dead_code)]

use crate::channel::traits::{
    ApiResult, Calendar, CalendarApi, Document, DocumentContent, DocumentsApi, FreeBusyResult,
    MediaType, MediaUploadResult, Message, MessageContent, MessagingApi, Pagination, TimeRange,
};

pub async fn send_message(
    api: &dyn MessagingApi,
    target: &str,
    content: &MessageContent,
) -> ApiResult<String> {
    api.send_message(target, content).await
}

pub async fn reply_message(
    api: &dyn MessagingApi,
    parent_id: &str,
    content: &MessageContent,
) -> ApiResult<String> {
    api.reply(parent_id, content).await
}

pub async fn get_message(api: &dyn MessagingApi, message_id: &str) -> ApiResult<Message> {
    api.get_message(message_id).await
}

pub async fn list_messages(
    api: &dyn MessagingApi,
    chat_id: &str,
    pagination: &Pagination,
) -> ApiResult<Vec<Message>> {
    api.list_messages(chat_id, pagination).await
}

pub async fn upload_media(
    api: &dyn MessagingApi,
    file_path: &std::path::Path,
    media_type: MediaType,
) -> ApiResult<MediaUploadResult> {
    api.upload_media(file_path, media_type).await
}

pub async fn create_document(
    api: &dyn DocumentsApi,
    title: &str,
    content: Option<&str>,
) -> ApiResult<Document> {
    api.create_document(title, content, None).await
}

pub async fn read_document(api: &dyn DocumentsApi, doc_id: &str) -> ApiResult<DocumentContent> {
    api.read_document(doc_id).await
}

pub async fn list_calendars(api: &dyn CalendarApi) -> ApiResult<Vec<Calendar>> {
    api.list_calendars().await
}

pub async fn query_freebusy(
    api: &dyn CalendarApi,
    range: &TimeRange,
    participants: &[String],
) -> ApiResult<Vec<FreeBusyResult>> {
    api.query_freebusy(range, participants).await
}
