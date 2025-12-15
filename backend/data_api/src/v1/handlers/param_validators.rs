use crate::v1::handlers::error_into_response;
use axum::http::StatusCode;
use axum::response::Response;
use chrono::{DateTime, Utc};
use std::time::Duration;

pub fn validate_duration_no_longer_than(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    max_duration: Duration,
) -> Result<(DateTime<Utc>, DateTime<Utc>), Response> {
    let now = Utc::now();

    if end <= start {
        return Err(error_into_response(
            StatusCode::BAD_REQUEST,
            "end must be greater than start",
        ));
    }

    if start > now {
        return Err(error_into_response(
            StatusCode::BAD_REQUEST,
            "start must be greater than now",
        ));
    }

    if (end - start).num_seconds() > max_duration.as_secs() as i64 {
        let duration_str = format!(
            "end must be {} seconds or less after start",
            max_duration.as_secs()
        );
        return Err(error_into_response(StatusCode::BAD_REQUEST, duration_str));
    }

    Ok((start, end))
}
