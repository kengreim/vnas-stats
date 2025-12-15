use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Serialize};

pub mod active_sessions;
pub mod stats;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorMessage {
    pub status_code: u16,
    pub message: String,
}

pub fn error_into_response(code: StatusCode, msg: impl Into<String>) -> Response {
    (
        code,
        Json(ErrorMessage {
            status_code: code.into(),
            message: msg.into(),
        }),
    )
        .into_response()
}

impl From<(StatusCode, String)> for ErrorMessage {
    fn from((status_code, message): (StatusCode, String)) -> Self {
        Self {
            status_code: status_code.into(),
            message,
        }
    }
}

impl From<(StatusCode, &str)> for ErrorMessage {
    fn from((status_code, message): (StatusCode, &str)) -> Self {
        Self {
            status_code: status_code.into(),
            message: message.into(),
        }
    }
}