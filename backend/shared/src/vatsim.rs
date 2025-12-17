use reqwest::Client;
use serde::Deserialize;
use std::fmt::Display;

pub enum Scope {
    FullName,
    Email,
    VatsimDetails,
    Country,
}

impl Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FullName => write!(f, "full_name"),
            Self::Email => write!(f, "email"),
            Self::VatsimDetails => write!(f, "vatsim_details"),
            Self::Country => write!(f, "country"),
        }
    }
}

#[derive(Deserialize)]
struct VatsimUserResponse {
    data: VatsimUserData,
}

#[derive(Deserialize)]
pub struct VatsimUserData {
    pub cid: String,
    pub personal: Option<VatsimPersonalData>,
    pub vatsim: Option<VatsimDetails>,
    pub oauth: Oauth,
}

#[derive(Deserialize)]
pub struct VatsimPersonalData {
    pub name_first: Option<String>,
    pub mame_last: Option<String>,
    pub name_full: Option<String>,
    pub email: Option<String>,
    pub country: Option<IdName>,
}

#[derive(Deserialize)]
pub struct IdShortLong {
    pub id: String,
    pub short: String,
    pub long: String,
}

#[derive(Deserialize)]
pub struct IdName {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct VatsimDetails {
    pub rating: IdShortLong,
    #[serde(rename = "pilotrating")]
    pub pilot_rating: IdShortLong,
    pub region: IdName,
    pub division: IdName,
    pub subdivision: IdName,
}

#[derive(Deserialize)]
pub struct Oauth {
    pub token_valid: String,
}

pub async fn fetch_user_details(
    client: &Client,
    access_token: impl ToString,
) -> Result<VatsimUserData, reqwest::Error> {
    const USER_DETAILS_URL: &str = "https://auth.vatsim.net/api/user";

    let user_resp = client
        .get(USER_DETAILS_URL)
        .bearer_auth(access_token.to_string())
        .send()
        .await?
        .error_for_status()?;

    Ok(user_resp.json::<VatsimUserResponse>().await?.data)
}
