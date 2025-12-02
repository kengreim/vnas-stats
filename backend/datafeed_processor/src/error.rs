use crate::database::queries::QueryError;
use shared::error::InitializationError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcessorMainError {
    #[error("failed to initialize datafeed processor: {0}")]
    Initialization(#[from] InitializationError),
    #[error("failed to clear initial backlog of fetched datafeeds")]
    InitialBacklog(#[from] BacklogProcessingError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
}

#[derive(Debug, Error)]
pub enum BacklogProcessingError {
    #[error("query error: {0}")]
    Query(#[from] QueryError),
    #[error("payload processing error: {0}")]
    Payload(#[from] PayloadProcessingError),
    #[error("db transaction error: {0}")]
    TransactionError(#[from] sqlx::Error),
}

#[derive(Debug, Error)]
pub enum PayloadProcessingError {
    #[error("datafeed deserialization error: {0}")]
    Deserialize(#[from] serde_json::Error),
    #[error("query error: {0}")]
    Query(#[from] QueryError),
    #[error("db transaction error: {0}")]
    TransactionError(#[from] sqlx::Error),
}

#[derive(Debug, Error)]
pub enum ControllerParseError {
    #[error("invalid cid {cid}: {source}")]
    Cid {
        cid: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("invalid callsign {callsign}: {source}")]
    Callsign {
        callsign: String,
        #[source]
        source: CallsignParseError,
    },
}

#[derive(Debug, Error)]
pub enum CallsignParseError {
    #[error("callsign must have 2 or 3 parts delimited by an underscore, but found {0}")]
    IncorrectFormat(usize),
}
