use crate::v1::db::queries::get_latest_datafeed_updated_at;
use crate::v1::utils::ErrorMessage;
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::Response,
};
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres};

pub struct DatafeedMetadata {
    pub requested_at: DateTime<Utc>,
    pub last_datafeed_updated_at: DateTime<Utc>,
}

impl FromRequestParts<Pool<Postgres>> for DatafeedMetadata {
    type Rejection = ErrorMessage;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &Pool<Postgres>,
    ) -> Result<Self, Self::Rejection> {
        let now = Utc::now();

        let last_updated = get_latest_datafeed_updated_at(state).await.map_err(|_| {
            ErrorMessage::from((StatusCode::INTERNAL_SERVER_ERROR, "database error"))
        })?;

        let last_datafeed_updated_at = last_updated.ok_or_else(|| {
            ErrorMessage::from((
                StatusCode::SERVICE_UNAVAILABLE,
                "no datafeeds available yet",
            ))
        })?;

        Ok(Self {
            requested_at: now,
            last_datafeed_updated_at,
        })
    }
}
