use crate::state::{AppState, Db};
use crate::v1::db::queries::get_latest_datafeed_updated_at;
use crate::v1::error::ApiError;
use axum::{extract::FromRef, extract::FromRequestParts, http::request::Parts};
use chrono::{DateTime, Utc};

pub struct DatafeedMetadata {
    pub requested_at: DateTime<Utc>,
    pub last_datafeed_updated_at: DateTime<Utc>,
}

impl FromRequestParts<AppState> for DatafeedMetadata {
    type Rejection = ApiError;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let db = Db::from_ref(state);
        let now = Utc::now();

        let last_updated = get_latest_datafeed_updated_at(&db.pool).await?;

        let last_datafeed_updated_at = last_updated
            .ok_or_else(|| ApiError::ServiceUnavailable("no datafeeds found".to_owned()))?;

        Ok(Self {
            requested_at: now,
            last_datafeed_updated_at,
        })
    }
}
