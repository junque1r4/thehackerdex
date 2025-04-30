use thiserror::Error;

/// Custom error types for the HackerDex application
#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum HackerdexError {
    /// Error occurred during Solana RPC client operations
    #[error("RPC error: {0}")]
    RpcError(String),

    /// Invalid Solana address format
    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Database operation error
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// Entity not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Data parsing error
    #[error("Data parsing error: {0}")]
    DataParsing(String),

    /// Analysis error
    #[error("Analysis error: {0}")]
    AnalysisError(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error when a more specific error type doesn't apply
    #[error("Error: {0}")]
    Other(String),
}

/// Custom Result type for the HackerDex application
#[allow(dead_code)]
pub type HackerdexResult<T> = Result<T, HackerdexError>;

/// Convert solana_client::client_error::ClientError to HackerdexError
impl From<solana_client::client_error::ClientError> for HackerdexError {
    fn from(error: solana_client::client_error::ClientError) -> Self {
        // Parse the error message to provide more specific errors
        let error_msg = error.to_string();

        if error_msg.contains("rate limit") {
            HackerdexError::RateLimit(error_msg)
        } else if error_msg.contains("AccountNotFound") || error_msg.contains("not found") {
            HackerdexError::RpcError(format!("Account not found: {}", error_msg))
        } else if error_msg.contains("Invalid") && error_msg.contains("address") {
            HackerdexError::InvalidAddress(error_msg)
        } else {
            HackerdexError::RpcError(error_msg)
        }
    }
}

/// Convert sqlx::Error to HackerdexError
impl From<sqlx::Error> for HackerdexError {
    fn from(error: sqlx::Error) -> Self {
        match error {
            sqlx::Error::RowNotFound => HackerdexError::NotFound("Record not found".to_string()),
            _ => HackerdexError::DatabaseError(error.to_string()),
        }
    }
}
