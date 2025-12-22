use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const API_BASE: &str = "https://api.vatsim.net/v2";

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VatsimUserData {
    pub id: i32,
    pub rating: i8,
    #[serde(rename = "pilotrating")]
    pub pilot_rating: i8,
    #[serde(rename = "militaryrating")]
    pub military_rating: i8,
    pub susp_date: Option<DateTime<Utc>>,
    pub reg_date: DateTime<Utc>,
    pub region_id: String,
    pub division_id: String,
    pub subdivision_id: Option<String>,
    #[serde(rename = "lastratingchange")]
    pub last_rating_change: Option<DateTime<Utc>>,
}

#[derive(Deserialize, Debug)]
struct DiscordIdDto {
    id: String,
    user_id: String,
}

#[derive(Error, Debug)]
pub enum VatsimClientError {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),
}

#[derive(Clone)]
pub struct VatsimClient {
    client: Client,
}

impl VatsimClient {
    pub const fn new_with_client(client: Client) -> Self {
        Self { client }
    }

    pub async fn get_cid_from_discord_id(&self, discord_id: u64) -> Result<i32, VatsimClientError> {
        let url = format!("{API_BASE}/members/discord/{discord_id}");
        self.client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<DiscordIdDto>()
            .await?
            .user_id
            .parse::<i32>()
            .map_err(VatsimClientError::ParseInt)
    }

    pub async fn get_user_from_cid(&self, cid: i32) -> Result<VatsimUserData, VatsimClientError> {
        let url = format!("{API_BASE}/members/{cid}");
        self.client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<VatsimUserData>()
            .await
            .map_err(VatsimClientError::Reqwest)
    }

    pub async fn get_user_from_discord_id(
        &self,
        discord_id: u64,
    ) -> Result<VatsimUserData, VatsimClientError> {
        let cid = self.get_cid_from_discord_id(discord_id).await?;
        self.get_user_from_cid(cid).await
    }
}
