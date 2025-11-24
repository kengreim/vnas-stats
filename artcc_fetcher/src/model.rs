use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct FlatFacility {
    pub id: String,
    pub root_artcc_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub facility_type: String,
    pub last_updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct FlatPosition {
    pub id: String,
    pub facility_id: String,
    pub name: String,
    pub callsign: Option<String>,
    pub radio_name: Option<String>,
    pub frequency: Option<i64>,
    pub starred: bool,
    pub last_updated_at: DateTime<Utc>,
}
