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
use shared::shutdown_listener;
use shared::vnas::datafeed::{VnasEnvironment, datafeed_url};
use shared::{initialize_db, load_config};
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

    let interval_seconds = if let Some(fetcher_config) = config.fetcher {
        fetcher_config.interval_seconds
    } else {
        15
    };

    let last_attempted_update = Arc::new(RwLock::new(None));
    let last_successful_update = Arc::new(RwLock::new(None));
    let last_error = Arc::new(RwLock::new(None));

    // Cancellation token shared across tasks; listener cancels on SIGINT/SIGTERM.
    let shutdown_token = CancellationToken::new();
    let mut signal_handle = tokio::spawn(shutdown_listener(Some(shutdown_token.clone())));

    let mut axum_handle = tokio::spawn(run_health_server(
        Arc::clone(&last_attempted_update),
        Arc::clone(&last_successful_update),
        Arc::clone(&last_error),
        shutdown_token.clone(),
    ));

    let mut fetcher_handle = tokio::spawn(fetcher_loop(
        db_pool,
        interval_seconds,
        last_attempted_update,
        last_successful_update,
        last_error,
        shutdown_token.clone(),
    ));

    let mut first_err: Option<MainError> = None;
    let mut axum_done = false;
    let mut fetcher_done = false;

    tokio::select! {
        res = &mut axum_handle => {
            info!("axum task completed first, propagating cancellation token to other tasks");
            axum_done = true;
            shutdown_token.cancel();
            match res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    warn!(error = ?e, "axum task completed due to error");
                    first_err.get_or_insert(e.into());
                }
                Err(join) => {
                    warn!(error = ?join, "axum task completed due to error");
                    first_err.get_or_insert(join.into());
                }
            }
        }
        res = &mut fetcher_handle => {
            info!("fetcher task completed first, propagating cancellation token to other tasks");
            fetcher_done = true;
            shutdown_token.cancel();
            match res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    warn!(error = ?e, "fetcher task completed due to error");
                    first_err.get_or_insert(e.into());
                }
                Err(join) => {
                    warn!(error = ?join, "fetcher task completed due to error");
                    first_err.get_or_insert(join.into());
                }
            }
        }
        res = &mut signal_handle => {
            info!("SIGINT/SIGTERM listener task completed first, propagating cancellation token to other tasks");
            shutdown_token.cancel();
            if let Err(join) = res {
                warn!(error = ?join, "error with SIGINT/SIGTERM listener task");
                first_err.get_or_insert(join.into());
            }
        }
    }

    if !axum_done {
        info!("awaiting completion of axum task");
        match axum_handle.await {
            Ok(Ok(())) => {
                info!("axum task completed successfully");
            }
            Ok(Err(e)) => {
                info!(error = ?e, "axum task completed with error");
                first_err.get_or_insert(e.into());
            }
            Err(join) => {
                info!(error = ?join, "axum task completed with error");
                first_err.get_or_insert(join.into());
            }
        }
    }
    if !fetcher_done {
        info!("awaiting completion of fetcher task");
        match fetcher_handle.await {
            Ok(Ok(())) => {
                info!("fetcher task completed successfully");
            }
            Ok(Err(e)) => {
                info!(error = ?e, "fetcher task completed with error");
                first_err.get_or_insert(e.into());
            }
            Err(join) => {
                info!(error = ?join, "fetcher task completed with error");
                first_err.get_or_insert(join.into());
            }
        }
    }

    if let Some(err) = first_err {
        Err(err)
    } else {
        Ok(())
    }
}

async fn fetcher_loop(
    db_pool: Pool<Postgres>,
    interval_seconds: u64,
    last_attempted_update: Arc<RwLock<Option<DateTime<Utc>>>>,
    last_successful_update: Arc<RwLock<Option<DateTime<Utc>>>>,
    last_error: Arc<RwLock<Option<EnqueueError>>>,
    shutdown: CancellationToken,
) -> Result<(), EnqueueError> {
    // Default reqwest client
    let http_client = reqwest::Client::new();

    info!("initialized Datafeed Fetcher");
    let mut initial_loop = true;
    loop {
        if initial_loop {
            initial_loop = false;
        } else {
            tokio::select! {
                _ = sleep(Duration::from_secs(interval_seconds)) => {},
                _ = shutdown.cancelled() => {
                    info!("shutdown requested, exiting fetcher loop");
                    break;
                }
            }
        }

        let now = Utc::now();
        *last_attempted_update.write() = Some(now);
        let (payload, datafeed_updated_at) = match fetch_datafeed(&http_client).await {
            Ok((p, t)) => (p, t),
            Err(e) => {
                warn!(error = ?e, "failed to fetch and deserialize datafeed");
                *last_error.write() = Some(e.into());
                continue;
            }
        };
        info!(updated_at = ?datafeed_updated_at, "fetched datafeed");

        if let Err(e) = enqueue_datafeed(&db_pool, payload, datafeed_updated_at).await {
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
        .with_graceful_shutdown(shutdown.cancelled_owned())
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
        return if let Some(last_attempted_update) = last_attempted_update {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "Datafeed has not been successfully updated. Last attempted update: {last_attempted_update}. Last error: {last_error}"
                ),
            )
        } else {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "No attempted or successful datafeed updates".to_string(),
            )
        };
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
