#![warn(clippy::pedantic)]
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
use shared::vnas::datafeed::{VnasEnvironment, datafeed_url};
use shared::{init_tracing_and_oltp, shutdown_listener};
use shared::{initialize_db, load_config};
use sqlx::{Pool, Postgres};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

#[derive(Clone)]
struct FetcherState {
    db_pool: Pool<Postgres>,
    last_attempted_update: Arc<RwLock<Option<DateTime<Utc>>>>,
    last_successful_update: Arc<RwLock<Option<DateTime<Utc>>>>,
    last_error: Arc<RwLock<Option<EnqueueError>>>,
    in_memory_queue: Arc<RwLock<VecDeque<(Value, DateTime<Utc>)>>>,
}

#[tokio::main]
async fn main() -> Result<(), MainError> {
    let (tracer_provider, meter_provider) = init_tracing_and_oltp("artcc_updater")?;

    // Set up config
    let config = load_config().unwrap_or_else(|e| {
        error!(error = ?e, "configuration could not be initialized");
        panic!("configuration could not be initialized");
    });
    info!(name: "config.loaded", config = ?config, "config loaded");

    let db_pool = initialize_db(&config.postgres, true).await?;

    let interval_seconds = config.fetcher.map_or(15, |c| c.interval_seconds);

    let state = FetcherState {
        db_pool,
        last_attempted_update: Arc::new(RwLock::new(None)),
        last_successful_update: Arc::new(RwLock::new(None)),
        last_error: Arc::new(RwLock::new(None)),
        in_memory_queue: Arc::new(RwLock::new(VecDeque::new())),
    };

    // Cancellation token shared across tasks; listener cancels on SIGINT/SIGTERM.
    let shutdown_token = CancellationToken::new();
    let mut signal_handle = tokio::spawn(shutdown_listener(Some(shutdown_token.clone())));

    let mut axum_handle = tokio::spawn(run_health_server(state.clone(), shutdown_token.clone()));

    let mut fetcher_handle = tokio::spawn(fetcher_loop(
        state,
        interval_seconds,
        shutdown_token.clone(),
    ));

    let mut first_err: Option<MainError> = None;
    let mut axum_done = false;
    let mut fetcher_done = false;

    tokio::select! {
        res = &mut axum_handle => {
            info!(name: "axum.completed", "axum task completed first, propagating cancellation token to other tasks");
            axum_done = true;
            shutdown_token.cancel();
            match res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    warn!(name: "axum.completed", error = ?e, "axum task completed due to error");
                    first_err.get_or_insert(e.into());
                }
                Err(join) => {
                    warn!(name: "axum.completed", error = ?join, "axum task completed due to error");
                    first_err.get_or_insert(join.into());
                }
            }
        }
        res = &mut fetcher_handle => {
            info!(name: "fetcher.completed", "fetcher task completed first, propagating cancellation token to other tasks");
            fetcher_done = true;
            shutdown_token.cancel();
            match res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    warn!(name: "fetcher.completed", error = ?e, "fetcher task completed due to error");
                    first_err.get_or_insert(e.into());
                }
                Err(join) => {
                    warn!(name: "fetcher.completed", error = ?join, "fetcher task completed due to error");
                    first_err.get_or_insert(join.into());
                }
            }
        }
        res = &mut signal_handle => {
            info!(name: "listener.completed", "SIGINT/SIGTERM listener task completed first, propagating cancellation token to other tasks");
            shutdown_token.cancel();
            if let Err(join) = res {
                warn!(name: "listener.completed", error = ?join, "error with SIGINT/SIGTERM listener task");
                first_err.get_or_insert(join.into());
            }
        }
    }

    if !axum_done {
        info!(name:"axum.completion.awaiting", "awaiting completion of axum task");
        match axum_handle.await {
            Ok(Ok(())) => {
                info!(name: "axum.completed", "axum task completed successfully");
            }
            Ok(Err(e)) => {
                info!(name: "axum.completed", error = ?e, "axum task completed with error");
                first_err.get_or_insert(e.into());
            }
            Err(join) => {
                info!(name: "axum.completed", error = ?join, "axum task completed with error");
                first_err.get_or_insert(join.into());
            }
        }
    }
    if !fetcher_done {
        info!(name: "fetcher.completion.awaiting", "awaiting completion of fetcher task");
        match fetcher_handle.await {
            Ok(Ok(())) => {
                info!(name: "fetcher.completed", "fetcher task completed successfully");
            }
            Ok(Err(e)) => {
                info!(name: "fetcher.completed", error = ?e, "fetcher task completed with error");
                first_err.get_or_insert(e.into());
            }
            Err(join) => {
                info!(name: "fetcher.completed", error = ?join, "fetcher task completed with error");
                first_err.get_or_insert(join.into());
            }
        }
    }

    if let Err(e) = tracer_provider.shutdown() {
        eprintln!("failed to shut down tracer provider: {e:?}");
    }

    if let Err(e) = meter_provider.shutdown() {
        eprintln!("failed to shut down tracer provider: {e:?}");
    }

    if let Some(err) = first_err {
        Err(err)
    } else {
        Ok(())
    }
}

