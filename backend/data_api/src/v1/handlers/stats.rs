use crate::v1::db::queries;
use crate::v1::db::queries::QueryError;
use crate::v1::extractors::metadata::DatafeedMetadata;
use crate::v1::extractors::params::{MaxDurationInterval, OneMonth, OneYear};
use crate::v1::utils::ErrorMessage;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{Pool, Postgres};
use std::cmp;

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
struct IronMicResponse {
    pub requested_at: DateTime<Utc>,
    pub last_datafeed_updated_at: DateTime<Utc>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub actual_elapsed_duration_seconds: i64,
    pub callsigns: Vec<CallsignDurationStats>,
}

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
struct CallsignDurationStats {
    pub prefix: String,
    pub suffix: String,
    pub duration_seconds: i64,
    pub is_active: Option<bool>,
}

/// On success, returns a [`axum::response::Response`] with [`StatusCode::OK`] and [`IronMicResponse`] as JSON
pub async fn get_iron_mic_stats(
    State(pool): State<Pool<Postgres>>,
    meta: DatafeedMetadata,
    interval: MaxDurationInterval<OneYear>,
) -> Result<impl IntoResponse, ErrorMessage> {
    let stats =
        queries::get_iron_mic_stats(&pool, interval.start, interval.end, meta.requested_at, 50)
            .await;
    match stats {
        Err(e) => match e {
            QueryError::Sql(_) => Err(ErrorMessage::from((StatusCode::INTERNAL_SERVER_ERROR, ""))),
            QueryError::IllegalArgs(msg) => Err(ErrorMessage::from((StatusCode::BAD_REQUEST, msg))),
        },
        Ok(stats) => {
            let uptime_denominator =
                (cmp::min(interval.end, meta.requested_at) - interval.start).num_seconds();
            let durations = stats
                .into_iter()
                .map(|s| CallsignDurationStats {
                    prefix: s.prefix,
                    suffix: s.suffix,
                    duration_seconds: s.duration_seconds,
                    is_active: if meta.requested_at > interval.end {
                        None
                    } else {
                        Some(s.is_active)
                    },
                })
                .collect::<Vec<_>>();

            Ok((
                StatusCode::OK,
                Json(IronMicResponse {
                    requested_at: meta.requested_at,
                    last_datafeed_updated_at: meta.last_datafeed_updated_at,
                    start: interval.start,
                    end: interval.end,
                    actual_elapsed_duration_seconds: uptime_denominator,
                    callsigns: durations,
                }),
            ))
        }
    }
}

#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
struct ActivityTimeSeriesResponse {
    requested_at: DateTime<Utc>,
    last_datafeed_updated_at: DateTime<Utc>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    observations: Vec<DateTime<Utc>>,
    active_controllers: Vec<i32>,
    active_callsigns: Vec<i32>,
    active_positions: Vec<i32>,
}

/// On success, returns a [`axum::response::Response`] with [`StatusCode::OK`] and [`ActivityTimeSeriesResponse`] as JSON
pub async fn get_activity_timeseries(
    State(pool): State<Pool<Postgres>>,
    meta: DatafeedMetadata,
    interval: MaxDurationInterval<OneMonth>,
) -> Result<impl IntoResponse, ErrorMessage> {
    match queries::get_activity_snapshots(&pool, interval.start, interval.end).await {
        Ok(points) => {
            let mut observations = Vec::with_capacity(points.len());
            let mut active_controllers = Vec::with_capacity(points.len());
            let mut active_callsigns = Vec::with_capacity(points.len());
            let mut active_positions = Vec::with_capacity(points.len());

            for p in points {
                observations.push(p.observed_at);
                active_controllers.push(p.active_controllers);
                active_callsigns.push(p.active_callsigns);
                active_positions.push(p.active_positions);
            }

            Ok((
                StatusCode::OK,
                Json(ActivityTimeSeriesResponse {
                    requested_at: meta.requested_at,
                    last_datafeed_updated_at: meta.last_datafeed_updated_at,
                    start: interval.start,
                    end: interval.end,
                    observations,
                    active_controllers,
                    active_callsigns,
                    active_positions,
                }),
            ))
        }
        Err(QueryError::IllegalArgs(msg)) => {
            Err(ErrorMessage::from((StatusCode::BAD_REQUEST, msg)))
        }
        Err(QueryError::Sql(_)) => Err(ErrorMessage::from((StatusCode::INTERNAL_SERVER_ERROR, ""))),
    }
}
