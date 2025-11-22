mod database;

use crate::database::queries::{
    QueryError, archive_datafeed, complete_sessions, delete_queued_datafeed, fetch_datafeed_batch,
    get_active_session_keys, insert_controller_session, update_active_controller_session,
};
use chrono::{DateTime, Utc};
use shared::error::ConfigError;
use shared::vnas::datafeed::DatafeedRoot;
use shared::{PostgresConfig, load_config};
use sqlx::postgres::{PgListener, PgPoolOptions};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use tokio::time::{Duration, sleep};
use tracing::subscriber::SetGlobalDefaultError;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
enum ProcessorError {
    #[error("failed to set global tracing subscriber: {0}")]
    Tracing(#[from] SetGlobalDefaultError),
    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("query error: {0}")]
    Query(#[from] QueryError),
    #[error("datafeed deserialization error: {0}")]
    Deserialize(#[from] serde_json::Error),
}

#[tokio::main]
async fn main() -> Result<(), ProcessorError> {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .json()
        .with_file(true)
        .with_line_number(true)
        .with_env_filter("datafeed_processor=debug,sqlx=debug")
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    // Set up config
    let config = load_config().inspect_err(|e| {
        error!(error = ?e, "Configuration could not be initialized");
        panic!("Configuration could not be initialized")
    })?;

    // Initialize DB
    let db_pool = match initialize_db(&config.postgres).await {
        Ok(db_pool) => db_pool,
        Err(e) => {
            error!(error = ?e, "Could not initialize DB connection pool");
            panic!("Could not initialize DB connection pool")
        }
    };

    // Listen for new datafeeds
    let mut listener = PgListener::connect_with(&db_pool).await?;
    listener.listen("datafeed_queue").await?;
    info!("Listening for new datafeeds via Postgres NOTIFY");

    // Process any backlog before listening
    if let Err(e) = process_pending_datafeeds(&db_pool).await {
        warn!(error = ?e, "Failed to process pending datafeeds on startup");
    }

    loop {
        match listener.recv().await {
            Ok(notification) => {
                debug!(
                    payload = notification.payload(),
                    "Received datafeed notification"
                );
                if let Err(e) = process_pending_datafeeds(&db_pool).await {
                    warn!(error = ?e, "Failed to process datafeeds from queue");
                    // avoid tight loop if errors persist
                    sleep(Duration::from_secs(1)).await;
                }
            }
            Err(e) => {
                warn!(error = ?e, "Error receiving Postgres notification");
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn initialize_db(pg_config: &PostgresConfig) -> Result<Pool<Postgres>, ProcessorError> {
    // Create Db connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&pg_config.connection_string)
        .await?;

    // Run any new migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

async fn process_pending_datafeeds(pool: &Pool<Postgres>) -> Result<(), ProcessorError> {
    loop {
        let mut tx = pool.begin().await?;
        let messages = fetch_datafeed_batch(&mut *tx, 25).await?;

        if messages.is_empty() {
            tx.commit().await?;
            break;
        }

        for message in messages {
            let parsed = serde_json::from_value::<DatafeedRoot>(message.payload.clone());
            let datafeed_root = match parsed {
                Ok(root) => root,
                Err(e) => {
                    warn!(error = ?e, id = %message.id, "Failed to deserialize datafeed payload; archiving and skipping");
                    archive_datafeed(&mut *tx, &message, Utc::now()).await?;
                    delete_queued_datafeed(&mut *tx, message.id).await?;
                    continue;
                }
            };

            if let Err(e) = process_datafeed_payload(pool, datafeed_root).await {
                warn!(error = ?e, id = %message.id, "Failed to process datafeed payload; will retry");
                tx.rollback().await?;
                return Err(e);
            }

            archive_datafeed(&mut *tx, &message, Utc::now()).await?;
            delete_queued_datafeed(&mut *tx, message.id).await?;
        }

        tx.commit().await?;
    }

    Ok(())
}

async fn process_datafeed_payload(
    pool: &Pool<Postgres>,
    datafeed: DatafeedRoot,
) -> Result<(), ProcessorError> {
    let mut tx = pool.begin().await?;

    let active_sessions = get_active_session_keys(&mut *tx).await?;
    let mut active_by_cid: HashMap<i32, (Uuid, DateTime<Utc>)> = active_sessions
        .into_iter()
        .map(|session| (session.cid, (session.id, session.login_time)))
        .collect();
    let mut to_complete: Vec<Uuid> = Vec::new();

    for controller in datafeed.controllers {
        let cid: i32 = match controller.vatsim_data.cid.parse() {
            Ok(cid) => cid,
            Err(e) => {
                warn!(error = ?e, cid = controller.vatsim_data.cid, "Skipping controller with invalid CID");
                continue;
            }
        };

        if controller.is_active {
            if let Some((existing_id, existing_login_time)) = active_by_cid.remove(&cid) {
                if existing_login_time == controller.login_time {
                    update_active_controller_session(
                        &mut *tx,
                        existing_id,
                        &controller,
                        datafeed.updated_at,
                    )
                    .await?;
                } else {
                    // Connected again with a new login time; close old session and start a new one.
                    to_complete.push(existing_id);
                    insert_controller_session(&mut *tx, &controller, cid, datafeed.updated_at)
                        .await?;
                }
            } else {
                insert_controller_session(&mut *tx, &controller, cid, datafeed.updated_at).await?;
            }
        } else if let Some((existing_id, _existing_login_time)) = active_by_cid.remove(&cid) {
            // Controller reported inactive; close their session immediately.
            to_complete.push(existing_id);
        }
    }

    // Any active sessions not seen in this feed disappeared from the network.
    to_complete.extend(active_by_cid.into_values().map(|(id, _)| id));

    if !to_complete.is_empty() {
        let closed = complete_sessions(&mut *tx, &to_complete, datafeed.updated_at).await?;
        debug!(closed_sessions = closed, "Marked sessions as completed");
    }

    tx.commit().await?;
    Ok(())
}
