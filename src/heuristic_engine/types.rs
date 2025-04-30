use crate::db::models::AddressRecord;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents the results of various heuristic checks applied to a wallet or transaction
/// This struct stores boolean/numeric results of individual heuristics that help identify
/// potentially suspicious or risky behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeuristicFlags {
    /// Whether the wallet has shown high frequency transaction patterns
    pub is_high_frequency: bool,

    /// Score indicating possible structuring behavior (0.0 to 1.0, higher is more suspicious)
    pub structuring_score: f32,

    /// Whether the wallet appears to be a pass-through (receive and send funds immediately)
    pub is_pass_through: bool,

    /// Whether the wallet has directly interacted with known illicit addresses
    pub direct_illicit_interaction: bool,

    /// Score indicating the level of interaction with medium/high-risk categories (0.0 to 1.0)
    pub risky_category_interaction_score: f32,

    /// Whether the wallet is newly created (less than X days old)
    pub is_new_wallet: bool,

    /// Percentage of funding from risky sources (0.0 to 1.0)
    pub risky_funding_source_ratio: f32,

    /// Percentage of spending to risky destinations (0.0 to 1.0)
    pub risky_spending_destination_ratio: f32,

    /// Whether the wallet shows patterns of rapid fund dispersal (one to many)
    pub rapid_dispersal_pattern: bool,

    /// Whether the wallet shows patterns of fund consolidation (many to one)
    pub fund_consolidation_pattern: bool,

    /// Whether the wallet shows bidirectional fund flow with specific counterparties
    pub bidirectional_flow_pattern: bool,

    /// Custom heuristic flags that don't fit the predefined categories
    pub custom_flags: HashMap<String, f32>,
}

pub struct WalletContext {
    /// The wallet address being analyzed
    pub address: String,

    /// When the wallet was created (first transaction timestamp)
    pub creation_timestamp: Option<i64>,

    /// Current SOL balance of the wallet
    pub sol_balance: f64,

    /// Record from the known address database if this wallet is known
    pub known_address_record: Option<AddressRecord>,

    /// Number of transactions in the last 24 hours
    pub tx_count_24h: u32,

    /// Number of transactions in the last week
    pub tx_count_7d: u32,

    /// Total transaction volume (in SOL) in the last 24 hours
    pub volume_24h: f64,

    /// Total transaction volume (in SOL) in the last week
    pub volume_7d: f64,

    /// Recent incoming transactions (limited to a reasonable number)
    /// This includes information about where funds are coming from
    pub recent_incoming_txs: Vec<WalletTransaction>,

    /// Recent outgoing transactions (limited to a reasonable number)
    /// This includes information about where funds are going
    pub recent_outgoing_txs: Vec<WalletTransaction>,
}

pub struct WalletTransaction {
    /// Transaction signature
    pub signature: String,

    /// Timestamp of the transaction
    pub timestamp: i64,

    /// Amount transferred (in SOL)
    pub amount: f64,

    /// The counterparty wallet address (sender for incoming, receiver for outgoing)
    pub counterparty: String,

    /// Whether the counterparty is a known address in our database
    pub is_known_counterparty: bool,

    /// If known, the record for the counterparty
    pub counterparty_record: Option<AddressRecord>,
}

// Allow cloning WalletTransaction for analysis purposes
impl Clone for WalletTransaction {
    fn clone(&self) -> Self {
        Self {
            signature: self.signature.clone(),
            timestamp: self.timestamp,
            amount: self.amount,
            counterparty: self.counterparty.clone(),
            is_known_counterparty: self.is_known_counterparty,
            counterparty_record: self.counterparty_record.clone(),
        }
    }
}

impl HeuristicFlags {
    /// Creates a new instance with default values (all flags set to false/0.0)
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a score representing the overall suspiciousness based on all flags
    /// This is a simple implementation that could be refined with weighted scoring
    pub fn get_overall_suspicion_score(&self) -> f32 {
        let mut score = 0.0;

        // Add 1.0 for each boolean flag that is true
        if self.is_high_frequency {
            score += 1.0;
        }
        if self.direct_illicit_interaction {
            score += 2.0;
        } // This one is weighted higher
        if self.is_pass_through {
            score += 1.0;
        }
        if self.is_new_wallet {
            score += 0.5;
        } // Being new alone is less suspicious
        if self.rapid_dispersal_pattern {
            score += 1.0;
        }
        if self.fund_consolidation_pattern {
            score += 1.0;
        }
        if self.bidirectional_flow_pattern {
            score += 1.0;
        }

        // Add the ratio scores directly (they are already in 0.0 to 1.0 range)
        score += self.structuring_score;
        score += self.risky_category_interaction_score;
        score += self.risky_funding_source_ratio;
        score += self.risky_spending_destination_ratio;

        // Add any custom flags
        for (_, flag_value) in &self.custom_flags {
            score += flag_value;
        }

        // Normalize to a 0-10 scale
        score.min(10.0)
    }

