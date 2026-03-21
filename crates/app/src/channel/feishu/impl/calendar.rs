use crate::channel::feishu::FeishuClient;
use crate::channel::feishu::resources::calendar::{self as cal, FeishuCalendarListPage};
use crate::channel::traits::calendar::{ApiResult, CalendarApi, TimeRange};

pub(super) struct FeishuCalendarImpl {
    client: FeishuClient,
}

impl FeishuCalendarImpl {
    pub(super) fn new(client: FeishuClient) -> Self {
        Self { client }
    }
}

#[derive(Clone, Debug)]
pub struct FeishuCalendar {
    pub calendar_id: String,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct FeishuCalendarList {
    pub calendars: Vec<FeishuCalendar>,
    pub has_more: bool,
}

#[derive(Clone, Debug)]
pub struct FeishuFreeBusyResult {
    pub busy_slots: Vec<FeishuBusySlot>,
}

#[derive(Clone, Debug)]
pub struct FeishuBusySlot {
    pub start_time: String,
    pub end_time: String,
}

#[async_trait::async_trait]
impl CalendarApi for FeishuCalendarImpl {
    type Calendar = FeishuCalendar;
    type CalendarList = FeishuCalendarList;
    type FreeBusyResult = FeishuFreeBusyResult;

    async fn list_calendars(&self) -> ApiResult<Self::CalendarList> {
        let token = self
            .client
            .get_tenant_access_token()
            .await
            .map_err(|e| crate::channel::traits::calendar::ApiError::Auth(e.to_string()))?;

        let page: FeishuCalendarListPage = cal::list_calendars(&self.client, &token, None)
            .await
            .map_err(|e| {
                crate::channel::traits::calendar::ApiError::InvalidRequest(e.to_string())
            })?;

        let calendars = page
            .calendar_list
            .unwrap_or_default()
            .into_iter()
            .map(|c| FeishuCalendar {
                calendar_id: c.calendar_id.unwrap_or_default(),
                summary: c.summary.unwrap_or_default(),
            })
            .collect();

        Ok(FeishuCalendarList {
            calendars,
            has_more: page.has_more.unwrap_or(false),
        })
    }

    async fn get_primary_calendar(&self) -> ApiResult<Self::Calendar> {
        let token = self
            .client
            .get_tenant_access_token()
            .await
            .map_err(|e| crate::channel::traits::calendar::ApiError::Auth(e.to_string()))?;

        let cal_response = cal::get_primary_calendar(&self.client, &token)
            .await
            .map_err(|e| {
                crate::channel::traits::calendar::ApiError::InvalidRequest(e.to_string())
            })?;

        Ok(FeishuCalendar {
            calendar_id: cal_response.calendar_id.unwrap_or_default(),
            summary: cal_response.summary.unwrap_or_default(),
        })
    }

    async fn query_freebusy(
        &self,
        time_range: &TimeRange,
        participants: &[String],
    ) -> ApiResult<Self::FreeBusyResult> {
        let token = self
            .client
            .get_tenant_access_token()
            .await
            .map_err(|e| crate::channel::traits::calendar::ApiError::Auth(e.to_string()))?;

        let query = cal::FeishuCalendarFreebusyQuery {
            time_min: time_range.start_timestamp.to_string(),
            time_max: time_range.end_timestamp.to_string(),
            users: participants.iter().map(|s| s.as_str()).collect(),
            ..Default::default()
        };

        let result = cal::get_freebusy(&self.client, &token, &query)
            .await
            .map_err(|e| {
                crate::channel::traits::calendar::ApiError::InvalidRequest(e.to_string())
            })?;

        let busy_slots = result
            .freebusy_slots
            .unwrap_or_default()
            .into_iter()
            .map(|slot| FeishuBusySlot {
                start_time: slot.start_time.unwrap_or_default(),
                end_time: slot.end_time.unwrap_or_default(),
            })
            .collect();

        Ok(FeishuFreeBusyResult { busy_slots })
    }
}
