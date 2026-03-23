use async_trait::async_trait;

use crate::channel::feishu::api::client::FeishuClient;
use crate::channel::traits::{
    ApiError, ApiResult, Calendar, CalendarApi, CalendarEvent, CreateEventRequest, FreeBusyResult,
    TimeRange,
};

#[async_trait]
impl CalendarApi for FeishuClient {
    async fn list_calendars(&self) -> ApiResult<Vec<Calendar>> {
        let token = self
            .get_tenant_access_token()
            .await
            .map_err(|e| ApiError::Auth {
                message: e,
                retry_after: None,
            })?;

        let query = crate::feishu::resources::calendar::FeishuCalendarListQuery::default();
        let page = crate::feishu::resources::calendar::list_calendars(self, &token, &query)
            .await
            .map_err(|e| ApiError::Platform {
                platform: "feishu".to_string(),
                code: "calendar_list_failed".to_string(),
                message: e,
                raw: None,
            })?;

        let calendars: Vec<Calendar> = page
            .calendar_list
            .into_iter()
            .map(|entry| Calendar {
                id: entry.calendar_id,
                name: entry.summary.unwrap_or_default(),
                description: entry.description,
                is_primary: entry.calendar_type.as_deref() == Some("primary"),
                timezone: None,
                platform_metadata: serde_json::Value::Null,
            })
            .collect();

        Ok(calendars)
    }

    async fn get_primary_calendar(&self) -> ApiResult<Calendar> {
        let token = self
            .get_tenant_access_token()
            .await
            .map_err(|e| ApiError::Auth {
                message: e,
                retry_after: None,
            })?;

        let primary_list =
            crate::feishu::resources::calendar::get_primary_calendars(self, &token, None)
                .await
                .map_err(|e| ApiError::Platform {
                    platform: "feishu".to_string(),
                    code: "primary_calendar_failed".to_string(),
                    message: e,
                    raw: None,
                })?;

        let primary =
            primary_list
                .calendars
                .into_iter()
                .next()
                .ok_or_else(|| ApiError::NotFound {
                    resource: "primary_calendar".to_string(),
                    id: None,
                })?;

        Ok(Calendar {
            id: primary.calendar.calendar_id,
            name: primary.calendar.summary.unwrap_or_default(),
            description: primary.calendar.description,
            is_primary: true,
            timezone: None,
            platform_metadata: serde_json::Value::Null,
        })
    }

    async fn get_calendar(&self, calendar_id: &str) -> ApiResult<Calendar> {
        let calendars = self.list_calendars().await?;
        calendars
            .into_iter()
            .find(|c| c.id == calendar_id)
            .ok_or(ApiError::NotFound {
                resource: "calendar".to_string(),
                id: Some(calendar_id.to_string()),
            })
    }

    async fn query_freebusy(
        &self,
        _time_range: &TimeRange,
        _participants: &[String],
    ) -> ApiResult<Vec<FreeBusyResult>> {
        Err(ApiError::NotSupported {
            operation: "query_freebusy".to_string(),
            platform: "feishu".to_string(),
        })
    }

    async fn create_event(
        &self,
        _calendar_id: &str,
        _event: &CreateEventRequest,
    ) -> ApiResult<CalendarEvent> {
        Err(ApiError::NotSupported {
            operation: "create_event".to_string(),
            platform: "feishu".to_string(),
        })
    }

    async fn list_events(
        &self,
        _calendar_id: &str,
        _time_range: &TimeRange,
    ) -> ApiResult<Vec<CalendarEvent>> {
        Err(ApiError::NotSupported {
            operation: "list_events".to_string(),
            platform: "feishu".to_string(),
        })
    }
}
