use crate::database::models::{
    ActiveSessionKey, CallsignSession, PositionSession, PositionSessionDetails, QueuedDatafeed,
    UserRating,
};
use chrono::{DateTime, Utc};
use shared::vnas::datafeed::Controller;
use sqlx::{Executor, Postgres};
use std::num::TryFromIntError;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum QueryError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("payload serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("payload compression failed: {0}")]
    Compress(#[from] std::io::Error),
    #[error("payload too large: {0}")]
    PayloadTooLarge(#[from] TryFromIntError),
}

// pub async fn get_all_controller_sessions(
//     pool: &Pool<Postgres>,
// ) -> Result<Vec<ControllerSession>, QueryError> {
//     sqlx::query_as::<_, ControllerSession>("SELECT * FROM controller_sessions")
//         .fetch_all(pool)
//         .await
//         .map_err(QueryError::from)
// }

pub async fn fetch_datafeed_batch<'e, E>(
    executor: E,
    limit: i64,
) -> Result<Vec<QueuedDatafeed>, QueryError>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, QueuedDatafeed>(
        r"
        SELECT id, updated_at, payload, created_at
        FROM datafeed_queue
        ORDER BY updated_at
        FOR UPDATE SKIP LOCKED
        LIMIT $1
        ",
    )
    .bind(limit)
    .fetch_all(executor)
    .await
    .map_err(QueryError::from)
}

/// Returns a tuple `(Uuid, bool)` where the Uuid is the payload primary key in the
/// database and the bool indicates whether a new row was inserted
pub async fn upsert_datafeed_payload<'e, E>(
    executor: &mut E,
    message: &QueuedDatafeed,
) -> Result<(Uuid, bool), QueryError>
where
    for<'c> &'c mut E: Executor<'c, Database = Postgres>,
{
    let payload_bytes = serde_json::to_vec(&message.payload)?;
    let original_size = i32::try_from(payload_bytes.len()).map_err(QueryError::PayloadTooLarge)?;
    let payload_compressed = zstd::encode_all(payload_bytes.as_slice(), 3)?;

    if let Some(id) = sqlx::query_scalar::<_, Uuid>(
        r"
        INSERT INTO datafeed_payloads (
            id,
            updated_at,
            payload_compressed,
            original_size_bytes,
            compression_algo,
            created_at
        )
        VALUES ($1, $2, $3, $4, 'zstd', $5)
        ON CONFLICT (updated_at) DO NOTHING
        RETURNING id
        ",
    )
    .bind(Uuid::now_v7())
    .bind(message.updated_at)
    .bind(payload_compressed)
    .bind(original_size)
    .bind(message.created_at)
    .fetch_optional(&mut *executor)
    .await?
    {
        return Ok((id, true));
    }

    let existing_id =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM datafeed_payloads WHERE updated_at = $1")
            .bind(message.updated_at)
            .fetch_one(&mut *executor)
            .await?;

    Ok((existing_id, false))
}

pub async fn insert_datafeed_message<'e, E>(
    executor: &mut E,
    queue_id: Uuid,
    payload_id: Uuid,
    enqueued_at: DateTime<Utc>,
    processed_at: DateTime<Utc>,
) -> Result<(), QueryError>
where
    for<'c> &'c mut E: Executor<'c, Database = Postgres>,
{
    sqlx::query(
        r"
        INSERT INTO datafeed_messages (id, queue_id, payload_id, enqueued_at, processed_at)
        VALUES ($1, $2, $3, $4, $5)
        ",
    )
    .bind(Uuid::now_v7())
    .bind(queue_id)
    .bind(payload_id)
    .bind(enqueued_at)
    .bind(processed_at)
    .execute(&mut *executor)
    .await
    .map(|_| ())
    .map_err(QueryError::from)
}

pub async fn delete_queued_datafeed<'e, E>(executor: &mut E, id: Uuid) -> Result<(), QueryError>
where
    for<'c> &'c mut E: Executor<'c, Database = Postgres>,
{
    sqlx::query("DELETE FROM datafeed_queue WHERE id = $1")
        .bind(id)
        .execute(&mut *executor)
        .await
        .map(|_| ())
        .map_err(QueryError::from)
}

pub async fn get_active_controller_session_keys<'e, E>(
    executor: E,
) -> Result<Vec<ActiveSessionKey>, QueryError>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, ActiveSessionKey>(
        r"
        SELECT id, cid, login_time, connected_callsign, callsign_session_id, primary_position_id, position_session_id
        FROM controller_sessions
        WHERE is_active = TRUE
        ",
    )
    .fetch_all(executor)
    .await
    .map_err(QueryError::from)
}

