use chrono::{DateTime, Utc};
use rsmq_async::{Rsmq, RsmqConnection, RsmqError, RsmqOptions};
use serde_json::Value;
use shared::load_config;
use shared::vnas::datafeed::{VnasEnvironment, datafeed_url};
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .json()
        .with_file(true)
        .with_line_number(true)
        //.with_env_filter("datafeed_fetcher=debug")
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    // Set up config
    let config = load_config().inspect_err(|e| {
        error!(error = ?e, "Configuration could not be initialized");
        panic!("Configuration could not be initialized")
    })?;

    // Set up Redis Queue based on configuration
    let connection_options = RsmqOptions::from(&config.redis);

    let mut rsmq = initialize_rsmq(connection_options, config.redis.force_recreate)
        .await
        .inspect_err(|e| {
            error!(error = ?e, "RSMQ could not be initialized");
            panic!("RSMQ could not be initialized")
        })?;

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

        let (bytes, current_timestamp) =
            match fetch_datafeed_bytes_and_timestamp(&http_client).await {
                Ok((b, t)) => (b, t),
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
        let sent = rsmq
            .send_message::<Vec<u8>>(shared::DATAFEED_QUEUE_NAME, bytes, None)
            .await;
        if let Err(e) = sent {
            warn!(error = ?e, "Could not send message to Redis");
            continue;
        } else {
            debug!("Sent message to Redis");
        }
    }
}

async fn initialize_rsmq(
    connection_options: RsmqOptions,
    force_recreate: bool,
) -> Result<Rsmq, RsmqError> {
    let mut rsmq = Rsmq::new(connection_options).await?;
    let queues = rsmq.list_queues().await?;

    let queue_exists = queues.contains(&shared::DATAFEED_QUEUE_NAME.to_string());
    if queue_exists && force_recreate {
        rsmq.delete_queue(shared::DATAFEED_QUEUE_NAME).await?;
        rsmq.create_queue(shared::DATAFEED_QUEUE_NAME, None, None, Some(-1))
            .await?;
    } else if queue_exists {
        rsmq.set_queue_attributes(shared::DATAFEED_QUEUE_NAME, None, None, Some(-1))
            .await?;
    } else {
        rsmq.create_queue(shared::DATAFEED_QUEUE_NAME, None, None, Some(-1))
            .await?
    }

    Ok(rsmq)
}

#[derive(Error, Debug)]
enum DatafeedFetchError {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Deserialize(#[from] serde_json::Error),
    #[error(transparent)]
    TimestampDeserialize(#[from] chrono::format::ParseError),
    #[error("unable to find or parse updatedAt field in JSON")]
    MissingUpdatedAt,
}

async fn fetch_datafeed_bytes_and_timestamp(
    client: &reqwest::Client,
) -> Result<(Vec<u8>, DateTime<Utc>), DatafeedFetchError> {
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
        Ok((resp.into_bytes(), timestamp.with_timezone(&Utc)))
    } else {
        Err(DatafeedFetchError::MissingUpdatedAt)
    }
}
