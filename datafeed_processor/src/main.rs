mod database;
mod error;
mod helpers;

use crate::database::queries::{
    archive_and_delete_datafeed, complete_controller_sessions, fetch_datafeed_batch,
};
use crate::error::{
    BacklogProcessingError, CallsignParseError, PayloadProcessingError, ProcessorError,
};
use crate::helpers::{
    ActiveState, SessionCollections, SessionMaps, finalize_callsign_sessions,
    finalize_position_sessions, handle_active_controller, load_active_state,
};
use chrono::Utc;
use shared::PostgresConfig;
use shared::error::InitializationError;
use shared::load_config;
use shared::vnas::datafeed::DatafeedRoot;
use sqlx::postgres::{PgListener, PgPoolOptions};
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use tokio::time::{Duration, sleep};
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), ProcessorError> {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber).map_err(InitializationError::from)?;

    // Set up config
    let config = load_config().map_err(InitializationError::from)?;

    // Initialize DB
    let db_pool = initialize_db(&config.postgres).await?;

    // Process any backlog before listening
    info!("starting processing backlog of queued datafeeds");
    process_pending_datafeeds(&db_pool, 25).await?;

    // Listen for new datafeeds
    let mut listener = PgListener::connect_with(&db_pool)
        .await
        .map_err(InitializationError::from)?;
    listener
        .listen("datafeed_queue")
        .await
        .map_err(InitializationError::from)?;
    info!("listening for new datafeeds via Postgres NOTIFY");

    loop {
        match listener.recv().await {
            Ok(notification) => {
                debug!(
                    payload = notification.payload(),
                    "received datafeed notification"
                );
                // If this fails, we end all processing by throwing error out of main
                // because any datafeeds that failed to process will always remain in the queue
                // and will cause future batches to fail until fixed
                process_pending_datafeeds(&db_pool, 10).await?;
            }
            Err(e) => {
                warn!(error = ?e, "error receiving Postgres notification");
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn initialize_db(pg_config: &PostgresConfig) -> Result<Pool<Postgres>, InitializationError> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&pg_config.connection_string)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

async fn process_pending_datafeeds(
    pool: &Pool<Postgres>,
    limit: i64,
) -> Result<(), BacklogProcessingError> {
    loop {
        let mut tx = pool.begin().await.map_err(BacklogProcessingError::from)?;
        let messages = fetch_datafeed_batch(&mut *tx, limit).await?;

        if messages.is_empty() {
            tx.commit().await?;
            break;
        }

        for message in messages {
            let parsed = serde_json::from_value::<DatafeedRoot>(message.payload.clone());
            let datafeed_root = match parsed {
                Ok(root) => root,
                Err(e) => {
                    tx.rollback().await?;
                    return Err(PayloadProcessingError::Deserialize(e).into());
                }
            };

            if let Err(e) = process_datafeed_payload(pool, datafeed_root).await {
                tx.rollback().await?;
                return Err(e.into());
            }

            archive_and_delete_datafeed(&mut *tx, &message, Utc::now()).await?;
        }

        tx.commit().await.map_err(BacklogProcessingError::from)?;
    }

    Ok(())
}

async fn process_datafeed_payload(
    pool: &Pool<Postgres>,
    datafeed: DatafeedRoot,
) -> Result<(), PayloadProcessingError> {
    let mut tx = pool.begin().await?;

    let mut active_state = load_active_state(&mut tx).await?;
    let ActiveState {
        active_by_cid,
        callsign_counts,
        position_counts,
        active_callsign_sessions,
        active_position_sessions,
    } = &mut active_state;
    let mut active_callsign_ids: HashSet<Uuid> = HashSet::new();
    let mut active_position_ids: HashSet<String> = HashSet::new();
    let mut extra_close_callsign: Vec<Uuid> = Vec::new();
    let mut extra_close_positions: Vec<Uuid> = Vec::new();
    let mut controllers_to_complete: Vec<Uuid> = Vec::new();

    for controller in datafeed.controllers {
        let cid: i32 = match controller.vatsim_data.cid.parse() {
            Ok(cid) => cid,
            Err(e) => {
                warn!(error = ?e, cid = controller.vatsim_data.cid, "skipping controller with invalid CID");
                continue;
            }
        };

        let (prefix, _infix, suffix) = match parse_callsign(&controller.vatsim_data.callsign) {
            Ok(parts) => parts,
            Err(e) => {
                warn!(
                    error = ?e,
                    callsign = controller.vatsim_data.callsign,
                    "skipping controller with invalid callsign format"
                );
                continue;
            }
        };
        let position_id = controller.primary_position_id.clone();

        if controller.is_active {
            let maps = SessionMaps {
                active_by_cid,
                callsign_counts,
                position_counts,
                active_position_sessions,
            };
            let collections = SessionCollections {
                active_callsign_ids: &mut active_callsign_ids,
                active_position_ids: &mut active_position_ids,
                extra_close_callsign: &mut extra_close_callsign,
                extra_close_positions: &mut extra_close_positions,
                controllers_to_complete: &mut controllers_to_complete,
            };
            handle_active_controller(
                &mut tx,
                &controller,
                cid,
                prefix,
                suffix,
                &position_id,
                datafeed.updated_at,
                maps,
                collections,
            )
            .await?;
        } else if let Some(existing) = active_by_cid.remove(&cid) {
            controllers_to_complete.push(existing.controller_session_id);
        }
    }

    controllers_to_complete.extend(
        active_by_cid
            .values()
            .map(|state| state.controller_session_id),
    );

    if !controllers_to_complete.is_empty() {
        let closed =
            complete_controller_sessions(&mut *tx, &controllers_to_complete, datafeed.updated_at)
                .await?;
        debug!(closed_sessions = closed, "marked sessions as completed");
    }

    finalize_callsign_sessions(
        &mut tx,
        &active_callsign_sessions,
        &active_callsign_ids,
        extra_close_callsign,
        datafeed.updated_at,
    )
    .await?;

    finalize_position_sessions(
        &mut tx,
        &active_position_sessions,
        &active_position_ids,
        extra_close_positions,
        datafeed.updated_at,
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

type Callsign<'a> = (&'a str, Option<&'a str>, &'a str);
fn parse_callsign(callsign: &str) -> Result<Callsign<'_>, CallsignParseError> {
    let parts: Vec<&str> = callsign.split('_').collect();
    // Direct indexing below is safe because we have already checked the length
    match parts.len() {
        2 => Ok((parts[0], None, parts[1])),
        3 => Ok((parts[0], Some(parts[1]), parts[2])),
        other => Err(CallsignParseError::IncorrectFormat(other)),
    }
}
