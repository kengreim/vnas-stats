use rsmq_async::{Rsmq, RsmqConnection, RsmqError, RsmqOptions};
use shared::vnas::datafeed::{DatafeedRoot, VnasEnvironment, datafeed_url};
use shared::{RedisConfigLoader, load_config};
use std::cmp::min;
use std::time::{Duration, Instant};
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
    let connection_options = RsmqOptions::from_config(&config.redis);

    let mut rsmq = initialize_rsmq(connection_options, config.redis.force_recreate)
        .await
        .inspect_err(|e| {
            error!(error = ?e, "RSMQ could not be initialized");
            panic!("RSMQ could not be initialized")
        })?;

    let mut last_datafeed_update = None;
    info!("Initialized Datafeed Fetcher");

    // Datafetcher infinite loop
    loop {
        let start = Instant::now();

        let latest_data_result = fetch_datafeed().await;
        if let Err(e) = latest_data_result {
            warn!(error = ?e, "Could not fetch or deserialize vNAS datafeed");
            sleep(Duration::from_secs(1)).await;
            continue;
        };

        // Unwrap and check if duplicate from last fetch
        // Safe to unwrap because checked Err case above already
        let latest_data = latest_data_result.expect("Could not fetch or deserialize vNAS datafeed");
        if let Some(last_datafeed_time) = last_datafeed_update {
            if last_datafeed_time == latest_data.updated_at {
                debug!(time = %latest_data.updated_at, "Found duplicate");
                sleep(Duration::from_secs(15)).await;
                continue;
            }
        }

        // Update timestamp of latest data and process datafeed
        debug!(time = %latest_data.updated_at, "Found new datafeed");
        last_datafeed_update = Some(latest_data.updated_at);

        // Send message to Redis with Controllers JSON
        let sent = rsmq
            .send_message::<Vec<u8>>(
                shared::DATAFEED_QUEUE_NAME,
                serde_json::to_string(&latest_data)?.into_bytes(),
                None,
            )
            .await;
        if let Err(e) = sent {
            warn!(error = ?e, "Could not send message to Redis");
            // No continue here because at this point we want to sleep for 5 seconds
        } else {
            debug!("Sent message to Redis");
        }

        // Sleep for 15 seconds minus the time this loop took, with some protections to make sure we
        // don't have a negative duration
        let loop_time = Instant::now() - start;
        if loop_time > Duration::from_secs(14) {
            warn!(?loop_time, "Long loop");
        }
        let sleep_duration = Duration::from_secs(15) - min(Duration::from_secs(14), loop_time);
        debug!(?sleep_duration, "Sleeping");
        sleep(sleep_duration).await;
    }
}

async fn fetch_datafeed() -> Result<DatafeedRoot, reqwest::Error> {
    reqwest::get(datafeed_url(VnasEnvironment::Live))
        .await?
        .json::<DatafeedRoot>()
        .await
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
