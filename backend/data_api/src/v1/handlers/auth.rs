use crate::state::{HttpClients, OauthClient};
use crate::v1::error::ApiError;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use oauth2::{AuthorizationCode, CsrfToken, PkceCodeChallenge, Scope, TokenResponse};
use serde::{Deserialize, Serialize};
use shared::vatsim;
use tower_sessions::Session;

// The key used in the session to store the user
const SESSION_USER_KEY: &str = "user";
const CSRF_TOKEN_KEY: &str = "oauth_csrf";
const PKCE_VERIFIER_KEY: &str = "oauth_pkce_verifier";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthUser {
    pub cid: String,
}

#[derive(Deserialize)]
pub struct AuthCallbackParams {
    code: String,
    state: String,
}

pub async fn login(
    State(oauth_client): State<OauthClient>,
    session: Session,
) -> Result<impl IntoResponse, ApiError> {
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token) = oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(shared::vatsim::Scope::VatsimDetails.to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    session.insert(CSRF_TOKEN_KEY, csrf_token.secret()).await?;
    session.insert(PKCE_VERIFIER_KEY, pkce_verifier).await?;

    Ok(Redirect::to(auth_url.as_str()))
}

pub async fn callback(
    State(oauth_client): State<OauthClient>,
    State(http_clients): State<HttpClients>,
    session: Session,
    Query(params): Query<AuthCallbackParams>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify CSRF
    let stored_csrf: Option<String> = session.get(CSRF_TOKEN_KEY).await?;

    if stored_csrf.is_none() {
        return Err(ApiError::MissingCsrfToken);
    }

    if stored_csrf.unwrap() != params.state {
        return Err(ApiError::InvalidCsrfToken(params.state));
    }

    let pkce_verifier = session
        .get(PKCE_VERIFIER_KEY)
        .await?
        .ok_or(ApiError::MissingPkceVerifier)?;

    // Exchange code
    let token_result = oauth_client
        .exchange_code(AuthorizationCode::new(params.code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(&http_clients.no_redirect)
        .await?;

    // Fetch VATSIM user details
    let user_data =
        vatsim::fetch_user_details(&http_clients.standard, token_result.access_token().secret())
            .await?;

    let auth_user = AuthUser { cid: user_data.cid };
    session.insert(SESSION_USER_KEY, auth_user).await?;

    // Redirect to frontend root
    Ok(Redirect::to("/"))
}

pub async fn logout(session: Session) -> Result<impl IntoResponse, ApiError> {
    session.delete().await?;
    Ok(Redirect::to("/"))
}

pub async fn me(session: Session) -> impl IntoResponse {
    let user: Option<AuthUser> = session.get(SESSION_USER_KEY).await.unwrap_or(None);
    match user {
        Some(u) => (StatusCode::OK, Json(Some(u))).into_response(),
        None => (StatusCode::UNAUTHORIZED, Json(None::<AuthUser>)).into_response(),
    }
}
