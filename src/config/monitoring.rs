use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::error::HackerdexError;

/// Monitoring strategy options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitoringStrategy {
    /// Poll getSignaturesForAddress periodically
    Polling,
    /// Use WebSocket logsSubscribe with filters
    WebSocket,
}

impl Default for MonitoringStrategy {
    fn default() -> Self {
        // Start with polling as specified in the requirements
        MonitoringStrategy::Polling
    }
}

/// Configuration for wallet monitoring system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// List of program IDs to monitor
    pub target_programs: HashSet<String>,

    /// List of high-risk addresses to watch interactions for
    pub watch_addresses: HashSet<String>,

    /// Monitoring strategy configuration
    #[serde(default)]
    pub strategy: MonitoringStrategy,

    /// Polling interval in seconds (used when strategy is Polling)
    #[serde(default = "default_polling_interval")]
    pub polling_interval_seconds: u64,

    /// WebSocket connection parameters (used when strategy is WebSocket)
    #[serde(default)]
    pub websocket_params: WebSocketParams,

    /// Critical alert criteria
    pub critical_criteria: CriticalCriteria,

    /// Configuration for automatically added wallets
    pub auto_added_wallets: AutoAddedWalletConfig,
}

/// Default polling interval (30 seconds)
fn default_polling_interval() -> u64 {
    30
}

/// WebSocket connection parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketParams {
    /// Maximum number of reconnect attempts
    #[serde(default = "default_max_reconnects")]
    pub max_reconnects: u32,

    /// Reconnection backoff in seconds
    #[serde(default = "default_reconnect_backoff")]
    pub reconnect_backoff_seconds: u64,

    /// Subscription batch size
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

impl Default for WebSocketParams {
    fn default() -> Self {
        Self {
            max_reconnects: default_max_reconnects(),
            reconnect_backoff_seconds: default_reconnect_backoff(),
            batch_size: default_batch_size(),
        }
    }
}

fn default_max_reconnects() -> u32 {
    5
}

fn default_reconnect_backoff() -> u64 {
    5
}

fn default_batch_size() -> usize {
    100
}

/// Criteria that define when a transaction or interaction is critical
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalCriteria {
    /// Minimum risk score to consider critical
    pub risk_score_threshold: u32,

    /// List of heuristic flags that indicate critical status
    pub required_flags: Vec<String>,
}

/// Configuration for automatically added wallets during monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoAddedWalletConfig {
    /// Default category for auto-added wallets
    pub default_category: String,

    /// Default risk level (1-5) for auto-added wallets
    pub default_risk_level: u8,

    /// Source note template (will be stored in DB)
    pub source_note: String,
}

impl Default for AutoAddedWalletConfig {
    fn default() -> Self {
        Self {
            default_category: "monitored".to_string(),
            default_risk_level: 3,
            source_note: "Automated Monitor v1.0".to_string(),
        }
    }
}

impl MonitoringConfig {
    /// Load monitoring configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, HackerdexError> {
        let content = fs::read_to_string(path).map_err(|e| {
            HackerdexError::ConfigError(format!("Failed to read monitoring config: {}", e))
        })?;

        let config: MonitoringConfig = toml::from_str(&content).map_err(|e| {
            HackerdexError::ConfigError(format!("Failed to parse monitoring config: {}", e))
        })?;

        Ok(config)
    }

    /// Create a new monitoring configuration with default values
    pub fn default() -> Self {
        let mut required_flags = Vec::new();
        required_flags.push("direct_illicit_interaction".to_string());
        required_flags.push("suspicious_approval".to_string());
        required_flags.push("total_balance_sweep".to_string());

        Self {
            target_programs: HashSet::new(),
            watch_addresses: HashSet::new(),
            strategy: MonitoringStrategy::default(),
            polling_interval_seconds: default_polling_interval(),
            websocket_params: WebSocketParams::default(),
            critical_criteria: CriticalCriteria {
                risk_score_threshold: 80,
                required_flags,
            },
            auto_added_wallets: AutoAddedWalletConfig::default(),
        }
    }

    /// Get polling interval as Duration
    pub fn polling_interval(&self) -> Duration {
        Duration::from_secs(self.polling_interval_seconds)
    }
}
