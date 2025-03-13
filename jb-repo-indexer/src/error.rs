use thiserror::Error;

#[derive(Debug, Error)]
pub enum IndexerError {
    #[error("an I/O error occurred: {0}")]
    GenericIo(#[from] std::io::Error),

    #[error("database error: {0}")]
    DatabaseError(#[from] libsql::Error),

    #[error("http client error: {0}")]
    HttpClientError(#[from] reqwest::Error),

    #[error("deserialization error: {0}")]
    DeserializeError(#[from] serde::de::value::Error),

    #[error("deserialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("bad base64 data: {0}")]
    InvalidBase64(#[from] base64::DecodeError),

    #[error("not found")]
    NotFound,
}