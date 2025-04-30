use crate::analysis::transaction_analysis::TransactionAnalysisData;
use crate::heuristic_engine::HeuristicFlags;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The categorical risk level assigned to a wallet or transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskCategory {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            RiskCategory::Low => write!(f, "Low"),
            RiskCategory::Medium => write!(f, "Medium"),
            RiskCategory::High => write!(f, "High"),
            RiskCategory::Critical => write!(f, "Critical"),
        }
    }
}

/// Represents a comprehensive risk assessment of a transaction or wallet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskScore {
    /// Numerical score representing overall risk level (0.0 - 10.0)
    pub numerical_score: f32,

    /// Categorical risk level derived from the numerical score
    pub category: RiskCategory,

    /// Descriptions of triggered risk factors contributing to the score
    pub risk_factors: Vec<String>,

    /// Detailed breakdown of score components and their weights
    pub score_components: HashMap<String, f32>,

    /// Flag indicating if any critical risk factors were triggered
    /// This can override the numerical score to set the category to Critical
    pub has_critical_flags: bool,
}

impl Default for RiskScore {
    fn default() -> Self {
        Self {
            numerical_score: 0.0,
            category: RiskCategory::Low,
            risk_factors: Vec::new(),
            score_components: HashMap::new(),
            has_critical_flags: false,
        }
    }
}

impl RiskScore {
    /// Creates a new RiskScore with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a RiskScore from a numerical score and risk factors
    pub fn from_score_and_factors(
        score: f32,
        risk_factors: Vec<String>,
        has_critical: bool,
    ) -> Self {
        let category = if has_critical {
            RiskCategory::Critical
        } else if score >= 7.0 {
            RiskCategory::Critical
        } else if score >= 5.0 {
            RiskCategory::High
        } else if score >= 3.0 {
            RiskCategory::Medium
        } else {
            RiskCategory::Low
        };

        Self {
            numerical_score: score,
            category,
            risk_factors,
            score_components: HashMap::new(),
            has_critical_flags: has_critical,
        }
    }

    /// Adds a score component with a specific weight
    pub fn add_component(&mut self, name: &str, value: f32) {
        self.score_components.insert(name.to_string(), value);
        // Recalculate score based on all components
        self.recalculate_score();
    }

    /// Recalculates the numerical score based on all components
    fn recalculate_score(&mut self) {
        let mut total = 0.0;
        for (_, value) in &self.score_components {
            total += *value;
        }
        self.numerical_score = total.min(10.0); // Cap at 10.0

        // Update category based on new score or critical flags
        if self.has_critical_flags {
            self.category = RiskCategory::Critical;
        } else {
            self.category = if self.numerical_score >= 7.0 {
                RiskCategory::Critical
            } else if self.numerical_score >= 5.0 {
                RiskCategory::High
            } else if self.numerical_score >= 3.0 {
                RiskCategory::Medium
            } else {
                RiskCategory::Low
            };
        }
    }

    /// Returns a human-readable summary of the risk assessment
    pub fn summary(&self) -> String {
        let mut result = format!(
            "Risk Assessment: {} (Score: {:.2}/10.0)",
            self.category, self.numerical_score
        );

        if !self.risk_factors.is_empty() {
            result.push_str("\n\nRisk Factors:");
            for factor in &self.risk_factors {
                result.push_str(&format!("\n- {}", factor));
            }
        }

        result
    }
}

