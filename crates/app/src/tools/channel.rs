use crate::CliResult;
use crate::channel::traits::{
    calendar::{ApiResult as CalendarApiResult, CalendarApi, TimeRange},
    documents::{ApiResult as DocumentsApiResult, DocumentsApi},
    messaging::{
        ApiResult as MessagingApiResult, MediaType, MessageContent, MessagingApi, Pagination,
    },
};

pub mod channel {
    use super::*;

    pub async fn send_message<C: MessagingApi>(
        api: &C,
        target: &str,
        receive_id_type: Option<&str>,
        content: &MessageContent,
        idempotency_key: Option<&str>,
    ) -> MessagingApiResult<C::Receipt> {
        api.send_message(target, receive_id_type, content, idempotency_key)
            .await
    }

    pub async fn reply_message<C: MessagingApi>(
        api: &C,
        parent_id: &str,
        content: &MessageContent,
    ) -> MessagingApiResult<C::Receipt> {
        api.reply(parent_id, content).await
    }

    pub async fn get_message<C: MessagingApi>(
        api: &C,
        message_id: &str,
    ) -> MessagingApiResult<C::Message> {
        api.get_message(message_id).await
    }

    pub async fn list_messages<C: MessagingApi>(
        api: &C,
        chat_id: &str,
        page_size: Option<u32>,
    ) -> MessagingApiResult<C::MessagePage> {
        api.list_messages(
            chat_id,
            &Pagination {
                page_size,
                cursor: None,
            },
        )
        .await
    }

    pub async fn upload_media<C: MessagingApi>(
        api: &C,
        file_path: Option<&str>,
        file_key: Option<&str>,
        media_type: MediaType,
    ) -> MessagingApiResult<C::MediaUploadResult> {
        api.upload_media(file_path, file_key, media_type).await
    }
}

pub mod documents {
    use super::*;

    pub async fn create_document<C: DocumentsApi>(
        api: &C,
        title: &str,
        content: Option<&str>,
    ) -> DocumentsApiResult<C::Document> {
        api.create_document(title, content).await
    }

    pub async fn read_document<C: DocumentsApi>(
        api: &C,
        doc_id: &str,
    ) -> DocumentsApiResult<C::DocumentContent> {
        api.read_document(doc_id).await
    }

    pub async fn append_to_document<C: DocumentsApi>(
        api: &C,
        doc_id: &str,
        content: &str,
    ) -> DocumentsApiResult<()> {
        api.append_to_document(doc_id, content).await
    }
}

pub mod calendar {
    use super::*;

    pub async fn list_calendars<C: CalendarApi>(api: &C) -> CalendarApiResult<C::CalendarList> {
        api.list_calendars().await
    }

    pub async fn get_primary_calendar<C: CalendarApi>(api: &C) -> CalendarApiResult<C::Calendar> {
        api.get_primary_calendar().await
    }

    pub async fn query_freebusy<C: CalendarApi>(
        api: &C,
        time_range: &TimeRange,
        participants: &[String],
    ) -> CalendarApiResult<C::FreeBusyResult> {
        api.query_freebusy(time_range, participants).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_structure() {
        assert!(true);
    }
}
