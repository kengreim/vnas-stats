use crate::v1::error::ErrorMessage;
use axum::{
    extract::{FromRequestParts, Query},
    http::{StatusCode, request::Parts},
};
use chrono::{DateTime, Utc};
use humantime;
use serde::Deserialize;
use std::marker::PhantomData;
use std::time::Duration;

// This struct will be used to deserialize the raw query parameters
#[derive(Debug, Deserialize)]
struct RawInterval {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// A trait to provide a maximum duration for validation.
pub trait WithMaxDuration {
    const MAX_DURATION: Duration;
}

/// Marker for 365 days maximum duration.
pub struct OneYear;
impl WithMaxDuration for OneYear {
    const MAX_DURATION: Duration = Duration::from_secs(60 * 60 * 24 * 365);
}

/// Marker for 1 month (31 days)  maximum duration.
pub struct OneMonth;
impl WithMaxDuration for OneMonth {
    const MAX_DURATION: Duration = Duration::from_secs(60 * 60 * 24 * 31);
}

/// Generic extractor for validated intervals that ensures:
/// 1) `start` is in the past
/// 2) `start` is prior to `end`
/// 3) the difference between `start` and `end` is not greater than the maximum allowed duration provided by the marker that impls [`WithMaxDuration`].
#[derive(Debug, Clone)]
pub struct MaxDurationInterval<T: WithMaxDuration> {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    _marker: PhantomData<T>,
}

impl<S, T> FromRequestParts<S> for MaxDurationInterval<T>
where
    S: Send + Sync,
    T: WithMaxDuration + Send + Sync,
{
    type Rejection = ErrorMessage;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(params) = Query::<RawInterval>::from_request_parts(parts, state)
            .await
            .map_err(|e| ErrorMessage::from((StatusCode::BAD_REQUEST, e.to_string())))?;

        let max_duration = T::MAX_DURATION;
        let now = Utc::now();

        if params.end <= params.start {
            return Err(ErrorMessage::from((
                StatusCode::BAD_REQUEST,
                "end must be greater than start",
            )));
        }

        if params.start > now {
            return Err(ErrorMessage::from((
                StatusCode::BAD_REQUEST,
                "start must be in the past",
            )));
        }

        if (params.end - params.start).num_seconds() > max_duration.as_secs() as i64 {
            let duration_str = format!(
                "end must be {} seconds or less after start ({})",
                max_duration.as_secs(),
                humantime::format_duration(max_duration)
            );
            return Err(ErrorMessage::from((StatusCode::BAD_REQUEST, duration_str)));
        }

        Ok(Self {
            start: params.start,
            end: params.end,
            _marker: PhantomData,
        })
    }
}
