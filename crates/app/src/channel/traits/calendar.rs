use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::error::{ApiResult, PlatformApi};

#[async_trait]
pub trait CalendarApi: PlatformApi {
    async fn list_calendars(&self) -> ApiResult<Vec<Calendar>>;

    async fn get_primary_calendar(&self) -> ApiResult<Calendar>;

    async fn get_calendar(&self, calendar_id: &str) -> ApiResult<Calendar>;

    async fn query_freebusy(
        &self,
        time_range: &TimeRange,
        participants: &[String],
    ) -> ApiResult<Vec<FreeBusyResult>>;

    async fn create_event(
        &self,
        calendar_id: &str,
        event: &CreateEventRequest,
    ) -> ApiResult<CalendarEvent>;

    async fn list_events(
        &self,
        calendar_id: &str,
        time_range: &TimeRange,
    ) -> ApiResult<Vec<CalendarEvent>>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Calendar {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_primary: bool,
    pub timezone: Option<String>,
    pub platform_metadata: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub calendar_id: String,
    pub title: String,
    pub description: Option<String>,
    #[serde(
        serialize_with = "super::messaging::serialize_timestamp",
        deserialize_with = "super::messaging::deserialize_timestamp"
    )]
    pub start_time: chrono::DateTime<chrono::Utc>,
    #[serde(
        serialize_with = "super::messaging::serialize_timestamp",
        deserialize_with = "super::messaging::deserialize_timestamp"
    )]
    pub end_time: chrono::DateTime<chrono::Utc>,
    pub location: Option<String>,
    pub organizer_id: String,
    pub attendee_ids: Vec<String>,
    pub status: EventStatus,
    pub platform_metadata: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    Confirmed,
    Tentative,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateEventRequest {
    pub title: String,
    pub description: Option<String>,
    #[serde(
        serialize_with = "super::messaging::serialize_timestamp",
        deserialize_with = "super::messaging::deserialize_timestamp"
    )]
    pub start_time: chrono::DateTime<chrono::Utc>,
    #[serde(
        serialize_with = "super::messaging::serialize_timestamp",
        deserialize_with = "super::messaging::deserialize_timestamp"
    )]
    pub end_time: chrono::DateTime<chrono::Utc>,
    pub location: Option<String>,
    pub attendee_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeRange {
    #[serde(
        serialize_with = "super::messaging::serialize_timestamp",
        deserialize_with = "super::messaging::deserialize_timestamp"
    )]
    pub start: chrono::DateTime<chrono::Utc>,
    #[serde(
        serialize_with = "super::messaging::serialize_timestamp",
        deserialize_with = "super::messaging::deserialize_timestamp"
    )]
    pub end: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FreeBusyResult {
    pub user_id: String,
    pub freebusy: Vec<TimeRange>,
}
