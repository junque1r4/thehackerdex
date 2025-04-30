use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Configuration for heuristic weights in risk scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeuristicWeightsConfig {
    /// Mapping of heuristic name to its weight in risk calculation
    pub weights: HashMap<String, f32>,
}

impl Default for HeuristicWeightsConfig {
    fn default() -> Self {
        let mut weights = HashMap::new();

        // Critical risk indicators (direct connection to known bad actors)
        weights.insert("direct_illicit_interaction".to_string(), 2.5);
        weights.insert("high_risk_program".to_string(), 2.0);
        weights.insert("high_risk_wallet".to_string(), 2.0);

        // High risk indicators (suspicious fund flow patterns)
        weights.insert("risky_funding".to_string(), 1.5);
        weights.insert("risky_spending".to_string(), 1.5);
        weights.insert("rapid_dispersal".to_string(), 1.2);
        weights.insert("fund_consolidation".to_string(), 1.2);

        // Medium risk indicators (suspicious behavior patterns)
        weights.insert("structuring".to_string(), 1.0);
        weights.insert("high_frequency".to_string(), 1.0);
        weights.insert("pass_through".to_string(), 1.0);
        weights.insert("risky_category".to_string(), 1.0);

        // Low risk indicators (contextual factors)
        weights.insert("new_wallet".to_string(), 0.5);

        // Custom flags from heuristics
        weights.insert("rapid_dispersal_score".to_string(), 0.8);
        weights.insert("fund_consolidation_score".to_string(), 0.8);

        Self { weights }
    }
}

impl HeuristicWeightsConfig {
    /// Load heuristic weights configuration from a TOML file
    pub fn from_toml(path: &Path) -> Result<Self, std::io::Error> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let config: HeuristicWeightsConfig = toml::from_str(&contents)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(config)
    }

    /// Load heuristic weights configuration from a JSON file
    pub fn from_json(path: &Path) -> Result<Self, std::io::Error> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let config: HeuristicWeightsConfig = serde_json::from_str(&contents)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(config)
    }

    /// Load heuristic weights from config or use defaults
    ///
    /// First tries to load from the path specified in app_config.heuristic_weights_path
    /// If that fails, falls back to default weights
    pub fn load(config: &Config) -> Self {
        if let Some(path) = &config.heuristic_weights_path {
            let path = Path::new(path);

            // Try to load based on file extension
            if let Some(extension) = path.extension() {
                if extension == "toml" || extension == "tml" {
                    if let Ok(weights) = Self::from_toml(path) {
                        return weights;
                    }
                } else if extension == "json" {
                    if let Ok(weights) = Self::from_json(path) {
                        return weights;
                    }
                }
            }

            // If we couldn't load, log warning and use defaults
            eprintln!(
                "Warning: Failed to load heuristic weights from {}. Using defaults.",
                path.display()
            );
        }

        // Use defaults
        Self::default()
    }

    /// Get weight for a specific heuristic
    pub fn get_weight(&self, heuristic: &str) -> f32 {
        *self.weights.get(heuristic).unwrap_or(&1.0)
    }
}