    /// Returns a list of triggered heuristic flags with descriptions
    pub fn get_triggered_flags_description(&self) -> Vec<String> {
        let mut triggered = Vec::new();

        if self.is_high_frequency {
            triggered.push("High frequency transaction patterns detected".to_string());
        }

        if self.structuring_score > 0.5 {
            triggered.push(format!(
                "Possible structuring behavior (score: {:.2})",
                self.structuring_score
            ));
        }

        if self.is_pass_through {
            triggered.push("Pass-through wallet behavior detected".to_string());
        }

        if self.direct_illicit_interaction {
            triggered.push("Direct interaction with known illicit address".to_string());
        }

        if self.risky_category_interaction_score > 0.5 {
            triggered.push(format!(
                "High interaction with risky categories (score: {:.2})",
                self.risky_category_interaction_score
            ));
        }

        if self.is_new_wallet {
            triggered.push("Newly created wallet".to_string());
        }

        if self.risky_funding_source_ratio > 0.3 {
            triggered.push(format!(
                "{:.1}% of funds from risky sources",
                self.risky_funding_source_ratio * 100.0
            ));
        }

        if self.risky_spending_destination_ratio > 0.3 {
            triggered.push(format!(
                "{:.1}% of funds sent to risky destinations",
                self.risky_spending_destination_ratio * 100.0
            ));
        }

        if self.rapid_dispersal_pattern {
            triggered.push("Rapid fund dispersal pattern detected".to_string());
        }

        if self.fund_consolidation_pattern {
            triggered.push("Fund consolidation pattern detected".to_string());
        }

        if self.bidirectional_flow_pattern {
            triggered.push("Bidirectional fund flow pattern detected".to_string());
        }

        // Add any custom flags with high values
        for (flag_name, flag_value) in &self.custom_flags {
            if *flag_value > 0.5 {
                triggered.push(format!("Custom flag '{}': {:.2}", flag_name, flag_value));
            }
        }

        triggered
    }
}

impl WalletContext {
    /// Creates a new instance with minimal required information
    pub fn new(address: String) -> Self {
        Self {
            address,
            creation_timestamp: None,
            sol_balance: 0.0,
            known_address_record: None,
            tx_count_24h: 0,
            tx_count_7d: 0,
            volume_24h: 0.0,
            volume_7d: 0.0,
            recent_incoming_txs: Vec::new(),
            recent_outgoing_txs: Vec::new(),
        }
    }

    /// Returns true if the wallet has been active for less than the specified number of days
    pub fn is_new_wallet(&self, max_days: u32) -> bool {
        if let Some(creation_time) = self.creation_timestamp {
            // Get current time in seconds
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            // Convert days to seconds
            let max_age_seconds = (max_days as i64) * 24 * 60 * 60;

            // Check if wallet age is less than max_days
            (now - creation_time) < max_age_seconds
        } else {
            // If creation time is unknown, we can't determine if it's new
            false
        }
    }

    /// Returns true if the wallet shows high frequency trading patterns
    pub fn has_high_frequency_pattern(&self, threshold: u32) -> bool {
        self.tx_count_24h > threshold
    }
}

/// Add Default implementation for HeuristicFlags for convenience
impl Default for HeuristicFlags {
    fn default() -> Self {
        Self {
            is_high_frequency: false,
            structuring_score: 0.0,
            is_pass_through: false,
            direct_illicit_interaction: false,
            risky_category_interaction_score: 0.0,
            is_new_wallet: false,
            risky_funding_source_ratio: 0.0,
            risky_spending_destination_ratio: 0.0,
            rapid_dispersal_pattern: false,
            fund_consolidation_pattern: false,
            bidirectional_flow_pattern: false,
            custom_flags: HashMap::new(),
        }
    }
}
