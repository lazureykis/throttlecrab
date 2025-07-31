use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Connection pool error: {0}")]
    Pool(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Server returned error response")]
    ServerError,

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Timeout")]
    Timeout,

    #[error("Invalid response from server")]
    InvalidResponse,
}

pub type Result<T> = std::result::Result<T, ClientError>;
