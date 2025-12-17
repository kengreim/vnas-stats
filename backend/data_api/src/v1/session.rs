use serde::{Deserialize, Serialize};

pub mod constants {
    // The key used in the session to store the user
    pub const SESSION_USER_KEY: &str = "user";
    pub const CSRF_TOKEN_KEY: &str = "oauth_csrf";
    pub const PKCE_VERIFIER_KEY: &str = "oauth_pkce_verifier";
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthUser {
    pub cid: u32,
}
