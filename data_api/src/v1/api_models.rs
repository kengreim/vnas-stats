use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AllSessionDto<T> {
    pub active: ActiveSessionsDto<T>,
    pub completed: CompletedSessionsDto<T>,
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSessionsDto<T> {
    pub requested_at: DateTime<Utc>,
    pub datafeed_last_updated_at: Option<DateTime<Utc>>,
    pub sessions: Vec<T>,
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CompletedSessionsDto<T> {
    pub count: usize,
    pub total_duration_seconds: i64,
    pub sessions: Vec<T>,
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ControllerSessionDetailsDto {
    pub id: Uuid,
    pub cid: i32,
    pub connected_callsign: String,
    pub primary_position_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i64>,
    pub seconds_since_start_time: i64,
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CallsignSessionDetailsDto {
    pub id: Uuid,
    pub prefix: String,
    pub suffix: String,
    pub start_time: DateTime<Utc>,
    pub seconds_since_start_time: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PositionSessionDetailsDto {
    pub id: Uuid,
    pub position_id: String,
    pub start_time: DateTime<Utc>,
    pub seconds_since_start_time: i64,
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CallsignSessionAggregate {
    pub prefix: String,
    pub suffix: String,
    pub total_duration_seconds: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallsignSessionsGroupedDto {
    pub requested_at: DateTime<Utc>,
    pub datafeed_last_updated_at: Option<DateTime<Utc>>,
    pub sessions: Vec<CallsignSessionAggregate>,
}
