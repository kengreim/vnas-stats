use crate::v1::db::queries;
use crate::v1::db::queries::{QueryError, get_latest_datafeed_updated_at};
use crate::v1::handlers::param_validators::validate_duration_no_longer_than;
use crate::v1::handlers::{ClosedSessionInterval, error_into_response};
use crate::v1::traits::Session;
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{Pool, Postgres};
use std::cmp;
use std::collections::HashMap;
use std::time::Duration;

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
    Query(params): Query<ClosedSessionInterval>,
) -> impl IntoResponse {
    let now = Utc::now();
    let (start, end) = match validate_duration_no_longer_than(
        params.start,
        params.end,
        Duration::from_hours(24 * 365),
    ) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    // Get last updated datafeed and return with errors if we can't unwrap
    let last_datafeed_updated_at = get_latest_datafeed_updated_at(&pool).await;
    let Ok(last_datafeed_updated_at) = last_datafeed_updated_at else {
        return error_into_response(StatusCode::INTERNAL_SERVER_ERROR, "");
    };
    let Some(last_datafeed_updated_at) = last_datafeed_updated_at else {
        return error_into_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "no datafeeds have been fetched yet",
        );
    };

    let callsign_sessions = queries::get_callsign_sessions_between(&pool, start, end).await;
    match callsign_sessions {
        Err(e) => match e {
            QueryError::Sql(_) => error_into_response(StatusCode::INTERNAL_SERVER_ERROR, ""),
            QueryError::IllegalArgs(msg) => error_into_response(StatusCode::BAD_REQUEST, msg),
        },
        Ok(callsign_sessions) => {
            let mut map = HashMap::new();
            for session in callsign_sessions {
                if let Ok(session_duration) = session.duration_seconds_within(start, end, now) {
                    map.entry((session.prefix, session.suffix))
                        .and_modify(|(duration, is_active)| {
                            *duration += session_duration;
                            *is_active = *is_active || session.end_time.is_none();
                        })
                        .or_insert((session_duration, session.end_time.is_none()));
                }
            }

            let uptime_denominator = (cmp::min(end, now) - start).num_seconds();
            let mut durations = map
                .into_iter()
                .map(
                    |((prefix, suffix), (duration_seconds, is_active))| CallsignDurationStats {
                        prefix,
                        suffix,
                        duration_seconds,
                        is_active: if now > end { None } else { Some(is_active) },
                    },
                )
                .collect::<Vec<_>>();
            durations.sort_by_key(|k| i64::MAX - k.duration_seconds);

            (
                StatusCode::OK,
                Json(IronMicResponse {
                    requested_at: now,
                    last_datafeed_updated_at,
                    start,
                    end,
                    actual_elapsed_duration_seconds: uptime_denominator,
                    callsigns: durations,
                }),
            )
                .into_response()
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
    Query(params): Query<ClosedSessionInterval>,
) -> impl IntoResponse {
    let (start, end) = match validate_duration_no_longer_than(
        params.start,
        params.end,
        Duration::from_hours(24 * 31),
    ) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let last_datafeed_updated_at = match get_latest_datafeed_updated_at(&pool).await {
        Ok(Some(ts)) => ts,
        Ok(None) => {
            return error_into_response(StatusCode::SERVICE_UNAVAILABLE, "no datafeeds yet");
        }
        Err(_) => return error_into_response(StatusCode::INTERNAL_SERVER_ERROR, ""),
    };

    match queries::get_activity_snapshots(&pool, start, end).await {
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

            (
                StatusCode::OK,
                Json(ActivityTimeSeriesResponse {
                    requested_at: Utc::now(),
                    last_datafeed_updated_at,
                    start,
                    end,
                    observations,
                    active_controllers,
                    active_callsigns,
                    active_positions,
                }),
            )
                .into_response()
        }
        Err(QueryError::IllegalArgs(msg)) => error_into_response(StatusCode::BAD_REQUEST, msg),
        Err(QueryError::Sql(_)) => error_into_response(StatusCode::INTERNAL_SERVER_ERROR, ""),
    }
}
