use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub server_address: String,
    #[allow(dead_code)] // Field might be used in the future or by other parts of the application
    pub solana_rpc_url: String,
    // Add other configuration fields as needed, e.g., API keys, log level
    // pub chainabuse_api_key: Option<String>,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok(); // Load .env file if present

        Ok(AppConfig {
            database_url: env::var("DATABASE_URL")
                .map_err(|_| ConfigError::Missing("DATABASE_URL".to_string()))?,
            server_address: env::var("SERVER_ADDRESS")
                .unwrap_or_else(|_| "127.0.0.1:3000".to_string()),
            solana_rpc_url: env::var("SOLANA_RPC_URL")
                .map_err(|_| ConfigError::Missing("SOLANA_RPC_URL".to_string()))?,
            // chainabuse_api_key: env::var("CHAINABUSE_API_KEY").ok(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Missing configuration value: {0}")]
    Missing(String),
    // Add other configuration errors if needed
}