pub async fn insert_controller_session<'e, E>(
    executor: E,
    controller: &Controller,
    cid: i32,
    seen_at: DateTime<Utc>,
    callsign_session_id: Uuid,
    position_session_id: Uuid,
) -> Result<Uuid, QueryError>
where
    E: Executor<'e, Database = Postgres>,
{
    let user_rating: UserRating = controller.vatsim_data.user_rating.into();
    let requested_rating: UserRating = controller.vatsim_data.requested_rating.into();
    let id = Uuid::now_v7();

    sqlx::query(
        r"
        INSERT INTO controller_sessions (
            id,
            login_time,
            start_time,
            end_time,
            duration,
            last_seen,
            is_active,
            is_observer,
            cid,
            name,
            user_rating,
            requested_rating,
            connected_callsign,
            primary_position_id,
            callsign_session_id,
            position_session_id
        )
        VALUES (
            $1, $2, $3, NULL, NULL, $4, TRUE, $5, $6, $7, $8, $9, $10, $11, $12, $13
        )
        ",
    )
    .bind(id)
    .bind(controller.login_time)
    .bind(seen_at)
    .bind(seen_at)
    .bind(controller.is_observer)
    .bind(cid)
    .bind(controller.vatsim_data.real_name.clone())
    .bind(user_rating)
    .bind(requested_rating)
    .bind(controller.vatsim_data.callsign.clone())
    .bind(controller.primary_position_id.clone())
    .bind(callsign_session_id)
    .bind(position_session_id)
    .execute(executor)
    .await
    .map_err(QueryError::from)?;

    Ok(id)
}

pub async fn update_active_controller_session<'e, E>(
    executor: E,
    session_id: Uuid,
    controller: &Controller,
    seen_at: DateTime<Utc>,
) -> Result<(), QueryError>
where
    E: Executor<'e, Database = Postgres>,
{
    let user_rating: UserRating = controller.vatsim_data.user_rating.into();
    let requested_rating: UserRating = controller.vatsim_data.requested_rating.into();

    sqlx::query(
        r"
        UPDATE controller_sessions
        SET
            last_seen = $2,
            is_observer = $3,
            name = $4,
            user_rating = $5,
            requested_rating = $6,
            connected_callsign = $7,
            primary_position_id = $8
        WHERE id = $1
        ",
    )
    .bind(session_id)
    .bind(seen_at)
    .bind(controller.is_observer)
    .bind(controller.vatsim_data.real_name.clone())
    .bind(user_rating)
    .bind(requested_rating)
    .bind(controller.vatsim_data.callsign.clone())
    .bind(controller.primary_position_id.clone())
    .execute(executor)
    .await
    .map(|_| ())
    .map_err(QueryError::from)
}

pub async fn complete_controller_sessions<'e, E>(
    executor: E,
    ids: &[Uuid],
    ended_at: DateTime<Utc>,
) -> Result<u64, QueryError>
where
    E: Executor<'e, Database = Postgres>,
{
    let result = sqlx::query(
        r"
        UPDATE controller_sessions
        SET
            is_active = FALSE,
            end_time = $2,
            duration = $2 - start_time,
            last_seen = $2
        WHERE id = ANY($1)
        ",
    )
    .bind(ids)
    .bind(ended_at)
    .execute(executor)
    .await
    .map_err(QueryError::from)?;

    Ok(result.rows_affected())
}

pub async fn get_active_callsign_sessions<'e, E>(
    executor: E,
) -> Result<Vec<CallsignSession>, QueryError>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, CallsignSession>(
        r"
        SELECT id, prefix, suffix, start_time, end_time, duration, last_seen, is_active, created_at
        FROM callsign_sessions
        WHERE is_active = TRUE
        ",
    )
    .fetch_all(executor)
    .await
    .map_err(QueryError::from)
}

pub async fn get_active_position_sessions<'e, E>(
    executor: E,
) -> Result<Vec<PositionSession>, QueryError>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, PositionSession>(
        r"
        SELECT id, position_id, start_time, end_time, duration, last_seen, is_active, created_at
        FROM position_sessions
        WHERE is_active = TRUE
        ",
    )
    .fetch_all(executor)
    .await
    .map_err(QueryError::from)
}

pub async fn fetch_callsign_session_details<'e, E>(
    executor: E,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, String, String)>, QueryError>
where
    E: Executor<'e, Database = Postgres>,
{
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_as::<_, (Uuid, String, String)>(
        r"
        SELECT id, prefix, suffix
        FROM callsign_sessions
        WHERE id = ANY($1)
        ",
    )
    .bind(ids)
    .fetch_all(executor)
    .await
    .map_err(QueryError::from)
}

pub async fn fetch_position_session_details<'e, E>(
    executor: E,
    ids: &[Uuid],
) -> Result<Vec<PositionSessionDetails>, QueryError>
where
    E: Executor<'e, Database = Postgres>,
{
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_as::<_, PositionSessionDetails>(
        r"
        SELECT s.id, s.position_id, p.name AS position_name, p.callsign AS position_callsign
        FROM position_sessions s
        LEFT JOIN facility_positions p ON p.id = s.position_id
        WHERE s.id = ANY($1)
        ",
    )
    .bind(ids)
    .fetch_all(executor)
    .await
    .map_err(QueryError::from)
}

