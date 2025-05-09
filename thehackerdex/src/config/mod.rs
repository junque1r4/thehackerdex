pub mod feature_flags;
pub mod monitoring;

pub use feature_flags::{AnalysisFeatureFlags, FeatureFlags};
pub use monitoring::{
    AutoAddedWalletConfig, CriticalCriteria, MonitoringConfig, MonitoringStrategy, WebSocketParams,
};

use crate::error::HackerdexError;
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path};

/// Configuration for the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Solana RPC endpoint URL
    pub rpc_url: String,

    /// Database connection string
    pub database_url: String,

    /// Whether to enable debug logging
    pub debug: bool,

    /// Maximum number of retries for RPC requests
    pub max_retries: usize,

    /// OSINT crawler user agent
    pub crawler_user_agent: String,

    /// ChainAbuse API key
    pub chainabuse_api_key: Option<String>,

    /// Path to heuristic weights configuration file (TOML or JSON)
    pub heuristic_weights_path: Option<String>,

    /// Path to monitoring configuration file (TOML)
    pub monitoring_config_path: Option<String>,

    /// Path to feature flags configuration file (TOML)
    pub feature_flags_path: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            database_url: "sqlite:hackerdex.db".to_string(),
            debug: false,
            max_retries: 3,
            crawler_user_agent: "HackerDex/0.1.0".to_string(),
            chainabuse_api_key: None,
            heuristic_weights_path: Some("config/heuristic_weights.toml".to_string()),
            monitoring_config_path: Some("config/monitoring.toml".to_string()),
            feature_flags_path: Some("config/feature_flags.toml".to_string()),
        }
    }
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        // Load .env file if present
        let _ = dotenv();

        Self {
            rpc_url: env::var("SOLANA_RPC_URL")
                .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string()),

            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:hackerdex.db".to_string()),

            debug: env::var("DEBUG")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(false),

            max_retries: env::var("MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),

            crawler_user_agent: env::var("CRAWLER_USER_AGENT")
                .unwrap_or_else(|_| "HackerDex/0.1.0".to_string()),

            chainabuse_api_key: env::var("CHAINABUSE_API").ok(),

            heuristic_weights_path: env::var("HEURISTIC_WEIGHTS_PATH").ok(),

            monitoring_config_path: Some(
                env::var("MONITORING_CONFIG_PATH")
                    .unwrap_or_else(|_| "config/monitoring.toml".to_string()),
            ),

            feature_flags_path: Some(
                env::var("FEATURE_FLAGS_PATH")
                    .unwrap_or_else(|_| "config/feature_flags.toml".to_string()),
            ),
        }
    }

    /// Load monitoring configuration if path is provided
    pub fn load_monitoring_config(&self) -> Result<Option<MonitoringConfig>, HackerdexError> {
        if let Some(path) = &self.monitoring_config_path {
            let config = MonitoringConfig::from_file(path)?;
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }

    /// Load feature flags configuration if path is provided
    pub fn load_feature_flags(&self) -> Result<FeatureFlags, HackerdexError> {
        if let Some(path) = &self.feature_flags_path {
            // Create default file if it doesn't exist
            if !Path::new(path).exists() {
                let default_flags = FeatureFlags::default();
                default_flags.save_to_file(path)?;
            }

            // Load configuration
            FeatureFlags::from_file(path)
        } else {
            // Use default values if no path is specified
            Ok(FeatureFlags::default())
        }
    }
}
