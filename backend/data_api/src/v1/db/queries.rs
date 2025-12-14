use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error("illegal args for query: {0}")]
    IllegalArgs(String),
}

#[derive(sqlx::FromRow)]
pub struct CallsignSessionRecord {
    pub id: Uuid,
    pub prefix: String,
    pub suffix: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
}

impl crate::v1::traits::Session for CallsignSessionRecord {
    fn start_time(&self) -> DateTime<Utc> {
        self.start_time
    }

    fn end_time(&self) -> Option<DateTime<Utc>> {
        self.end_time
    }
}

/// Fetch callsign sessions that overlap the provided time window.
pub async fn get_callsign_sessions_between(
    pool: &Pool<Postgres>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<CallsignSessionRecord>, QueryError> {
    if end <= start {
        return Err(QueryError::IllegalArgs(
            "end must be greater than start".to_owned(),
        ));
    }

    sqlx::query_as::<_, CallsignSessionRecord>(
        r"
        SELECT id, prefix, suffix, start_time, end_time
        FROM callsign_sessions
        WHERE start_time < $2
          AND (end_time IS NULL OR end_time > $1)
        ORDER BY start_time
        ",
    )
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
    .map_err(QueryError::Sql)
}

pub async fn get_latest_datafeed_updated_at(
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

#[derive(sqlx::FromRow)]
pub struct ActivitySnapshot {
    pub observed_at: DateTime<Utc>,
    pub active_controllers: i32,
    pub active_callsigns: i32,
    pub active_positions: i32,
}

/// Return activity snapshots between start/end, collapsing consecutive duplicates across any of the three counts.
pub async fn get_activity_snapshots(
    pool: &Pool<Postgres>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<ActivitySnapshot>, QueryError> {
    if end <= start {
        return Err(QueryError::IllegalArgs(
            "end must be greater than start".to_owned(),
        ));
    }

    sqlx::query_as::<_, ActivitySnapshot>(
        r#"
        SELECT observed_at, active_controllers, active_callsigns, active_positions
        FROM (
            SELECT
                observed_at,
                active_controllers,
                active_callsigns,
                active_positions,
                LAG(active_controllers) OVER (ORDER BY observed_at) AS prev_c,
                LAG(active_callsigns) OVER (ORDER BY observed_at) AS prev_cs,
                LAG(active_positions) OVER (ORDER BY observed_at) AS prev_p
            FROM session_activity_stats
            WHERE observed_at >= $1 AND observed_at <= $2
            ORDER BY observed_at
        ) s
        WHERE prev_c IS NULL
           OR active_controllers <> prev_c
           OR active_callsigns <> prev_cs
           OR active_positions <> prev_p
        ORDER BY observed_at
        "#,
    )
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
    .map_err(QueryError::Sql)
}
