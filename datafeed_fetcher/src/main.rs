mod error;

use crate::error::{EnqueueError, FetchError};
use chrono::{DateTime, Utc};
use serde_json::Value;
use shared::PostgresConfig;
use shared::error::InitializationError;
use shared::load_config;
use shared::vnas::datafeed::{VnasEnvironment, datafeed_url};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), InitializationError> {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .json()
        .with_file(true)
        .with_line_number(true)
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    // Set up config
    let config = load_config().unwrap_or_else(|e| {
        error!(error = ?e, "Configuration could not be initialized");
        panic!("Configuration could not be initialized");
    });

    let db_pool = initialize_db(&config.postgres).await?;

    // Default reqwest client
    let http_client = reqwest::Client::new();

    // Datafetcher infinite loop
    info!("Initialized Datafeed Fetcher");
    let mut initial_loop = true;
    let mut previous_timestamp: Option<DateTime<Utc>> = None;
    loop {
        if initial_loop {
            initial_loop = false;
        } else {
            sleep(Duration::from_secs(15)).await;
        }

        let (payload, current_timestamp) = match fetch_datafeed(&http_client).await {
            Ok((p, t)) => (p, t),
            Err(e) => {
                warn!(error = ?e, "Failed to fetch and deserialize datafeed");
                continue;
            }
        };

        // If we found a duplicate, continue the loop which will sleep at the top
        if let Some(previous_timestamp) = previous_timestamp
            && previous_timestamp == current_timestamp
        {
            info!(
                timestamp = ?previous_timestamp,
                "Found no change to datafeed"
            );
            continue;
        }

        info!(timestamp = ?current_timestamp, "Found updated datafeed");
        previous_timestamp = Some(current_timestamp);

        if let Err(e) = enqueue_datafeed(&db_pool, payload, current_timestamp).await {
            warn!(error = ?e, "Could not enqueue datafeed into Postgres");
            continue;
        } else {
            debug!("Enqueued datafeed into Postgres queue");
        }
    }
}

async fn initialize_db(pg_config: &PostgresConfig) -> Result<Pool<Postgres>, InitializationError> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&pg_config.connection_string)
        .await?;

    // Run any new migrations
    sqlx::migrate!("../datafeed_processor/migrations")
        .run(&pool)
        .await?;

    Ok(pool)
}

async fn fetch_datafeed(client: &reqwest::Client) -> Result<(Value, DateTime<Utc>), FetchError> {
    let resp = client
        .get(datafeed_url(VnasEnvironment::Live))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let value: Value = serde_json::from_str(&resp)?;
    if let Some(timestamp_val) = value.get("updatedAt")
        && let Some(timestamp_str) = timestamp_val.as_str()
    {
        let timestamp = DateTime::parse_from_rfc3339(timestamp_str)?;
        Ok((value, timestamp.with_timezone(&Utc)))
    } else {
        Err(FetchError::MissingUpdatedAt)
    }
}

async fn enqueue_datafeed(
    pool: &Pool<Postgres>,
    payload: Value,
    updated_at: DateTime<Utc>,
) -> Result<(), EnqueueError> {
    let id = Uuid::now_v7();
    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO datafeed_queue (id, updated_at, payload)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(id)
    .bind(updated_at)
    .bind(payload)
    .execute(&mut *tx)
    .await?;

    // Notify listeners that a new datafeed is available.
    sqlx::query_scalar::<_, String>("SELECT pg_notify('datafeed_queue', $1)")
        .bind(id.to_string())
        .fetch_one(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}