/// Calculates a risk score for a transaction based on heuristic flags and analysis data
///
/// This function applies configurable weights to each heuristic flag and combines them
/// to generate a final risk score for the transaction.
///
/// # Arguments
///
/// * `analysis_data` - The transaction analysis data containing wallet and program information
/// * `heuristic_flags` - The flags containing the results of heuristic checks
///
/// # Returns
///
/// A `RiskScore` structure containing the numerical score, risk category, and triggered factors
pub fn calculate_transaction_risk(
    analysis_data: &TransactionAnalysisData,
    heuristic_flags: &HeuristicFlags,
    config: Option<&crate::heuristic_engine::config::HeuristicWeightsConfig>,
) -> RiskScore {
    // Get the configured weights for various heuristic flags
    let weights = match config {
        Some(config) => {
            // Use provided weights
            let mut weights_map = std::collections::HashMap::new();
            for (key, value) in &config.weights {
                weights_map.insert(key.as_str(), *value);
            }
            weights_map
        }
        None => {
            // Use default weights
            get_heuristic_weights()
        }
    };

    let mut risk_score = RiskScore::new();
    let mut has_critical_flags = false;

    // Apply weights for boolean flags
    if heuristic_flags.is_high_frequency {
        risk_score.add_component(
            "high_frequency",
            weights.get("high_frequency").unwrap_or(&1.0).clone(),
        );
    }

    if heuristic_flags.direct_illicit_interaction {
        risk_score.add_component(
            "direct_illicit_interaction",
            weights
                .get("direct_illicit_interaction")
                .unwrap_or(&2.0)
                .clone(),
        );
        has_critical_flags = true; // Direct illicit interaction is considered critical
    }

    if heuristic_flags.is_pass_through {
        risk_score.add_component(
            "pass_through",
            weights.get("pass_through").unwrap_or(&1.0).clone(),
        );
    }

    if heuristic_flags.is_new_wallet {
        risk_score.add_component(
            "new_wallet",
            weights.get("new_wallet").unwrap_or(&0.5).clone(),
        );
    }

    if heuristic_flags.rapid_dispersal_pattern {
        risk_score.add_component(
            "rapid_dispersal",
            weights.get("rapid_dispersal").unwrap_or(&1.0).clone(),
        );
    }

    if heuristic_flags.fund_consolidation_pattern {
        risk_score.add_component(
            "fund_consolidation",
            weights.get("fund_consolidation").unwrap_or(&1.0).clone(),
        );
    }

    // Apply weights for ratio/score flags, scaling by the value
    let structuring_weight =
        weights.get("structuring").unwrap_or(&1.0) * heuristic_flags.structuring_score;
    if structuring_weight > 0.0 {
        risk_score.add_component("structuring", structuring_weight);
    }

    let risky_category_weight = weights.get("risky_category").unwrap_or(&1.0)
        * heuristic_flags.risky_category_interaction_score;
    if risky_category_weight > 0.0 {
        risk_score.add_component("risky_category", risky_category_weight);
    }

    let risky_funding_weight =
        weights.get("risky_funding").unwrap_or(&1.5) * heuristic_flags.risky_funding_source_ratio;
    if risky_funding_weight > 0.0 {
        risk_score.add_component("risky_funding", risky_funding_weight);
    }

    let risky_spending_weight = weights.get("risky_spending").unwrap_or(&1.5)
        * heuristic_flags.risky_spending_destination_ratio;
    if risky_spending_weight > 0.0 {
        risk_score.add_component("risky_spending", risky_spending_weight);
    }

    // Check for high-risk programs or wallets in the transaction
    // This information comes from direct DB lookups, not heuristics
    let has_high_risk_programs = analysis_data.program_analysis.iter().any(|pa| {
        pa.is_known
            && pa.address_record.as_ref().map_or(false, |record| {
                record.risk_level == "High" || record.risk_level == "Critical"
            })
    });

    let has_high_risk_wallets = analysis_data.wallet_direct_analysis.iter().any(|wa| {
        wa.is_known
            && wa.address_record.as_ref().map_or(false, |record| {
                record.risk_level == "High" || record.risk_level == "Critical"
            })
    });

    // Add weight for high-risk programs or wallets
    if has_high_risk_programs {
        risk_score.add_component(
            "high_risk_program",
            weights.get("high_risk_program").unwrap_or(&2.0).clone(),
        );
        has_critical_flags = true;
    }

    if has_high_risk_wallets {
        risk_score.add_component(
            "high_risk_wallet",
            weights.get("high_risk_wallet").unwrap_or(&2.0).clone(),
        );
        has_critical_flags = true;
    }

    // Process custom flags from the heuristic_flags
    for (flag_name, flag_value) in &heuristic_flags.custom_flags {
        if let Some(weight) = weights.get(flag_name.as_str()) {
            risk_score.add_component(flag_name, weight * flag_value);
        } else {
            // Use default weight of 1.0 for unknown custom flags
            risk_score.add_component(flag_name, flag_value * 1.0);
        }
    }

    // Get triggered flags descriptions
    let risk_factors = heuristic_flags.get_triggered_flags_description();

    // Add program and wallet risk factors if applicable
    let mut all_risk_factors = risk_factors;

    if has_high_risk_programs {
        all_risk_factors.push("Transaction involves high-risk programs".to_string());
    }

    if has_high_risk_wallets {
        all_risk_factors.push("Transaction involves high-risk wallets".to_string());
    }

    // Set risk factors and critical flag
    risk_score.risk_factors = all_risk_factors;
    risk_score.has_critical_flags = has_critical_flags;

    // Force recalculation to ensure category is updated based on critical flags
    risk_score.recalculate_score();

    risk_score
}

