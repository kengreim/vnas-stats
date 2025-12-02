use crate::v1::api_models::CallsignSessionsGroupedDto;
use crate::v1::api_models::{
    ActiveSessionsDto, CallsignSessionAggregate, CallsignSessionDetailsDto,
    ControllerSessionDetailsDto, PositionSessionDetailsDto,
};
use crate::v1::db::queries::get_latest_datafeed_updated_at;
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use chrono::Utc;
use serde::Deserialize;
use sqlx::{Pool, Postgres};

#[derive(Deserialize)]
pub struct ControllerSessionQuery {
    pub cid: Option<i32>,
    pub artcc: Option<String>,
}

#[derive(Deserialize)]
pub struct CallsignSessionsQuery {
    pub start_date: chrono::NaiveDate,
    pub end_date: Option<chrono::NaiveDate>,
}

pub async fn get_active_controllers(
    State(pool): State<Pool<Postgres>>,
) -> Result<Json<ActiveSessionsDto<ControllerSessionDetailsDto>>, StatusCode> {
    let requested_at = Utc::now();

    let datafeed_last_updated_at = get_latest_datafeed_updated_at(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let sessions = sqlx::query_as::<_, ControllerSessionDetailsDto>(
        r#"
        SELECT id, cid, connected_callsign, primary_position_id, start_time,
               end_time,
               EXTRACT(EPOCH FROM (COALESCE(end_time, now()) - start_time))::bigint AS duration_seconds,
               EXTRACT(EPOCH FROM (now() - start_time))::bigint AS seconds_since_start_time
        FROM controller_sessions
        WHERE is_active = TRUE
        "#,
    )
        .fetch_all(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ActiveSessionsDto {
        requested_at,
        datafeed_last_updated_at,
        sessions,
    }))
}

pub async fn get_active_callsigns(
    State(pool): State<Pool<Postgres>>,
) -> Result<Json<ActiveSessionsDto<CallsignSessionDetailsDto>>, StatusCode> {
    let requested_at = Utc::now();

    let datafeed_last_updated_at = get_latest_datafeed_updated_at(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let sessions = sqlx::query_as::<_, CallsignSessionDetailsDto>(
        r"
        SELECT id, prefix, suffix, start_time,
               EXTRACT(EPOCH FROM (now() - start_time))::bigint AS seconds_since_start_time
        FROM callsign_sessions
        WHERE is_active = TRUE
        ",
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ActiveSessionsDto {
        requested_at,
        datafeed_last_updated_at,
        sessions,
    }))
}

pub async fn get_active_positions(
    State(pool): State<Pool<Postgres>>,
) -> Result<Json<ActiveSessionsDto<PositionSessionDetailsDto>>, StatusCode> {
    let requested_at = Utc::now();

    let datafeed_last_updated_at = get_latest_datafeed_updated_at(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let sessions = sqlx::query_as::<_, PositionSessionDetailsDto>(
        r"
        SELECT id, position_id, start_time,
               EXTRACT(EPOCH FROM (now() - start_time))::bigint AS seconds_since_start_time
        FROM position_sessions
        WHERE is_active = TRUE
        ",
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ActiveSessionsDto {
        requested_at,
        datafeed_last_updated_at,
        sessions,
    }))
}

pub async fn get_controller_sessions(
    State(pool): State<Pool<Postgres>>,
    Query(params): Query<ControllerSessionQuery>,
) -> Result<Json<ActiveSessionsDto<ControllerSessionDetailsDto>>, StatusCode> {
    let requested_at = Utc::now();

    let datafeed_last_updated_at = get_latest_datafeed_updated_at(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let sessions = sqlx::query_as::<_, ControllerSessionDetailsDto>(
        r"
        SELECT cs.id,
               cs.cid,
               cs.connected_callsign,
               cs.primary_position_id,
               cs.start_time,
               cs.end_time,
               EXTRACT(EPOCH FROM (COALESCE(cs.end_time, now()) - cs.start_time))::bigint AS duration_seconds,
               EXTRACT(EPOCH FROM (now() - cs.start_time))::bigint AS seconds_since_start_time
        FROM controller_sessions cs
        LEFT JOIN facility_positions fp ON fp.id = cs.primary_position_id
        LEFT JOIN facilities f ON f.id = fp.facility_id
        LEFT JOIN facilities artcc_root ON artcc_root.id = f.artcc_root_facility_id
        WHERE ($1::int IS NULL OR cs.cid = $1)
          AND ($2::text IS NULL OR artcc_root.id = $2)
        ",
    )
        .bind(params.cid)
        .bind(params.artcc.clone())
        .fetch_all(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ActiveSessionsDto {
        requested_at,
        datafeed_last_updated_at,
        sessions,
    }))
}

pub async fn get_callsign_sessions(
    State(pool): State<Pool<Postgres>>,
    Query(params): Query<CallsignSessionsQuery>,
) -> Result<Json<CallsignSessionsGroupedDto>, StatusCode> {
    let requested_at = Utc::now();

    let start = params.start_date.and_hms_opt(0, 0, 0).unwrap();
    let mut end = params
        .end_date
        .unwrap_or(params.start_date + chrono::Duration::days(365))
        .and_hms_opt(23, 59, 59)
        .unwrap();
    // Enforce max 1-year window
    if end - start > chrono::Duration::days(365) {
        end = start + chrono::Duration::days(365);
    }

    let datafeed_last_updated_at = get_latest_datafeed_updated_at(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let aggregates = sqlx::query_as::<_, CallsignSessionAggregate>(
        r#"
        SELECT prefix, suffix,
               SUM(
                   CASE
                       WHEN end_time IS NULL THEN EXTRACT(EPOCH FROM (LEAST(now(), $2) - GREATEST(start_time, $1)))
                       ELSE EXTRACT(EPOCH FROM (LEAST(end_time, $2) - GREATEST(start_time, $1)))
                   END
               )::bigint AS total_duration_seconds
        FROM callsign_sessions
        WHERE (end_time IS NULL AND start_time <= $2)
           OR (end_time IS NOT NULL AND end_time >= $1)
        GROUP BY prefix, suffix
        "#,
    )
    .bind(start)
    .bind(end)
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(CallsignSessionsGroupedDto {
        requested_at,
        datafeed_last_updated_at,
        sessions: aggregates,
    }))
}