async fn fetcher_loop(
    state: FetcherState,
    interval_seconds: u64,
    shutdown: CancellationToken,
) -> Result<(), EnqueueError> {
    // Default reqwest client
    let http_client = reqwest::Client::new();

    info!(name: "fetcher.loop.initialized", "initialized Datafeed Fetcher");
    let mut initial_loop = true;
    loop {
        if initial_loop {
            initial_loop = false;
        } else {
            tokio::select! {
                _ = sleep(Duration::from_secs(interval_seconds)) => {},
                _ = shutdown.cancelled() => {
                    info!(name: "fetcher_loop.shutdown.requested", "shutdown requested, exiting fetcher loop");
                    break;
                }
            }
        }

        // Try to process in-memory queue first
        process_in_memory_queue(&state).await;

        let now = Utc::now();
        *state.last_attempted_update.write() = Some(now);
        let (payload, datafeed_updated_at) = match fetch_datafeed(&http_client).await {
            Ok((p, t)) => (p, t),
            Err(e) => {
                warn!(name:"fetcher_loop.datafeed.received", error = ?e, "failed to fetch and deserialize datafeed");
                *state.last_error.write() = Some(e.into());
                continue;
            }
        };
        info!(updated_at = ?datafeed_updated_at, "fetched datafeed");

        if let Err(e) = enqueue_datafeed(&state.db_pool, payload.clone(), datafeed_updated_at).await
        {
            warn!(name:"fetcher_loop.datafeed.enqueued", error = ?e, "could not enqueue datafeed into Postgres");
            *state.last_error.write() = Some(e);
            state
                .in_memory_queue
                .write()
                .push_back((payload, datafeed_updated_at));
            info!(
                name = "fetcher_loop.in_memory_queue.item_added",
                "added item to in-memory queue"
            );
        } else {
            *state.last_successful_update.write() = Some(now);
            debug!(name:"fetcher_loop.datafeed.enqueued", "enqueued datafeed into Postgres queue");
        }

        // If shutdown was requested during processing, break after finishing the iteration.
        if shutdown.is_cancelled() {
            info!(name: "fetcher_loop.shutdown.requested", "shutdown requested, fetcher loop exiting after current iteration");
            break;
        }
    }

    Ok(())
}

async fn process_in_memory_queue(state: &FetcherState) {
    let queue_len = state.in_memory_queue.read().len();
    if queue_len == 0 {
        return;
    }

    clear_in_memory_queue(state).await;
}

#[instrument(skip(state))]
async fn clear_in_memory_queue(state: &FetcherState) {
    let queue_len = state.in_memory_queue.read().len();
    info!(
        name = "fetcher_loop.in_memory_queue.processing.started",
        count = queue_len,
        "processing in-memory queue"
    );

    loop {
        let item = state.in_memory_queue.write().pop_front();

        if let Some((payload, datafeed_updated_at)) = item {
            match enqueue_datafeed(&state.db_pool, payload.clone(), datafeed_updated_at).await {
                Ok(()) => {
                    info!(
                        name = "fetcher_loop.in_memory_queue.processing.item",
                        "processed item from in-memory queue"
                    );
                }
                Err(e) => {
                    warn!(
                        name = "fetcher_loop.in_memory_queue.item",
                        error = ?e,
                        "failed to process item from in-memory queue, will retry later"
                    );
                    state
                        .in_memory_queue
                        .write()
                        .push_front((payload, datafeed_updated_at));
                    break;
                }
            }
        } else {
            break;
        }
    }
    let queue_len_after = state.in_memory_queue.read().len();
    if queue_len_after > 0 {
        info!(
            name = "fetcher_loop.in_memory_queue.processing.paused",
            count = queue_len_after,
            "paused processing in-memory queue"
        );
    } else {
        info!(
            name = "fetcher_loop.in_memory_queue.processing.ended",
            "finished processing in-memory queue"
        );
    }
}

async fn run_health_server(
    state: FetcherState,
    shutdown: CancellationToken,
) -> Result<(), std::io::Error> {
    info!(name: "axum.initialized", "starting axum health server");
    let app = Router::new()
        .route("/health", get(health_check))
        .with_state(state);
    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await?;
    Ok(())
}

async fn health_check(State(state): State<FetcherState>) -> impl IntoResponse {
    let last_attempted_update = *state.last_attempted_update.read();
    let last_successful_update = *state.last_successful_update.read();
    let last_error = if let Some(e) = state.last_error.read().as_ref() {
        format!("{e:?}")
    } else {
        "unknown".to_string()
    };
    let in_memory_queue_len = state.in_memory_queue.read().len();

    if last_attempted_update.is_none() || last_successful_update.is_none() {
        return if let Some(last_attempted_update) = last_attempted_update {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "Datafeed has not been successfully updated. Last attempted update: {last_attempted_update}. Last error: {last_error}. In-memory queue length: {in_memory_queue_len}"
                ),
            )
        } else {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "No attempted or successful datafeed updates. In-memory queue length: {in_memory_queue_len}"
                ),
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
                "Datafeed not fetched in the last 60 seconds. Last successful update: {last_successful_update}. Last attempted updated: {last_attempted_update}. Last error: {last_error}. In-memory queue length: {in_memory_queue_len}"
            ),
        )
    } else {
        (
            StatusCode::OK,
            format!(
                "Datafeed last successfully fetched: {last_successful_update}. In-memory queue length: {in_memory_queue_len}"
            ),
        )
    }
}

#[instrument(skip(client))]
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

#[instrument(skip(pool, payload))]
async fn enqueue_datafeed(
    pool: &Pool<Postgres>,
    payload: Value,
    updated_at: DateTime<Utc>,
) -> Result<(), EnqueueError> {
    let id = Uuid::now_v7();
    let mut tx = pool.begin().await?;

    sqlx::query(
        r"
        INSERT INTO datafeed_queue (id, updated_at, payload)
        VALUES ($1, $2, $3)
        ",
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
