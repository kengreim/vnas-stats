use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::vnas::datafeed::UserRating as DatafeedUserRating;
use shared::vnas::datafeed::VatsimFacilityType as DatafeedVatsimFacilityType;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct ControllerSession {
    pub id: Uuid,
    pub login_time: DateTime<Utc>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration: sqlx::postgres::types::PgInterval,
    pub is_active: bool,
    pub is_observer: bool,
    pub cid: i32,
    pub name: String,
    pub user_rating: UserRating,
    pub requested_rating: UserRating,
    pub callsign: String,
    pub facility_type: String,
    pub primary_frequency: i32,
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

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, sqlx::Type, Deserialize, Serialize)]
#[sqlx(type_name = "vatsim_facility_type", rename_all = "snake_case")]
pub enum VatsimFacilityType {
    Observer,
    FlightServiceStation,
    ClearanceDelivery,
    Ground,
    Tower,
    ApproachDeparture,
    Center,
}

impl From<DatafeedVatsimFacilityType> for VatsimFacilityType {
    fn from(value: DatafeedVatsimFacilityType) -> Self {
        match value {
            DatafeedVatsimFacilityType::Observer => Self::Observer,
            DatafeedVatsimFacilityType::FlightServiceStation => Self::FlightServiceStation,
            DatafeedVatsimFacilityType::ClearanceDelivery => Self::ClearanceDelivery,
            DatafeedVatsimFacilityType::Ground => Self::Ground,
            DatafeedVatsimFacilityType::Tower => Self::Tower,
            DatafeedVatsimFacilityType::ApproachDeparture => Self::ApproachDeparture,
            DatafeedVatsimFacilityType::Center => Self::Center,
        }
    }
}
