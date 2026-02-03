use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres};

/// Callsigns to exclude from Iron Mic stats.
/// Format: (prefix, suffix) - e.g., ("SJU", "APP") for SJU_APP
const IRON_MIC_EXCLUDED_CALLSIGNS: &[(&str, &str)] = &[
    ("SJU", "APP"),
    // Add more exclusions here as needed:
    // ("ABC", "CTR"),
];

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error("illegal args for query: {0}")]
    IllegalArgs(String),
}

#[derive(sqlx::FromRow)]
pub struct CallsignDurationStatsRecord {
    pub prefix: String,
    pub suffix: String,
    pub duration_seconds: i64,
    pub is_active: bool,
}

/// Builds a SQL exclusion clause for callsigns, or empty string if no exclusions.
fn build_callsign_exclusion_clause() -> String {
    if IRON_MIC_EXCLUDED_CALLSIGNS.is_empty() {
        return String::new();
    }

    let conditions: Vec<String> = IRON_MIC_EXCLUDED_CALLSIGNS
        .iter()
        .map(|(prefix, suffix)| format!("(prefix = '{prefix}' AND suffix = '{suffix}')"))
        .collect();

    format!(" AND NOT ({})", conditions.join(" OR "))
}

pub async fn get_iron_mic_stats(
    pool: &Pool<Postgres>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    now: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<CallsignDurationStatsRecord>, QueryError> {
    if end <= start {
        return Err(QueryError::IllegalArgs(format!(
            "end must be greater than start. end = {end:?}. start = {start:?}"
        )));
    }

    let exclusion_clause = build_callsign_exclusion_clause();
    let query = format!(
        r"
        SELECT
            prefix,
            suffix,
            SUM(
                EXTRACT(EPOCH FROM (
                    LEAST(COALESCE(end_time, $3), $2) - GREATEST(start_time, $1)
                ))
            )::BIGINT AS duration_seconds,
            BOOL_OR(end_time IS NULL) AS is_active
        FROM callsign_sessions
        WHERE start_time < $2
          AND (end_time IS NULL OR end_time > $1){exclusion_clause}
        GROUP BY prefix, suffix
        ORDER BY duration_seconds DESC
        LIMIT $4
        "
    );

    sqlx::query_as::<_, CallsignDurationStatsRecord>(&query)
        .bind(start)
        .bind(end)
        .bind(now)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(QueryError::Sql)
}

pub async fn get_latest_datafeed_updated_at(
    pool: &Pool<Postgres>,
) -> Result<Option<DateTime<Utc>>, QueryError> {
    sqlx::query_scalar::<_, DateTime<Utc>>(
        r"
        SELECT max(updated_at)
        FROM datafeed_payloads
        ",
    )
    .fetch_optional(pool)
    .await
    .map_err(QueryError::Sql)
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
        return Err(QueryError::IllegalArgs(format!(
            "end must be greater than start. end = {end:?}. start = {start:?}"
        )));
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
