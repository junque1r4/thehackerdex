use crate::error::HackerdexError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Configuration for feature flags to enable/disable certain analyses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Analysis feature flags
    pub analysis_features: AnalysisFeatureFlags,
}

/// Feature flags for different analysis types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisFeatureFlags {
    /// Whether Peel Chain Exfiltration analysis is enabled
    #[serde(default = "default_true")]
    pub peel_chain_exfiltration_enabled: bool,

    /// Whether Fund Churning analysis is enabled
    #[serde(default = "default_true")]
    pub fund_churning_enabled: bool,

    /// Whether Bridge Hopping analysis is enabled
    #[serde(default = "default_true")]
    pub bridge_hopping_enabled: bool,

    /// Whether Structuring analysis is enabled
    #[serde(default = "default_true")]
    pub structuring_enabled: bool,

    /// Whether Mixer Interaction analysis is enabled
    #[serde(default = "default_true")]
    pub mixer_interaction_enabled: bool,

    /// Whether Drainer Consolidation analysis is enabled
    #[serde(default = "default_true")]
    pub drainer_consolidation_enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            analysis_features: AnalysisFeatureFlags::default(),
        }
    }
}

impl Default for AnalysisFeatureFlags {
    fn default() -> Self {
        Self {
            peel_chain_exfiltration_enabled: true,
            fund_churning_enabled: true,
            bridge_hopping_enabled: true,
            structuring_enabled: true,
            mixer_interaction_enabled: true,
            drainer_consolidation_enabled: true,
        }
    }
}

impl FeatureFlags {
    /// Load feature flags from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, HackerdexError> {
        let contents = fs::read_to_string(path).map_err(|e| {
            HackerdexError::ConfigError(format!("Failed to read feature flags file: {}", e))
        })?;

        toml::from_str(&contents).map_err(|e| {
            HackerdexError::ConfigError(format!("Failed to parse feature flags TOML: {}", e))
        })
    }

    /// Save feature flags to a TOML file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), HackerdexError> {
        let contents = toml::to_string_pretty(self).map_err(|e| {
            HackerdexError::ConfigError(format!("Failed to serialize feature flags: {}", e))
        })?;

        fs::write(path, contents).map_err(|e| {
            HackerdexError::ConfigError(format!("Failed to write feature flags file: {}", e))
        })?;

        Ok(())
    }
}
