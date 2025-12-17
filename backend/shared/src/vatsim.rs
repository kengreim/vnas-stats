use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
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
#[serde(untagged)]
pub enum VatsimPersonalData {
    Empty(Vec<Value>),
    Data(VatsimPersonalDataInner),
}

#[derive(Deserialize)]
pub struct VatsimPersonalDataInner {
    pub name_first: Option<String>,
    pub mame_last: Option<String>,
    pub name_full: Option<String>,
    pub email: Option<String>,
    pub country: Option<IdName>,
}

#[derive(Deserialize)]
pub struct IdShortLong {
    pub id: Option<String>,
    pub short: Option<String>,
    pub long: Option<String>,
}

#[derive(Deserialize)]
pub struct Rating {
    pub id: i16,
    pub short: String,
    pub long: String,
}

#[derive(Deserialize)]
pub struct IdName {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct VatsimDetails {
    pub rating: Rating,
    #[serde(rename = "pilotrating")]
    pub pilot_rating: Rating,
    pub region: IdName,
    pub division: IdName,
    pub subdivision: IdName,
}

#[derive(Deserialize)]
pub struct Oauth {
    pub token_valid: String,
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub enum OauthEnvironment {
    Live,
    Development,
}

pub struct OauthEndpoints {
    pub auth_url: String,
    pub token_url: String,
    pub user_details_url: String,
}

impl From<OauthEnvironment> for OauthEndpoints {
    fn from(env: OauthEnvironment) -> Self {
        match env {
            OauthEnvironment::Live => Self {
                auth_url: "https://auth.vatsim.net/oauth/authorize".to_string(),
                token_url: "https://auth.vatsim.net/oauth/token".to_string(),
                user_details_url: "https://auth.vatsim.net/api/user".to_string(),
            },
            OauthEnvironment::Development => Self {
                auth_url: "https://auth-dev.vatsim.net/oauth/authorize".to_string(),
                token_url: "https://auth-dev.vatsim.net/oauth/token".to_string(),
                user_details_url: "https://auth-dev.vatsim.net/api/user".to_string(),
            },
        }
    }
}

pub async fn fetch_user_details(
    client: &Client,
    env: OauthEnvironment,
    access_token: impl ToString,
) -> Result<VatsimUserData, reqwest::Error> {
    let endpoints = OauthEndpoints::from(env);

    let user_resp = client
        .get(endpoints.user_details_url)
        .bearer_auth(access_token.to_string())
        .send()
        .await?
        .error_for_status()?;

    Ok(user_resp.json::<VatsimUserResponse>().await?.data)
}