pub async fn get_or_create_callsign_session<E>(
    executor: &mut E,
    prefix: &str,
    suffix: &str,
    seen_at: DateTime<Utc>,
) -> Result<Uuid, QueryError>
where
    for<'c> &'c mut E: Executor<'c, Database = Postgres>,
{
    let existing = sqlx::query_scalar::<_, Uuid>(
        r"
        SELECT id FROM callsign_sessions
        WHERE is_active = TRUE AND prefix = $1 AND suffix = $2
        FOR UPDATE
        ",
    )
    .bind(prefix)
    .bind(suffix)
    .fetch_optional(&mut *executor)
    .await?;

    if let Some(existing) = existing {
        update_callsign_session_last_seen(&mut *executor, existing, seen_at).await?;
        return Ok(existing);
    }

    let id = Uuid::now_v7();
    sqlx::query(
        r"
        INSERT INTO callsign_sessions (
            id,
            prefix,
            suffix,
            start_time,
            end_time,
            duration,
            last_seen,
            is_active,
            created_at
        )
        VALUES ($1, $2, $3, $4, NULL, NULL, $4, TRUE, $4)
        ",
    )
    .bind(id)
    .bind(prefix)
    .bind(suffix)
    .bind(seen_at)
    .execute(&mut *executor)
    .await
    .map_err(QueryError::from)?;

    Ok(id)
}

pub async fn update_callsign_session_last_seen<'e, E>(
    executor: &mut E,
    id: Uuid,
    seen_at: DateTime<Utc>,
) -> Result<(), QueryError>
where
    for<'c> &'c mut E: Executor<'c, Database = Postgres>,
{
    sqlx::query(
        r"
        UPDATE callsign_sessions
        SET last_seen = $2
        WHERE id = $1
        ",
    )
    .bind(id)
    .bind(seen_at)
    .execute(&mut *executor)
    .await
    .map(|_| ())
    .map_err(QueryError::from)
}

pub async fn complete_callsign_sessions<'e, E>(
    executor: E,
    ids: &[Uuid],
    ended_at: DateTime<Utc>,
) -> Result<u64, QueryError>
where
    E: Executor<'e, Database = Postgres>,
{
    let result = sqlx::query(
        r"
        UPDATE callsign_sessions
        SET
            is_active = FALSE,
            end_time = $2,
            duration = $2 - start_time,
            last_seen = $2
        WHERE id = ANY($1)
        ",
    )
    .bind(ids)
    .bind(ended_at)
    .execute(executor)
    .await
    .map_err(QueryError::from)?;

    Ok(result.rows_affected())
}

pub async fn get_or_create_position_session<E>(
    executor: &mut E,
    position_id: &str,
    seen_at: DateTime<Utc>,
) -> Result<Uuid, QueryError>
where
    for<'c> &'c mut E: Executor<'c, Database = Postgres>,
{
    let existing = sqlx::query_scalar::<_, Uuid>(
        r"
        SELECT id FROM position_sessions
        WHERE is_active = TRUE AND position_id = $1
        FOR UPDATE
        ",
    )
    .bind(position_id)
    .fetch_optional(&mut *executor)
    .await?;

    if let Some(existing) = existing {
        update_position_session_last_seen(&mut *executor, existing, seen_at).await?;
        return Ok(existing);
    }

    let id = Uuid::now_v7();
    sqlx::query(
        r"
        INSERT INTO position_sessions (
            id,
            position_id,
            start_time,
            end_time,
            duration,
            last_seen,
            is_active,
            created_at
        )
        VALUES ($1, $2, $3, NULL, NULL, $3, TRUE, $3)
        ",
    )
    .bind(id)
    .bind(position_id)
    .bind(seen_at)
    .execute(&mut *executor)
    .await
    .map_err(QueryError::from)?;

    Ok(id)
}

pub async fn update_position_session_last_seen<'e, E>(
    executor: &mut E,
    id: Uuid,
    seen_at: DateTime<Utc>,
) -> Result<(), QueryError>
where
    for<'c> &'c mut E: Executor<'c, Database = Postgres>,
{
    sqlx::query(
        r"
        UPDATE position_sessions
        SET last_seen = $2
        WHERE id = $1
        ",
    )
    .bind(id)
    .bind(seen_at)
    .execute(&mut *executor)
    .await
    .map(|_| ())
    .map_err(QueryError::from)
}

pub async fn complete_position_sessions<'e, E>(
    executor: E,
    ids: &[Uuid],
    ended_at: DateTime<Utc>,
) -> Result<u64, QueryError>
where
    E: Executor<'e, Database = Postgres>,
{
    let result = sqlx::query(
        r"
        UPDATE position_sessions
        SET
            is_active = FALSE,
            end_time = $2,
            duration = $2 - start_time,
            last_seen = $2
        WHERE id = ANY($1)
        ",
    )
    .bind(ids)
    .bind(ended_at)
    .execute(executor)
    .await
    .map_err(QueryError::from)?;

    Ok(result.rows_affected())
}
