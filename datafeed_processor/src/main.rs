mod database;
mod error;

use crate::database::queries::{
    archive_and_delete_datafeed, complete_callsign_sessions, complete_controller_sessions,
    complete_position_sessions, fetch_datafeed_batch, get_active_callsign_sessions,
    get_active_controller_session_keys, get_active_position_sessions,
    get_or_create_callsign_session, get_or_create_position_session, insert_controller_session,
    update_active_controller_session, update_callsign_session_last_seen,
    update_position_session_last_seen,
};

use crate::error::{
    BacklogProcessingError, CallsignParseError, PayloadProcessingError, ProcessorError,
};
use chrono::{DateTime, Utc};
use shared::PostgresConfig;
use shared::error::InitializationError;
use shared::load_config;
use shared::vnas::datafeed::DatafeedRoot;
use sqlx::postgres::{PgListener, PgPoolOptions};
use sqlx::{Pool, Postgres};
use std::collections::{HashMap, HashSet};
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
    // Create Db connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&pg_config.connection_string)
        .await?;

    // Run any new migrations
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

    let active_sessions = get_active_controller_session_keys(&mut *tx).await?;
    let mut active_by_cid: HashMap<i32, (Uuid, DateTime<Utc>, String, Uuid, String, Uuid)> =
        active_sessions
            .iter()
            .map(|session| {
                (
                    session.cid,
                    (
                        session.id,
                        session.login_time,
                        session.connected_callsign.clone(),
                        session.callsign_session_id,
                        session.primary_position_id.clone(),
                        session.position_session_id,
                    ),
                )
            })
            .collect();
    let active_callsign_sessions = get_active_callsign_sessions(&mut *tx)
        .await?
        .into_iter()
        .map(|s| s.id)
        .collect::<HashSet<_>>();
    let active_position_sessions = get_active_position_sessions(&mut *tx)
        .await?
        .into_iter()
        .map(|s| (s.position_id, s.id))
        .collect::<HashMap<_, _>>();
    let mut active_callsign_ids: HashSet<Uuid> = HashSet::new();
    let mut active_position_ids: HashSet<String> = HashSet::new();
    let mut extra_close_callsign: Vec<Uuid> = Vec::new();
    let mut extra_close_positions: Vec<Uuid> = Vec::new();
    let callsign_counts = active_sessions.iter().fold(HashMap::new(), |mut acc, s| {
        *acc.entry(s.callsign_session_id).or_insert(0usize) += 1;
        acc
    });
    let position_counts = active_sessions.iter().fold(HashMap::new(), |mut acc, s| {
        *acc.entry(s.position_session_id).or_insert(0usize) += 1;
        acc
    });
    let mut to_complete: Vec<Uuid> = Vec::new();

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
            if let Some((
                existing_id,
                existing_login_time,
                _existing_callsign,
                callsign_session_id,
                existing_position_id,
                position_session_id,
            )) = active_by_cid.remove(&cid)
            {
                // Check whether or not a new session has been started by comparing login times. Because we don't
                // check more frequently than every 15 seconds, we may have missed a disconnect and reconnect
                if existing_login_time == controller.login_time && existing_position_id == position_id {
                    update_callsign_session_last_seen(
                        &mut *tx,
                        callsign_session_id,
                        datafeed.updated_at,
                    )
                    .await?;
                    update_position_session_last_seen(
                        &mut *tx,
                        position_session_id,
                        datafeed.updated_at,
                    )
                    .await?;
                    update_active_controller_session(
                        &mut *tx,
                        existing_id,
                        &controller,
                        datafeed.updated_at,
                    )
                    .await?;
                    active_callsign_ids.insert(callsign_session_id);
                    active_position_ids.insert(existing_position_id);
                } else {
                    // Connected again with a new login time; close old session and start a new one.
                    to_complete.push(existing_id);
                    let callsign_count = callsign_counts
                        .get(&callsign_session_id)
                        .copied()
                        .unwrap_or(0);
                    let new_callsign_session_id = if callsign_count <= 1 {
                        extra_close_callsign.push(callsign_session_id);
                        get_or_create_callsign_session(
                            &mut *tx,
                            &prefix,
                            &suffix,
                            datafeed.updated_at,
                        )
                        .await?
                    } else {
                        update_callsign_session_last_seen(
                            &mut *tx,
                            callsign_session_id,
                            datafeed.updated_at,
                        )
                        .await?;
                        callsign_session_id
                    };
                    active_callsign_ids.insert(new_callsign_session_id);

                    let position_count = position_counts
                        .get(&position_session_id)
                        .copied()
                        .unwrap_or(0);
                    let new_position_session_id = if position_count <= 1 {
                        extra_close_positions.push(position_session_id);
                        get_or_create_position_session(
                            &mut *tx,
                            &position_id,
                            datafeed.updated_at,
                        )
                        .await?
                    } else {
                        update_position_session_last_seen(
                            &mut *tx,
                            position_session_id,
                            datafeed.updated_at,
                        )
                        .await?;
                        position_session_id
                    };
                    active_position_ids.insert(position_id.clone());
                    insert_controller_session(
                        &mut *tx,
                        &controller,
                        cid,
                        datafeed.updated_at,
                        new_callsign_session_id,
                        new_position_session_id,
                    )
                    .await?;
                }
            } else {
                let callsign_session_id =
                    get_or_create_callsign_session(&mut *tx, prefix, suffix, datafeed.updated_at)
                        .await?;
                active_callsign_ids.insert(callsign_session_id);
                let position_session_id = if let Some(id) =
                    active_position_sessions.get(&position_id).cloned()
                {
                    update_position_session_last_seen(&mut *tx, id, datafeed.updated_at).await?;
                    id
                } else {
                    let new_id =
                        get_or_create_position_session(&mut *tx, &position_id, datafeed.updated_at)
                            .await?;
                    new_id
                };
                active_position_ids.insert(position_id.clone());
                insert_controller_session(
                    &mut *tx,
                    &controller,
                    cid,
                    datafeed.updated_at,
                    callsign_session_id,
                    position_session_id,
                )
                .await?;
            }
        } else if let Some((
            existing_id,
            _existing_login_time,
            _existing_callsign,
            _callsign_session_id,
            _existing_position_id,
            _existing_position_session_id,
        )) = active_by_cid.remove(&cid)
        {
            // Controller reported inactive; close their session immediately.
            to_complete.push(existing_id);
        }
    }

    // Any active sessions not seen in this feed disappeared from the network.
    to_complete.extend(active_by_cid.values().map(|(id, _, _, _, _, _)| *id));

    if !to_complete.is_empty() {
        let closed =
            complete_controller_sessions(&mut *tx, &to_complete, datafeed.updated_at).await?;
        debug!(closed_sessions = closed, "marked sessions as completed");
    }

    let mut to_close_callsign: Vec<Uuid> = active_callsign_sessions
        .difference(&active_callsign_ids)
        .cloned()
        .collect();
    to_close_callsign.extend(extra_close_callsign);
    if !to_close_callsign.is_empty() {
        let closed =
            complete_callsign_sessions(&mut *tx, &to_close_callsign, datafeed.updated_at).await?;
        debug!(
            closed_callsign_sessions = closed,
            "marked callsign sessions as completed"
        );
    }

    let mut to_close_positions: Vec<Uuid> = active_position_sessions
        .iter()
        .filter_map(|(pos_id, session_id)| {
            if active_position_ids.contains(pos_id) {
                None
            } else {
                Some(*session_id)
            }
        })
        .collect();
    to_close_positions.extend(extra_close_positions);
    if !to_close_positions.is_empty() {
        let closed =
            complete_position_sessions(&mut *tx, &to_close_positions, datafeed.updated_at).await?;
        debug!(
            closed_position_sessions = closed,
            "marked position sessions as completed"
        );
    }

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
