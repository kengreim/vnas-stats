use axum::extract::FromRef;
use oauth2::{
    Client, EndpointNotSet, EndpointSet, StandardRevocableToken,
    basic::{
        BasicErrorResponse, BasicRevocationErrorResponse, BasicTokenIntrospectionResponse,
        BasicTokenResponse,
    },
};
use shared::vatsim::OauthEnvironment;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

pub type OauthClient = Client<
    BasicErrorResponse,
    BasicTokenResponse,
    BasicTokenIntrospectionResponse,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
    EndpointSet,    // AuthUrl
    EndpointNotSet, // DeviceAuthUrl
    EndpointNotSet, // IntrospectionUrl
    EndpointNotSet, // RevocationUrl
    EndpointSet,    // TokenUrl
>;

#[derive(Clone, FromRef)]
pub struct AppState {
    pub db: Db,
    pub oauth: Oauth,
    pub http_clients: HttpClients,
}

#[derive(Clone)]
pub struct Db {
    pub pool: Pool<Postgres>,
}

#[derive(Clone)]
pub struct HttpClients {
    pub standard: reqwest::Client,
    pub no_redirect: reqwest::Client,
}

#[derive(Clone)]
pub struct Oauth {
    pub client: Arc<OauthClient>,
    pub environment: OauthEnvironment,
    pub frontend_login_success_url: String,
}

//
// impl axum::extract::FromRef<AppState> for Pool<Postgres> {
//     fn from_ref(state: &AppState) -> Self {
//         state.pool.clone()
//     }
// }
//
// impl axum::extract::FromRef<AppState> for Option<OauthClient> {
//     fn from_ref(state: &AppState) -> Self {
//         state.oauth_client.clone()
//     }
// }
