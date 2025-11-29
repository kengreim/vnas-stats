#[warn(clippy::pedantic)]
mod error;

use crate::error::{EnqueueError, FetchError, MainError};
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use chrono::{DateTime, TimeDelta, Utc};
use parking_lot::RwLock;
use serde_json::Value;
use shared::error::InitializationError;
use shared::load_config;
use shared::vnas::datafeed::{VnasEnvironment, datafeed_url};
use shared::{PostgresConfig, shutdown_listener};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), MainError> {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber).map_err(InitializationError::Tracing)?;

    // Set up config
    let config = load_config().unwrap_or_else(|e| {
        error!(error = ?e, "configuration could not be initialized");
        panic!("configuration could not be initialized");
    });

    let db_pool = initialize_db(&config.postgres).await?;

    let last_attempted_update = Arc::new(RwLock::new(None));
    let last_successful_update = Arc::new(RwLock::new(None));
    let last_error = Arc::new(RwLock::new(None));

    // Cancellation token shared across tasks; listener cancels on SIGINT/SIGTERM.
    let shutdown_token = CancellationToken::new();
    let signal_handle = tokio::spawn(shutdown_listener(Some(shutdown_token.clone())));

    let axum_handle = tokio::spawn(run_health_server(
        Arc::clone(&last_attempted_update),
        Arc::clone(&last_successful_update),
        Arc::clone(&last_error),
        shutdown_token.clone(),
    ));

    let fetcher_handle = tokio::spawn(fetcher_loop(
        db_pool,
        last_attempted_update,
        last_successful_update,
        last_error,
        shutdown_token.clone(),
    ));

    tokio::select! {
        res = axum_handle => {
            shutdown_token.cancel();
            res??;
        }
        res = fetcher_handle => {
            shutdown_token.cancel();
            res??;
        }
        res = signal_handle => {
            shutdown_token.cancel();
            res?;
        }
    }

    Ok(())
}

async fn fetcher_loop(
    db_pool: Pool<Postgres>,
    last_attempted_update: Arc<RwLock<Option<DateTime<Utc>>>>,
    last_successful_update: Arc<RwLock<Option<DateTime<Utc>>>>,
    last_error: Arc<RwLock<Option<EnqueueError>>>,
    shutdown: CancellationToken,
) -> Result<(), EnqueueError> {
    // Default reqwest client
    let http_client = reqwest::Client::new();

    info!("initialized Datafeed Fetcher");
    let mut initial_loop = true;
    let mut previous_timestamp: Option<DateTime<Utc>> = None;
    loop {
        if initial_loop {
            initial_loop = false;
        } else {
            tokio::select! {
                _ = sleep(Duration::from_secs(15)) => {},
                _ = shutdown.cancelled() => {
                    info!("shutdown requested, exiting fetcher loop");
                    break;
                }
            }
        }

        let now = Utc::now();
        *last_attempted_update.write() = Some(now);
        let (payload, current_timestamp) = match fetch_datafeed(&http_client).await {
            Ok((p, t)) => (p, t),
            Err(e) => {
                warn!(error = ?e, "failed to fetch and deserialize datafeed");
                *last_error.write() = Some(e.into());
                continue;
            }
        };

        // If we found a duplicate, continue the loop which will sleep at the top
        if let Some(previous_timestamp) = previous_timestamp
            && previous_timestamp == current_timestamp
        {
            info!(
                timestamp = ?previous_timestamp,
                "found no change to datafeed"
            );
            *last_successful_update.write() = Some(now);
            continue;
        }

        info!(timestamp = ?current_timestamp, "found updated datafeed");
        previous_timestamp = Some(current_timestamp);

        if let Err(e) = enqueue_datafeed(&db_pool, payload, current_timestamp).await {
            warn!(error = ?e, "could not enqueue datafeed into Postgres");
            *last_error.write() = Some(e);
            continue;
        } else {
            *last_successful_update.write() = Some(now);
            debug!("enqueued datafeed into Postgres queue");
        }

        // If shutdown was requested during processing, break after finishing the iteration.
        if shutdown.is_cancelled() {
            info!("shutdown requested, fetcher loop exiting after current iteration");
            break;
        }
    }

    Ok(())
}

#[derive(Clone)]
struct AxumState {
    last_attempted_update: Arc<RwLock<Option<DateTime<Utc>>>>,
    last_successful_update: Arc<RwLock<Option<DateTime<Utc>>>>,
    last_error: Arc<RwLock<Option<EnqueueError>>>,
}

async fn run_health_server(
    last_attempted_update: Arc<RwLock<Option<DateTime<Utc>>>>,
    last_successful_update: Arc<RwLock<Option<DateTime<Utc>>>>,
    last_error: Arc<RwLock<Option<EnqueueError>>>,
    shutdown: CancellationToken,
) -> Result<(), std::io::Error> {
    info!("starting axum health server");
    let app = Router::new()
        .route("/health", get(health_check))
        .with_state(AxumState {
            last_successful_update,
            last_attempted_update,
            last_error,
        });
    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown.cancelled().await;
        })
        .await?;
    Ok(())
}

async fn health_check(State(state): State<AxumState>) -> impl IntoResponse {
    let last_attempted_update = *state.last_attempted_update.read();
    let last_successful_update = *state.last_successful_update.read();
    let last_error = if let Some(e) = state.last_error.read().as_ref() {
        format!("{e:?}")
    } else {
        "unknown".to_string()
    };

    if last_attempted_update.is_none() || last_successful_update.is_none() {
        if let Some(last_attempted_update) = last_attempted_update {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "Datafeed has not been successfully updated. Last attempted update: {last_attempted_update}. Last error: {last_error}"
                ),
            );
        } else {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "No attempted or successful datafeed updates".to_string(),
            );
        }
    }

    // We can safely unwrap here because we checked is_none above
    let last_attempted_update = last_attempted_update.unwrap();
    let last_successful_update = last_successful_update.unwrap();
    if (Utc::now() - last_successful_update) > TimeDelta::seconds(60) {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Datafeed not fetched in the last 60 seconds. Last successful update: {last_successful_update}. Last attempted updated: {last_attempted_update}. Last error: {last_error}"
            ),
        )
    } else {
        (
            StatusCode::OK,
            format!("Datafeed last successfully fetched: {last_successful_update}"),
        )
    }
}

async fn initialize_db(pg_config: &PostgresConfig) -> Result<Pool<Postgres>, InitializationError> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&pg_config.connection_string)
        .await?;

    // Run any new migrations
    sqlx::migrate!("../migrations").run(&pool).await?;

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
    sqlx::query("SELECT pg_notify('datafeed_queue', $1)")
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}
