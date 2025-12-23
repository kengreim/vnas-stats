use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const API_BASE: &str = "https://api.vatusa.net/v2";

#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct VatusaUserDto {
    pub data: VatusaUserData,
    pub testing: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VatusaUserData {
    pub cid: i32,
    pub fname: String,
    pub lname: String,
    pub email: Option<String>,
    pub facility: String,
    pub rating: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub flag_needbasic: bool,
    #[serde(rename = "flag_xferOverride")]
    pub flag_xfer_override: bool,
    pub facility_join: String,
    pub flag_homecontroller: bool,
    pub lastactivity: DateTime<Utc>,
    pub discord_id: u64,
    pub last_cert_sync: String,
    pub flag_nameprivacy: bool,
    pub last_competency_date: Option<DateTime<Utc>>,
    pub promotion_eligible: bool,
    pub transfer_eligible: Option<bool>,
    pub roles: Vec<Role>,
    pub rating_short: String,
    pub visiting_facilities: Vec<VisitingFacility>,
    #[serde(rename = "isMentor")]
    pub is_mentor: bool,
    #[serde(rename = "isSupIns")]
    pub is_sup_ins: bool,
    pub last_promotion: Option<DateTime<Utc>>,
}

#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Role {
    pub id: i64,
    pub cid: i32,
    pub facility: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct VisitingFacility {
    pub id: i64,
    pub cid: i32,
    pub facility: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct VatusaClient {
    client: Client,
}

impl VatusaClient {
    pub const fn new_with_client(client: Client) -> Self {
        Self { client }
    }

    pub async fn get_user_from_cid(&self, cid: i32) -> Result<VatusaUserData, reqwest::Error> {
        let url = format!("{API_BASE}/user/{cid}");
        self.send(&url).await
    }

    pub async fn get_user_from_discord_id(
        &self,
        discord_id: u64,
    ) -> Result<VatusaUserData, reqwest::Error> {
        let url = format!("{API_BASE}/user/{discord_id}?d");
        self.send(&url).await
    }

    async fn send(&self, url: &str) -> Result<VatusaUserData, reqwest::Error> {
        self.client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<VatusaUserDto>()
            .await
            .map(|d| d.data)
    }
}
