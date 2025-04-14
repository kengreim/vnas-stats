use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub enum VnasEnvironment {
    Live,
    Sweatbox1,
    Sweatbox2,
    Test,
}

pub const fn datafeed_url(env: VnasEnvironment) -> &'static str {
    match env {
        VnasEnvironment::Live => "https://live.env.vnas.vatsim.net/data-feed/controllers.json",
        VnasEnvironment::Sweatbox1 => {
            "https://sweatbox1.env.vnas.vatsim.net/data-feed/controllers.json"
        }
        VnasEnvironment::Sweatbox2 => {
            "https://sweatbox2.env.vnas.vatsim.net/data-feed/controllers.json"
        }
        VnasEnvironment::Test => "https://test.virtualnas.net/data-feed/controllers.json",
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct DatafeedRoot {
    pub updated_at: DateTime<Utc>,
    pub controllers: Vec<Controller>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Controller {
    pub artcc_id: String,
    pub primary_facility_id: String,
    pub primary_position_id: String,
    pub role: Role,
    pub positions: Vec<Position>,
    pub is_active: bool,
    pub is_observer: bool,
    pub login_time: DateTime<Utc>,
    pub vatsim_data: VatsimData,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum Role {
    Observer,
    Controller,
    Student,
    Instructor,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum PositionType {
    Artcc,
    Tracon,
    Atct,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct VatsimData {
    pub cid: String,
    pub real_name: String,
    pub controller_info: String,
    pub user_rating: UserRating,
    pub requested_rating: UserRating,
    pub callsign: String,
    pub facility_type: VatsimFacilityType,
    pub primary_frequency: i32,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum VatsimFacilityType {
    Observer,
    FlightServiceStation,
    ClearanceDelivery,
    Ground,
    Tower,
    ApproachDeparture,
    Center,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub facility_id: String,
    pub facility_name: String,
    pub position_id: String,
    pub position_name: String,
    pub position_type: PositionType,
    pub radio_name: String,
    pub default_callsign: String,
    pub frequency: i32,
    pub is_primary: bool,
    pub is_active: bool,
    pub eram_data: Option<EramPositionData>,
    pub stars_data: Option<StarsPositionData>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct EramPositionData {
    pub sector_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct StarsPositionData {
    pub subset: i32,
    pub sector_id: String,
    pub area_id: String,
}
