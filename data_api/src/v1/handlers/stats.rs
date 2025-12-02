use crate::v1::db::queries;
use crate::v1::db::queries::QueryError;
use crate::v1::db_helpers::get_latest_datafeed_updated_at;
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

#[derive(Serialize)]
struct IronMicResponse {
    pub requested_at: DateTime<Utc>,
    pub last_datafeed_updated_at: DateTime<Utc>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub callsigns: Vec<CallsignDurationStats>,
}

#[derive(Serialize)]
struct CallsignDurationStats {
    pub prefix: String,
    pub suffix: String,
    pub duration_seconds: i64,
    pub uptime_percent: f64,
}

pub async fn get_iron_mic_stats(
    State(pool): State<Pool<Postgres>>,
    Query(params): Query<ClosedSessionInterval>,
) -> impl IntoResponse {
    let now = Utc::now();

    if params.end <= params.start {
        return error_into_response(StatusCode::BAD_REQUEST, "end must be greater than start");
    }

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

    let callsign_sessions =
        queries::get_callsign_sessions_between(&pool, params.start, params.end).await;
    match callsign_sessions {
        Err(e) => match e {
            QueryError::Sql(_) => error_into_response(StatusCode::INTERNAL_SERVER_ERROR, ""),
            QueryError::IllegalArgs(msg) => error_into_response(StatusCode::BAD_REQUEST, msg),
        },
        Ok(callsign_sessions) => {
            let mut map = HashMap::new();
            for session in callsign_sessions {
                if let Ok(session_duration) =
                    session.duration_seconds_within(params.start, params.end, now)
                {
                    map.entry((session.prefix, session.suffix))
                        .and_modify(|e| *e += session_duration)
                        .or_insert(session_duration);
                }
            }

            let uptime_denominator = (cmp::min(params.end, now) - params.start).num_seconds();
            let mut durations = map
                .into_iter()
                .map(
                    |((prefix, suffix), duration_seconds)| CallsignDurationStats {
                        prefix,
                        suffix,
                        duration_seconds,
                        uptime_percent: duration_seconds as f64 / uptime_denominator as f64,
                    },
                )
                .collect::<Vec<_>>();
            durations.sort_by_key(|k| i64::MAX - k.duration_seconds);

            (
                StatusCode::OK,
                Json(IronMicResponse {
                    requested_at: now,
                    last_datafeed_updated_at,
                    start: params.start,
                    end: params.end,
                    callsigns: durations,
                }),
            )
                .into_response()
        }
    }
}
