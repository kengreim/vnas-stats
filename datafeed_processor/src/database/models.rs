use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::vnas::datafeed::UserRating as DatafeedUserRating;
use sqlx::postgres::types::PgInterval;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct QueuedDatafeed {
    pub id: Uuid,
    pub updated_at: DateTime<Utc>,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct ControllerNetworkSession {
    pub id: Uuid,
    pub controller_session_id: Uuid,
    pub login_time: DateTime<Utc>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration: Option<PgInterval>,
    pub last_seen: DateTime<Utc>,
    pub is_active: bool,
    pub connected_callsign: String,
    pub primary_position_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct ControllerSession {
    pub id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration: Option<PgInterval>,
    pub last_seen: DateTime<Utc>,
    pub is_active: bool,
    pub is_observer: bool,
    pub cid: i32,
    pub name: String,
    pub user_rating: UserRating,
    pub requested_rating: UserRating,
    pub connected_callsign: String,
    pub primary_position_id: String,
    pub callsign_session_id: Uuid,
    pub position_session_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct ActiveSessionKey {
    pub controller_session_id: Uuid,
    pub network_session_id: Uuid,
    pub cid: i32,
    pub login_time: DateTime<Utc>,
    pub connected_callsign: String,
    pub callsign_session_id: Uuid,
    pub primary_position_id: String,
    pub position_session_id: Uuid,
}

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct CallsignSession {
    pub id: Uuid,
    pub prefix: String,
    pub suffix: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration: Option<PgInterval>,
    pub last_seen: DateTime<Utc>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct PositionSession {
    pub id: Uuid,
    pub position_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration: Option<PgInterval>,
    pub last_seen: DateTime<Utc>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct PositionSessionDetails {
    pub id: Uuid,
    pub position_id: String,
    pub position_name: Option<String>,
    pub position_callsign: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, sqlx::Type, Deserialize, Serialize)]
#[sqlx(type_name = "user_rating", rename_all = "lowercase")]
pub enum UserRating {
    Observer,
    Student1,
    Student2,
    Student3,
    Controller1,
    Controller2,
    Controller3,
    Instructor1,
    Instructor2,
    Instructor3,
    Supervisor,
    Administrator,
}

impl From<DatafeedUserRating> for UserRating {
    fn from(value: DatafeedUserRating) -> Self {
        match value {
            DatafeedUserRating::Observer => Self::Observer,
            DatafeedUserRating::Student1 => Self::Student1,
            DatafeedUserRating::Student2 => Self::Student2,
            DatafeedUserRating::Student3 => Self::Student3,
            DatafeedUserRating::Controller1 => Self::Controller1,
            DatafeedUserRating::Controller2 => Self::Controller2,
            DatafeedUserRating::Controller3 => Self::Controller3,
            DatafeedUserRating::Instructor1 => Self::Instructor1,
            DatafeedUserRating::Instructor2 => Self::Instructor2,
            DatafeedUserRating::Instructor3 => Self::Instructor3,
            DatafeedUserRating::Supervisor => Self::Supervisor,
            DatafeedUserRating::Administrator => Self::Administrator,
        }
    }
}

// VATSIM facility type is still available from the datafeed models if needed later, but we do not persist it.
