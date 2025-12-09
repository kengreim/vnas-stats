#[warn(clippy::pedantic)]
mod database;
mod error;
mod helpers;
mod logging;

use crate::database::queries::{
    complete_controller_sessions, delete_queued_datafeed, fetch_datafeed_batch,
    insert_controller_session, insert_datafeed_message, update_active_controller_session,
    update_callsign_session_last_seen, update_position_session_last_seen, upsert_datafeed_payload,
};
use crate::error::{BacklogProcessingError, PayloadProcessingError, ProcessorMainError};
use crate::helpers::ControllerCloseReason;
use crate::helpers::{
    ActiveState, ControllerAction, ParsedController, ensure_callsign_session,
    ensure_position_session, finalize_callsign_sessions, finalize_position_sessions,
    load_active_state, login_times_match, parse_controller_parts,
};
use crate::logging::debug_log_sessions_changes;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use shared::error::InitializationError;
use shared::vnas::datafeed::DatafeedRoot;
use shared::{init_tracing_and_oltp, initialize_db, load_config, shutdown_listener};
use sqlx::postgres::PgListener;
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::time::{Duration, sleep};
use tokio_util::sync::CancellationToken;
use tracing::{Level, debug, event_enabled, info, trace, warn};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), ProcessorMainError> {
    let (subscriber, tracer_provider) = init_tracing_and_oltp("datafeed_processor")?;
    tracing::subscriber::set_global_default(subscriber).map_err(InitializationError::from)?;

    // Set up config
    let config = load_config().map_err(InitializationError::from)?;

    // Initialize DB
    let db_pool = initialize_db(&config.postgres).await?;

    // Arc for state for health check endpoint
    let last_processed_datafeed = Arc::new(RwLock::new(None));

    // Cancellation token shared across tasks; listener cancels on SIGINT/SIGTERM.
    let shutdown_token = CancellationToken::new();

    // Spawn listener, axum (health check endpoint) and datafeed processor tasks
    let mut signal_handle = tokio::spawn(shutdown_listener(Some(shutdown_token.clone())));
    let mut axum_handle = tokio::spawn(run_health_server(
        Arc::clone(&last_processed_datafeed),
        shutdown_token.clone(),
    ));
    let mut processor_handle = tokio::spawn(run_datafeed_processing_loop(
        db_pool,
        Arc::clone(&last_processed_datafeed),
        shutdown_token.clone(),
    ));

    let mut first_err: Option<ProcessorMainError> = None;
    let mut axum_done = false;
    let mut processor_done = false;

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
        res = &mut processor_handle => {
            info!("processor task completed first, propagating cancellation token to other tasks");
            processor_done = true;
            shutdown_token.cancel();
            match res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    warn!(error = ?e, "processor task completed due to error");
                    first_err.get_or_insert(e);
                }
                Err(join) => {
                    warn!(error = ?join, "processor task completed due to error");
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
    if !processor_done {
        info!("awaiting completion of processor task");
        match processor_handle.await {
            Ok(Ok(())) => {
                info!("processor task completed successfully");
            }
            Ok(Err(e)) => {
                info!(error = ?e, "processor task completed with error");
                first_err.get_or_insert(e);
            }
            Err(join) => {
                info!(error = ?join, "processor task completed with error");
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

#[derive(Clone)]
struct AxumState {
    last_processed_datafeed: Arc<RwLock<Option<DateTime<Utc>>>>,
}

async fn run_health_server(
    last_processed_datafeed: Arc<RwLock<Option<DateTime<Utc>>>>,
    shutdown: CancellationToken,
) -> Result<(), std::io::Error> {
    info!("starting axum health server");
    let app = Router::new()
        .route("/health", get(health_check))
        .with_state(AxumState {
            last_processed_datafeed,
        });
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await?;

    Ok(())
}

async fn health_check(State(state): State<AxumState>) -> impl IntoResponse {
    let last_processed_datafeed = *state.last_processed_datafeed.read();
    let msg = if let Some(timestamp) = last_processed_datafeed {
        format!("Last processed datafeed updated_at: {timestamp}")
    } else {
        "No datafeeds processed yet".into()
    };

    (StatusCode::OK, msg)
}

async fn run_datafeed_processing_loop(
    db_pool: Pool<Postgres>,
    last_processed_datafeed: Arc<RwLock<Option<DateTime<Utc>>>>,
    shutdown: CancellationToken,
) -> Result<(), ProcessorMainError> {
    // Process any backlog before listening
    info!("starting processing backlog of queued datafeeds");
    process_pending_datafeeds(&db_pool, &last_processed_datafeed, 25).await?;

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
        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("shutdown requested, exiting processor loop");
                break;
            }
            recv = listener.recv() => {
                match recv {
                    Ok(notification) => {
                        trace!(payload = notification.payload(), "received datafeed notification");
                        // Process pending datafeeds; if this fails, propagate the error after finishing this payload.
                        process_pending_datafeeds(&db_pool, &last_processed_datafeed, 10).await?;
                    }
                    Err(e) => {
                        warn!(error = ?e, "error receiving Postgres notification");
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn process_pending_datafeeds(
    pool: &Pool<Postgres>,
    last_processed_datafeed: &RwLock<Option<DateTime<Utc>>>,
    limit: i64,
) -> Result<(), BacklogProcessingError> {
    loop {
        let mut tx = pool.begin().await.map_err(BacklogProcessingError::from)?;
        let messages = fetch_datafeed_batch(&mut *tx, limit).await?;

        if messages.is_empty() {
            tx.commit().await?;
            break;
        }

        let mut latest = None;
        for message in messages {
            let parsed = serde_json::from_value::<DatafeedRoot>(message.payload.clone());
            let datafeed_root = match parsed {
                Ok(root) => root,
                Err(e) => {
                    tx.rollback().await?;
                    return Err(PayloadProcessingError::Deserialize(e).into());
                }
            };

            // Upsert payload; if not inserted (already seen), skip session processing.
            let (payload_id, new_payload) = upsert_datafeed_payload(tx.as_mut(), &message).await?;

            if new_payload {
                debug!(updated_at = ?datafeed_root.updated_at, "new datafeed update received");
                if let Err(e) = process_datafeed_payload(pool, &datafeed_root).await {
                    tx.rollback().await?;
                    return Err(e.into());
                }
            } else {
                trace!(
                    updated_at = ?datafeed_root.updated_at,
                    "skipping processing; datafeed already processed"
                );
            }

            insert_datafeed_message(
                tx.as_mut(),
                message.id,
                payload_id,
                message.created_at,
                Utc::now(),
            )
            .await?;
            delete_queued_datafeed(tx.as_mut(), message.id).await?;
            latest = Some(datafeed_root.updated_at);
        }

        tx.commit().await.map_err(BacklogProcessingError::from)?;
        *last_processed_datafeed.write() = latest;
    }

    Ok(())
}

async fn process_datafeed_payload(
    pool: &Pool<Postgres>,
    datafeed: &DatafeedRoot,
) -> Result<(), PayloadProcessingError> {
    let mut tx = pool.begin().await?;

    let mut existing_state = load_active_state(&mut tx).await?;
    let ActiveState {
        active_by_cid: existing_active_by_cid,
        active_callsign_sessions: existing_active_callsign_sessions,
        active_callsign_sessions_map: existing_active_callsign_sessions_map,
        active_position_sessions: existing_active_position_sessions,
    } = &mut existing_state;
    let mut active_callsign_ids: HashSet<Uuid> = HashSet::new();
    let mut active_position_ids: HashSet<String> = HashSet::new();
    let mut new_callsign_session_ids: HashSet<Uuid> = HashSet::new();
    let mut new_position_session_ids: HashSet<Uuid> = HashSet::new();
    let mut controller_actions: Vec<ControllerAction> = Vec::new();

    // First pass: only handle Controller-Position Sessions (i.e., a session with the unique combination
    // of a CID, primary position ID and loging time)
    for controller in &datafeed.controllers {
        let ParsedController {
            cid,
            prefix,
            suffix,
            position_id,
        } = match parse_controller_parts(controller) {
            Ok(parts) => parts,
            Err(e) => {
                warn!(
                    error = ?e,
                    callsign = controller.vatsim_data.callsign,
                    cid = controller.vatsim_data.cid,
                    "skipping controller with invalid identifiers"
                );
                continue;
            }
        };

        if controller.is_active {
            trace!(
                cid = cid,
                prefix = prefix,
                suffix = suffix,
                position_id = position_id,
                "controller in datafeed is active, starting processing"
            );
            if let Some(existing) = existing_active_by_cid.remove(&cid) {
                if login_times_match(&existing.login_time, &controller.login_time)
                    && existing.position_id == position_id
                {
                    trace!(
                        "controller was previously tracked and has same login time, position_id and callsign"
                    );
                    controller_actions.push(ControllerAction::UpdateExisting {
                        session_id: existing.controller_session_id,
                        controller: controller.clone(),
                        callsign_session_id: existing.callsign_session_id,
                        position_session_id: existing.position_session_id,
                    });
                    active_callsign_ids.insert(existing.callsign_session_id);
                    active_position_ids.insert(existing.position_id);
                } else {
                    trace!(
                        current_position_id = position_id,
                        existing_position_id = existing.position_id,
                        current_login_time = ?controller.login_time,
                        existing_login_time = ?existing.login_time,
                        "one of position ID or login_time does not match existing values, closing existing controller session and starting new"
                    );
                    controller_actions.push(ControllerAction::Close {
                        session_id: existing.controller_session_id,
                        cid,
                        callsign_session_id: existing.callsign_session_id,
                        position_session_id: existing.position_session_id,
                        connected_callsign: existing.connected_callsign,
                        reason: ControllerCloseReason::ReconnectedOrChangedPosition,
                    });
                    controller_actions.push(ControllerAction::CreateNew {
                        controller: controller.clone(),
                        callsign_key: (prefix.to_string(), suffix.to_string()),
                        position_id: position_id.clone(),
                        cid,
                    });
                }
            } else {
                trace!("controller was not previously tracked, creating new session");
                controller_actions.push(ControllerAction::CreateNew {
                    controller: controller.clone(),
                    callsign_key: (prefix.to_string(), suffix.to_string()),
                    position_id: position_id.clone(),
                    cid,
                });
            }
        } else if let Some(existing) = existing_active_by_cid.remove(&cid) {
            trace!(
                cid = cid,
                prefix = prefix,
                suffix = suffix,
                position_id = position_id,
                exisiting_controller_session_id = %existing.controller_session_id,
                "tracked controller is still in the datafeed but no longer active, closing session"
            );
            controller_actions.push(ControllerAction::Close {
                session_id: existing.controller_session_id,
                cid,
                callsign_session_id: existing.callsign_session_id,
                position_session_id: existing.position_session_id,
                connected_callsign: existing.connected_callsign,
                reason: ControllerCloseReason::DeactivatedPosition,
            });
        }
    }

    if event_enabled!(Level::TRACE) {
        let remaining_to_close = existing_active_by_cid.values().collect::<Vec<_>>();
        for missing in &remaining_to_close {
            trace!(controller = ?missing, "previously tracked controller no longer seen in datafeed");
        }
    }

    controller_actions.extend(existing_active_by_cid.iter_mut().map(|(cid, state)| {
        ControllerAction::Close {
            session_id: state.controller_session_id,
            cid: *cid,
            callsign_session_id: state.callsign_session_id,
            position_session_id: state.position_session_id,
            connected_callsign: state.connected_callsign.clone(),
            reason: ControllerCloseReason::MissingFromDatafeed,
        }
    }));

    // Second pass: Close controller sessions first to avoid unique constraint conflicts
    let close_controller_session_ids: Vec<Uuid> = controller_actions
        .iter()
        .filter_map(|action| match action {
            ControllerAction::Close { session_id, .. } => Some(*session_id),
            _ => None,
        })
        .collect();

    if !close_controller_session_ids.is_empty() {
        let _ = complete_controller_sessions(
            &mut *tx,
            &close_controller_session_ids,
            datafeed.updated_at,
        )
        .await?;
    }

    // Now, handle updates and inserts (any matches on Close do nothing)
    for action in &controller_actions {
        match action {
            ControllerAction::UpdateExisting {
                session_id,
                controller,
                callsign_session_id,
                position_session_id,
            } => {
                update_callsign_session_last_seen(
                    tx.as_mut(),
                    *callsign_session_id,
                    datafeed.updated_at,
                )
                .await?;
                update_position_session_last_seen(
                    tx.as_mut(),
                    *position_session_id,
                    datafeed.updated_at,
                )
                .await?;
                update_active_controller_session(
                    tx.as_mut(),
                    *session_id,
                    controller,
                    datafeed.updated_at,
                )
                .await?;
                active_callsign_ids.insert(*callsign_session_id);
                active_position_ids.insert(controller.primary_position_id.clone());
            }
            ControllerAction::CreateNew {
                controller,
                callsign_key,
                position_id,
                cid,
            } => {
                let (callsign_session_id, callsign_created) = ensure_callsign_session(
                    &mut tx,
                    existing_active_callsign_sessions_map,
                    callsign_key,
                    datafeed.updated_at,
                )
                .await?;
                if callsign_created {
                    new_callsign_session_ids.insert(callsign_session_id);
                }

                let (position_session_id, position_created) = ensure_position_session(
                    &mut tx,
                    existing_active_position_sessions,
                    position_id,
                    datafeed.updated_at,
                )
                .await?;
                if position_created {
                    new_position_session_ids.insert(position_session_id);
                }

                insert_controller_session(
                    tx.as_mut(),
                    controller,
                    *cid,
                    datafeed.updated_at,
                    callsign_session_id,
                    position_session_id,
                )
                .await?;
                active_callsign_ids.insert(callsign_session_id);
                active_position_ids.insert(position_id.to_string());
            }
            // Handled close actions before this loop
            ControllerAction::Close { .. } => {}
        }
    }
    trace!("completed processing controller sessions");

    let closed_callsign_session_ids = finalize_callsign_sessions(
        &mut tx,
        existing_active_callsign_sessions,
        &active_callsign_ids,
        datafeed.updated_at,
    )
    .await?;
    trace!("completed processing callsign sessions");

    let closed_position_session_ids = finalize_position_sessions(
        &mut tx,
        existing_active_position_sessions,
        &active_position_ids,
        datafeed.updated_at,
    )
    .await?;
    trace!("completed processing position sessions");

    tx.commit().await?;

    // Note: this function will only return if log level is DEBUG or TRACE, otherwise it returns
    // immediately
    debug_log_sessions_changes(
        pool,
        &controller_actions,
        &new_callsign_session_ids,
        &new_position_session_ids,
        &closed_callsign_session_ids,
        &closed_position_session_ids,
    )
    .await?;

    Ok(())
}
