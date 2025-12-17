use axum::extract::Request;
use axum::{middleware::Next, response::IntoResponse};
use tower_sessions::Session;

use crate::v1::error::ApiError;
use crate::v1::session::AuthUser;
use crate::v1::session::constants::SESSION_USER_KEY;

pub async fn require_auth(
    session: Session,
    req: Request,
    next: Next,
) -> Result<impl IntoResponse, ApiError> {
    let Some(_): Option<AuthUser> = session.get(SESSION_USER_KEY).await? else {
        return Err(ApiError::AuthRequired);
    };

    Ok(next.run(req).await)
}
