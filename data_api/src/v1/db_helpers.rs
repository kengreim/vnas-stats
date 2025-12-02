use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres};

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
