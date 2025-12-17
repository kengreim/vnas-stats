use crate::v1::db::queries::QueryError;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use oauth2::basic::BasicErrorResponse;
use oauth2::{HttpClientError, RequestTokenError};
use serde::{Serialize, Serializer};
use thiserror::Error;
use tracing::warn;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorMessage {
    #[serde(serialize_with = "serialize_status")]
    pub status_code: StatusCode,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    Session(#[from] tower_sessions::session::Error),
    #[error("missing CSRF token")]
    MissingCsrfToken,
    #[error("invalid CSRF token: {0}")]
    InvalidCsrfToken(String),
    #[error("missing PKCE verifier")]
    MissingPkceVerifier,
    #[error("transparent")]
    TokenRequest(#[from] RequestTokenError<HttpClientError<reqwest::Error>, BasicErrorResponse>),
    #[error(transparent)]
    VatsimDetailsFetch(#[from] reqwest::Error),
    #[error(transparent)]
    QueryError(#[from] QueryError),
    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::Session(e) => {
                warn!(error = ?e, "session error");
                ErrorMessage::from((StatusCode::INTERNAL_SERVER_ERROR, "internal server error"))
                    .into_response()
            }
            ApiError::InvalidCsrfToken(token) => {
                warn!(token = token, "invalid CSRF token");
                ErrorMessage::from((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("token {token} is invalid"),
                ))
                .into_response()
            }
            ApiError::MissingCsrfToken => {
                warn!("missing CSRF token");
                ErrorMessage::from((StatusCode::BAD_REQUEST, "missing CSRF token")).into_response()
            }
            ApiError::MissingPkceVerifier => {
                warn!("missing PKCE verifier");
                ErrorMessage::from((StatusCode::INTERNAL_SERVER_ERROR, "missing PKCE verifier"))
                    .into_response()
            }
            ApiError::TokenRequest(e) => {
                warn!(error = ?e, "failed to request VATSIM Connect token");
                ErrorMessage::from((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "VATSIM Connect access token request failed",
                ))
                .into_response()
            }
            ApiError::VatsimDetailsFetch(e) => {
                warn!(error = ?e, "failed to fetch VATSIM details");
                ErrorMessage::from((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "VATSIM details request failed",
                ))
                .into_response()
            }
            ApiError::QueryError(e) => match e {
                QueryError::Sql(e) => {
                    warn!(error = ?e, "sql error");
                    ErrorMessage::from((StatusCode::INTERNAL_SERVER_ERROR, "")).into_response()
                }
                QueryError::IllegalArgs(e) => {
                    warn!(error = e, "illegal arguments for Db query");
                    ErrorMessage::from((StatusCode::BAD_REQUEST, e)).into_response()
                }
            },
            ApiError::ServiceUnavailable(e) => {
                warn!(error = e, "service unavailable");
                ErrorMessage::from((StatusCode::SERVICE_UNAVAILABLE, "")).into_response()
            }
        }
    }
}

fn serialize_status<S>(value: &StatusCode, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u16(value.as_u16())
}

impl From<(StatusCode, String)> for ErrorMessage {
    fn from((status_code, message): (StatusCode, String)) -> Self {
        Self {
            status_code,
            message,
        }
    }
}

impl From<(StatusCode, &str)> for ErrorMessage {
    fn from((status_code, message): (StatusCode, &str)) -> Self {
        Self {
            status_code,
            message: message.into(),
        }
    }
}

impl IntoResponse for ErrorMessage {
    fn into_response(self) -> Response {
        (self.status_code, Json(self)).into_response()
    }
}

//
// pub fn to_error_message(code: StatusCode, msg: impl Into<String>) -> ErrorMessage {
//     ErrorMessage {
//         status_code: code,
//         message: msg.into(),
//     }
// }
