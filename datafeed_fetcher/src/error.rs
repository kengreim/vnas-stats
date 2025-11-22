use thiserror::Error;
use tracing::dispatcher::SetGlobalDefaultError;

#[derive(Error, Debug)]
pub enum FetchError {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Deserialize(#[from] serde_json::Error),
    #[error(transparent)]
    TimestampDeserialize(#[from] chrono::format::ParseError),
    #[error("unable to find or parse updatedAt field in JSON")]
    MissingUpdatedAt,
}

#[derive(Debug, Error)]
pub enum EnqueueError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("datafeed fetch error: {0}")]
    Fetch(#[from] FetchError),
}
