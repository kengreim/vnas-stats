use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Serialize, Serializer};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorMessage {
    #[serde(serialize_with = "serialize_status")]
    pub status_code: StatusCode,
    pub message: String,
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
