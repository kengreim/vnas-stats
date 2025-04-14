mod database;

use rsmq_async::{Rsmq, RsmqConnection, RsmqError, RsmqOptions};
use shared::vnas::datafeed::DatafeedRoot;
use shared::{PostgresConfig, RedisConfig, RedisConfigLoader, load_config};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::str;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, warn};

#[derive(Debug, thiserror::Error)]
enum InitError {
    #[error("error with database")]
    Database(#[from] sqlx::Error),

    #[error("could not apply migrations")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .json()
        .with_file(true)
        .with_line_number(true)
        .with_env_filter("datafeed_processor=debug,sqlx=debug")
        //.with_env_filter(EnvFilter::from_default_env())
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

    // Initialized RSMQ
    let mut rsmq = initialize_rsmq(shared::DATAFEED_QUEUE_NAME, &config.redis)
        .await
        .inspect_err(|e| {
            error!(error = ?e, "Could not initialize Redis connection");
            panic!("Could not initialize Redis connection");
        })?;

    loop {
        let msg = rsmq
            .receive_message::<Vec<u8>>(shared::DATAFEED_QUEUE_NAME, None)
            .await;

        if let Err(e) = &msg {
            warn!(error = ?e, "Error receiving message from Redis");
            sleep(Duration::from_secs(1)).await;
            continue;
        }

        if let Some(message) = msg.expect("Error receiving message from Redis") {
            debug!(id = message.id, "Received datafeed message from Redis");

            // Delete message from queue to prevent redelivery
            // See https://docs.rs/rsmq_async/latest/rsmq_async/trait.RsmqConnection.html#tymethod.delete_message
            match rsmq
                .delete_message(shared::DATAFEED_QUEUE_NAME, &message.id)
                .await
            {
                Ok(true) => debug!(id = ?message.id, "Redis message deleted"),
                Ok(false) => {
                    warn!(id = ?message.id, "Redis message failed to delete, false returned")
                }
                Err(e) => warn!(error =?e, "Error deleting received message from Redis"),
            }

            // Deserialize message
            let datafeed_root = match str::from_utf8(&message.message) {
                Ok(msg) => match serde_json::from_str::<DatafeedRoot>(msg) {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(error = ?e, "Error deserializing JSON from Redis");
                        continue;
                    }
                },
                Err(e) => {
                    warn!(error = ?e, "Error deserializing bytes as UTF-8");
                    continue;
                }
            };
            debug!(timestamp = ?datafeed_root.updated_at, num_controllers = datafeed_root.controllers.len(), "Deserialized datafeed message");

            // TODO -- do something with root
        }
    }
}

async fn initialize_rsmq(queue_name: &str, config: &RedisConfig) -> Result<Rsmq, RsmqError> {
    let connection_options = RsmqOptions::from_config(config);

    let mut rsmq = Rsmq::new(connection_options).await?;
    let queues = rsmq.list_queues().await?;
    if !queues.contains(&queue_name.to_string()) {
        rsmq.create_queue(queue_name, None, None, Some(-1)).await?
    }

    Ok(rsmq)
}

async fn initialize_db(pg_config: &PostgresConfig) -> Result<Pool<Postgres>, InitError> {
    // Create Db connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&pg_config.connection_string)
        .await?;

    // Run any new migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}
