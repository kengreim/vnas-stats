use crate::v1::session::constants::{CSRF_TOKEN_KEY, PKCE_VERIFIER_KEY, SESSION_USER_KEY};
use oauth2::{CsrfToken, PkceCodeVerifier};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

mod constants {
    // The key used in the session to store the user
    pub const SESSION_USER_KEY: &str = "user";
    pub const CSRF_TOKEN_KEY: &str = "oauth_csrf";
    pub const PKCE_VERIFIER_KEY: &str = "oauth_pkce_verifier";
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthUser {
    pub cid: u32,
}

pub async fn get_user(
    session: &Session,
) -> Result<Option<AuthUser>, tower_sessions::session::Error> {
    session.get(SESSION_USER_KEY).await
}

pub async fn remove_csrf_token(
    session: &Session,
) -> Result<Option<String>, tower_sessions::session::Error> {
    session.remove(CSRF_TOKEN_KEY).await
}

pub async fn remove_pkce_verifier(
    session: &Session,
) -> Result<Option<PkceCodeVerifier>, tower_sessions::session::Error> {
    session.remove(PKCE_VERIFIER_KEY).await
}

pub async fn insert_csrf_and_pkce(
    session: &Session,
    csrf_token: CsrfToken,
    pkce_verifier: PkceCodeVerifier,
) -> Result<(), tower_sessions::session::Error> {
    session.insert(CSRF_TOKEN_KEY, csrf_token.secret()).await?;
    session.insert(PKCE_VERIFIER_KEY, pkce_verifier).await?;
    Ok(())
}

pub async fn insert_user(
    session: &Session,
    user: AuthUser,
) -> Result<(), tower_sessions::session::Error> {
    session.insert(SESSION_USER_KEY, user).await?;
    Ok(())
}
