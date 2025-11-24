use crate::database::queries::QueryError;
use shared::error::InitializationError;

#[derive(Debug, thiserror::Error)]
pub enum ProcessorError {
    #[error("failed to initialize datafeed processor: {0}")]
    Initialization(#[from] InitializationError),
    #[error("failed to clear initial backlog of fetched datafeeds")]
    InitialBacklog(#[from] BacklogProcessingError),
}

#[derive(Debug, thiserror::Error)]
pub enum BacklogProcessingError {
    #[error("query error: {0}")]
    Query(#[from] QueryError),
    #[error("payload processing error: {0}")]
    Payload(#[from] PayloadProcessingError),
    #[error("db transaction error: {0}")]
    TransactionError(#[from] sqlx::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum PayloadProcessingError {
    #[error("datafeed deserialization error: {0}")]
    Deserialize(#[from] serde_json::Error),
    #[error("query error: {0}")]
    Query(#[from] QueryError),
    #[error("db transaction error: {0}")]
    TransactionError(#[from] sqlx::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum CallsignParseError {
    #[error("callsign must have 2 or 3 parts delimited by an underscore, but found {0}")]
    IncorrectFormat(usize),
}
