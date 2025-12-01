use axum::extract::Query;
use axum::http::StatusCode;
use axum::{Json, Router, extract::State, routing::get};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::Serialize;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

pub fn router(pool: Pool<Postgres>) -> Router {
    Router::new()
        .route("/controllers/active", get(get_active_controllers))
        .route("/controllers", get(get_controller_sessions))
        .route("/callsigns/active", get(get_active_callsigns))
        .route("/positions/active", get(get_active_positions))
        .with_state(pool)
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSessionsDto<T> {
    pub requested_at: DateTime<Utc>,
    pub datafeed_last_updated_at: Option<DateTime<Utc>>,
    pub sessions: Vec<T>,
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ControllerSessionDetailsDto {
    pub id: Uuid,
    pub cid: i32,
    pub connected_callsign: String,
    pub primary_position_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i64>,
    pub seconds_since_start_time: i64,
}

#[derive(Deserialize)]
pub struct ControllerSessionQuery {
    pub cid: Option<i32>,
    pub artcc: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CallsignSessionDetailsDto {
    pub id: Uuid,
    pub prefix: String,
    pub suffix: String,
    pub start_time: DateTime<Utc>,
    pub seconds_since_start_time: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PositionSessionDetailsDto {
    pub id: Uuid,
    pub position_id: String,
    pub start_time: DateTime<Utc>,
    pub seconds_since_start_time: i64,
}

async fn get_active_controllers(
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

async fn get_active_callsigns(
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

async fn get_active_positions(
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

async fn get_controller_sessions(
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

async fn get_latest_datafeed_updated_at(
    pool: &Pool<Postgres>,
) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    sqlx::query_scalar::<_, DateTime<Utc>>(
        r"
        SELECT max(updated_at)
        FROM datafeed_payloads
        ",
    )
    .fetch_optional(pool)
    .await
}
