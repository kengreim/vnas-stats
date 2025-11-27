#[warn(clippy::pedantic)]
mod database;
mod error;
mod helpers;

use crate::database::queries::{
    archive_and_delete_datafeed, complete_controller_sessions, fetch_datafeed_batch,
    insert_controller_session, update_active_controller_session, update_callsign_session_last_seen,
    update_position_session_last_seen,
};
use crate::error::{BacklogProcessingError, PayloadProcessingError, ProcessorError};
use crate::helpers::ControllerCloseReason;
use crate::helpers::{
    ActiveState, ControllerAction, ParsedController, ensure_callsign_session,
    ensure_position_session, finalize_callsign_sessions, finalize_position_sessions,
    load_active_state, login_times_match, parse_controller_parts,
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
use tracing::{Level, debug, event_enabled, info, trace, warn};
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

    sqlx::migrate!("../migrations").run(&pool).await?;

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

    let mut existing_state = load_active_state(&mut tx).await?;
    let ActiveState {
        active_by_cid: existing_active_by_cid,
        active_callsign_sessions: existing_active_callsign_sessions,
        active_callsign_sessions_map: existing_active_callsign_sessions_map,
        active_position_sessions: existing_active_position_sessions,
    } = &mut existing_state;
    let mut active_callsign_ids: HashSet<Uuid> = HashSet::new();
    let mut active_position_ids: HashSet<String> = HashSet::new();
    let mut controller_actions: Vec<ControllerAction> = Vec::new();

    // First pass: only handle Controller-Position Sessions (i.e., a session with the unique combination
    // of a CID, primary position ID and loging time)
    for controller in datafeed.controllers {
        let ParsedController {
            cid,
            prefix,
            suffix,
            position_id,
        } = match parse_controller_parts(&controller) {
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
                let callsign_session_id = ensure_callsign_session(
                    &mut tx,
                    existing_active_callsign_sessions_map,
                    callsign_key,
                    datafeed.updated_at,
                )
                .await?;
                let position_session_id = ensure_position_session(
                    &mut tx,
                    existing_active_position_sessions,
                    position_id,
                    datafeed.updated_at,
                )
                .await?;
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

    // Only need to run these additional allocations if we are logging at DEBUG level
    if event_enabled!(Level::DEBUG) {
        debug!("completed processing controller sessions");

        let created = controller_actions
            .iter()
            .filter_map(|a| {
                if let ControllerAction::CreateNew {
                    controller, cid, ..
                } = a
                {
                    Some((cid, controller.vatsim_data.callsign.clone()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let closed = controller_actions
            .iter()
            .filter_map(|a| {
                if let ControllerAction::Close {
                    cid,
                    connected_callsign,
                    reason,
                    ..
                } = a
                {
                    Some((cid, connected_callsign, reason))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if created.is_empty() {
            debug!("no new controller sessions");
        } else {
            debug!(controllers = ?created, "created new controller sessions");
        }

        if closed.is_empty() {
            debug!("no closed sessions");
        } else {
            debug!(controllers = ?closed, "closed controller sessions");
        }
    }

    finalize_callsign_sessions(
        &mut tx,
        existing_active_callsign_sessions,
        &active_callsign_ids,
        datafeed.updated_at,
    )
    .await?;

    finalize_position_sessions(
        &mut tx,
        existing_active_position_sessions,
        &active_position_ids,
        datafeed.updated_at,
    )
    .await?;

    tx.commit().await?;
    Ok(())
}