/// Returns a map of heuristic names to their weight factors
fn get_heuristic_weights() -> HashMap<&'static str, f32> {
    // These weights could be loaded from a config file
    let mut weights = HashMap::new();

    // Critical risk indicators (direct connection to known bad actors)
    weights.insert("direct_illicit_interaction", 2.5);
    weights.insert("high_risk_program", 2.0);
    weights.insert("high_risk_wallet", 2.0);

    // High risk indicators (suspicious fund flow patterns)
    weights.insert("risky_funding", 1.5);
    weights.insert("risky_spending", 1.5);
    weights.insert("rapid_dispersal", 1.2);
    weights.insert("fund_consolidation", 1.2);

    // Medium risk indicators (suspicious behavior patterns)
    weights.insert("structuring", 1.0);
    weights.insert("high_frequency", 1.0);
    weights.insert("pass_through", 1.0);
    weights.insert("risky_category", 1.0);

    // Low risk indicators (contextual factors)
    weights.insert("new_wallet", 0.5);

    // Custom flags from heuristics
    weights.insert("rapid_dispersal_score", 0.8);
    weights.insert("fund_consolidation_score", 0.8);

    weights
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::transaction_parser::ParsedTransaction;

    fn create_mock_heuristic_flags() -> HeuristicFlags {
        let mut flags = HeuristicFlags::default();
        flags.is_high_frequency = true;
        flags.structuring_score = 0.7;
        flags.risky_funding_source_ratio = 0.5;
        flags.direct_illicit_interaction = false;

        let mut custom_flags = HashMap::new();
        custom_flags.insert("suspicious_token_interaction".to_string(), 0.8);
        flags.custom_flags = custom_flags;

        flags
    }

    fn create_mock_analysis_data() -> TransactionAnalysisData {
        use crate::analysis::program_analyzer::ProgramAnalysis;
        use crate::analysis::wallet_analyzer::WalletDirectAnalysisResult;
        use crate::db::models::AddressRecord;
        use solana_transaction_status::UiTransactionTokenBalance;

        let parsed_tx = ParsedTransaction {
            signature: "mock_signature".to_string(),
            program_ids: vec!["program1".to_string(), "program2".to_string()],
            involved_accounts: vec!["account1".to_string(), "account2".to_string()],
            pre_token_balances: Vec::<UiTransactionTokenBalance>::new(),
            post_token_balances: Vec::<UiTransactionTokenBalance>::new(),
            execution_status: Some("confirmed".to_string()),
            fee: 5000,
        };

        let now = sqlx::types::time::OffsetDateTime::now_utc();

        let address_record1 = Some(AddressRecord {
            address: "program1".to_string(),
            entity_name: "Safe Program".to_string(),
            category: "DEX".to_string(),
            risk_level: "Low".to_string(),
            source_of_info: "Test".to_string(),
            confidence_score: 5,
            notes: Some("Test program".to_string()),
            created_at: now,
            updated_at: now,
        });

        let address_record2 = Some(AddressRecord {
            address: "program2".to_string(),
            entity_name: "Risky Program".to_string(),
            category: "Unknown".to_string(),
            risk_level: "Medium".to_string(),
            source_of_info: "Test".to_string(),
            confidence_score: 3,
            notes: Some("Test risky program".to_string()),
            created_at: now,
            updated_at: now,
        });

        let wallet_record = Some(AddressRecord {
            address: "account1".to_string(),
            entity_name: "Normal User".to_string(),
            category: "User".to_string(),
            risk_level: "Low".to_string(),
            source_of_info: "Test".to_string(),
            confidence_score: 5,
            notes: Some("Test wallet".to_string()),
            created_at: now,
            updated_at: now,
        });

        let program_analysis = vec![
            ProgramAnalysis {
                program_id: "program1".to_string(),
                is_known: true,
                address_record: address_record1,
            },
            ProgramAnalysis {
                program_id: "program2".to_string(),
                is_known: true,
                address_record: address_record2,
            },
        ];

        let wallet_analysis = vec![WalletDirectAnalysisResult {
            wallet_address: "account1".to_string(),
            is_known: true,
            address_record: wallet_record,
        }];

        TransactionAnalysisData {
            parsed_transaction: parsed_tx,
            program_analysis,
            wallet_direct_analysis: wallet_analysis,
            heuristic_flags: None,
            risk_score: None,
            risk_factors: None,
        }
    }

    #[test]
    fn test_risk_category_display() {
        assert_eq!(format!("{}", RiskCategory::Low), "Low");
        assert_eq!(format!("{}", RiskCategory::Medium), "Medium");
        assert_eq!(format!("{}", RiskCategory::High), "High");
        assert_eq!(format!("{}", RiskCategory::Critical), "Critical");
    }

    #[test]
    fn test_risk_score_from_score_and_factors() {
        let factors = vec!["Test factor 1".to_string(), "Test factor 2".to_string()];

        // Test Low category
        let low_score = RiskScore::from_score_and_factors(2.0, factors.clone(), false);
        assert_eq!(low_score.category, RiskCategory::Low);
        assert_eq!(low_score.numerical_score, 2.0);
        assert_eq!(low_score.risk_factors.len(), 2);

        // Test Medium category
        let medium_score = RiskScore::from_score_and_factors(4.0, factors.clone(), false);
        assert_eq!(medium_score.category, RiskCategory::Medium);

        // Test High category
        let high_score = RiskScore::from_score_and_factors(6.0, factors.clone(), false);
        assert_eq!(high_score.category, RiskCategory::High);

        // Test Critical category
        let critical_score = RiskScore::from_score_and_factors(8.0, factors.clone(), false);
        assert_eq!(critical_score.category, RiskCategory::Critical);

        // Test critical override with low score
        let critical_override = RiskScore::from_score_and_factors(2.0, factors, true);
        assert_eq!(critical_override.category, RiskCategory::Critical);
        assert_eq!(critical_override.numerical_score, 2.0);
        assert!(critical_override.has_critical_flags);
    }

    #[test]
    fn test_calculate_transaction_risk() {
        let mut analysis_data = create_mock_analysis_data();
        let heuristic_flags = create_mock_heuristic_flags();

        // Set heuristic flags in analysis data
        analysis_data.heuristic_flags = Some(heuristic_flags.clone());

        // Calculate risk score with no config (should use defaults)
        let risk_score = calculate_transaction_risk(&analysis_data, &heuristic_flags, None);

        // Verify basic scoring works
        assert!(risk_score.numerical_score > 0.0);
        assert!(!risk_score.risk_factors.is_empty());

        // Verify risk components exist
        assert!(risk_score.score_components.contains_key("high_frequency"));
        assert!(risk_score.score_components.contains_key("structuring"));
        assert!(risk_score.score_components.contains_key("risky_funding"));

        // Test with critical flag
        let mut critical_flags = heuristic_flags.clone();
        critical_flags.direct_illicit_interaction = true;

        let critical_risk = calculate_transaction_risk(&analysis_data, &critical_flags, None);
        assert_eq!(critical_risk.category, RiskCategory::Critical);
        assert!(critical_risk.has_critical_flags);

        // Test with custom config
        let mut weights_config = crate::heuristic_engine::config::HeuristicWeightsConfig::default();
        weights_config
            .weights
            .insert("high_frequency".to_string(), 3.0); // Increase this weight

        let custom_config_risk =
            calculate_transaction_risk(&analysis_data, &heuristic_flags, Some(&weights_config));
        // Should have higher score due to the increased weight
        assert!(custom_config_risk.numerical_score > risk_score.numerical_score);
    }

    #[test]
    fn test_risk_score_add_component() {
        let mut score = RiskScore::new();
        assert_eq!(score.numerical_score, 0.0);

        score.add_component("test1", 2.5);
        assert_eq!(score.numerical_score, 2.5);
        assert_eq!(score.category, RiskCategory::Low);

        score.add_component("test2", 1.0);
        assert_eq!(score.numerical_score, 3.5);
        assert_eq!(score.category, RiskCategory::Medium);

        score.add_component("test3", 4.0);
        assert_eq!(score.numerical_score, 7.5);
        assert_eq!(score.category, RiskCategory::Critical);

        // Test capping at 10.0
        score.add_component("test4", 5.0);
        assert_eq!(score.numerical_score, 10.0);
        assert_eq!(score.category, RiskCategory::Critical);
    }
}
