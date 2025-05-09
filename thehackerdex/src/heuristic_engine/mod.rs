//! Heuristic Engine for risk detection in blockchain transactions and wallets
//!
//! This module provides functionality for detecting various risk patterns
//! in blockchain transactions and wallets, including high frequency trading,
//! structuring, pass-through behavior, and illicit interactions.

pub mod bidirectional_flow_heuristics;
pub mod config;
pub mod context_builder;
pub mod risk_scoring;
pub mod transaction_heuristics;
pub mod types;
pub mod wallet_heuristics;

use crate::analysis::transaction_analysis::TransactionAnalysisData;
use crate::db::Repository;
use crate::error::HackerdexError;
use crate::rpc::client::RateLimitedClient; // Updated import
use tracing;

// Re-export main types for easier access
pub use config::HeuristicWeightsConfig;
pub use risk_scoring::{RiskCategory, RiskScore};
pub use types::{HeuristicFlags, WalletContext, WalletTransaction};

/// Runs all heuristic checks on the provided transaction analysis data and wallet context
///
/// This function orchestrates all the individual heuristic functions and combines their
/// results into a single `HeuristicFlags` structure.
///
/// # Arguments
///
/// * `analysis_data` - The transaction analysis data containing wallet and program information
/// * `wallet_context` - The wallet context for the wallet being analyzed
///
/// # Returns
///
/// A `HeuristicFlags` structure containing the results of all heuristic checks
pub fn run_all_heuristics(
    analysis_data: &TransactionAnalysisData,
    wallet_context: &WalletContext,
) -> HeuristicFlags {
    tracing::debug!(
        "Running all heuristics for wallet {}",
        wallet_context.address
    );

    let mut flags = HeuristicFlags::default();

    // Check if the wallet is new and potentially suspicious based on its age and activity
    let (suspicious_new_wallet, _new_wallet_score) =
        wallet_heuristics::check_wallet_age_and_activity(wallet_context);
    flags.is_new_wallet = suspicious_new_wallet;

    // Check for high frequency trading patterns
    let (is_high_freq, _score) = wallet_heuristics::check_high_frequency_volume(wallet_context);
    flags.is_high_frequency = is_high_freq;

    // Check for structuring patterns
    flags.structuring_score = wallet_heuristics::check_structuring_patterns(wallet_context);

    // Check for pass-through behavior
    let (is_pass_through, _score) = wallet_heuristics::check_pass_through(wallet_context);
    flags.is_pass_through = is_pass_through;

    // Check for direct illicit interactions
    flags.direct_illicit_interaction =
        transaction_heuristics::check_direct_illicit_interaction(analysis_data);

    // Check for interactions with risky categories
    flags.risky_category_interaction_score =
        transaction_heuristics::check_interaction_with_risky_categories(analysis_data) as f32;

    // Analyze funding sources for risks
    flags.risky_funding_source_ratio = wallet_heuristics::analyze_funding_sources(wallet_context);

    // Analyze spending destinations for risks
    flags.risky_spending_destination_ratio =
        wallet_heuristics::analyze_spending_destinations(wallet_context);

    // Check for rapid dispersal patterns (one to many)
    let (has_rapid_dispersal, rapid_dispersal_score) =
        transaction_heuristics::check_rapid_dispersal(analysis_data, wallet_context);
    flags.rapid_dispersal_pattern = has_rapid_dispersal;

    // Check for fund consolidation patterns (many to one)
    let (has_fund_consolidation, fund_consolidation_score) =
        transaction_heuristics::check_fund_consolidation(analysis_data, wallet_context);
    flags.fund_consolidation_pattern = has_fund_consolidation;

    // Check for bidirectional fund flow patterns between counterparties
    let (has_bidirectional_flow, bidirectional_flow_score) =
        bidirectional_flow_heuristics::check_bidirectional_flow(wallet_context);
    flags.bidirectional_flow_pattern = has_bidirectional_flow;

    // If any flow pattern was detected, add a custom flag with the score
    if has_rapid_dispersal {
        flags.custom_flags.insert(
            "rapid_dispersal_score".to_string(),
            rapid_dispersal_score as f32,
        );
    }

    if has_fund_consolidation {
        flags.custom_flags.insert(
            "fund_consolidation_score".to_string(),
            fund_consolidation_score as f32,
        );
    }

    if has_bidirectional_flow {
        flags.custom_flags.insert(
            "bidirectional_flow_score".to_string(),
            bidirectional_flow_score as f32,
        );
    }

    // Log detailed information if the wallet shows significant risk
    if flags.get_overall_suspicion_score() > 3.0 {
        tracing::info!(
            "Significant risk detected for wallet {}. Funding risk: {:.2}, Spending risk: {:.2}",
            wallet_context.address,
            flags.risky_funding_source_ratio,
            flags.risky_spending_destination_ratio
        );
    }

    tracing::info!(
        "Completed heuristics for wallet {}, suspicion score: {:.2}",
        wallet_context.address,
        flags.get_overall_suspicion_score()
    );

    flags
}

/// Runs heuristics on the provided transaction analysis data
///
/// This function handles fetching necessary wallet contexts for all involved wallets,
/// running the heuristic checks, and updating the analysis data with the results.
/// It respects RPC rate limits by using the context_builder module.
///
/// # Arguments
///
/// * `analysis_data` - The transaction analysis data to run heuristics on
/// * `repo` - Database repository for querying known address information
/// * `rpc_client` - Rate-limited RPC client for fetching on-chain data // Updated comment
///
/// # Returns
///
/// A Result containing the updated TransactionAnalysisData with heuristic results
pub async fn run_heuristics(
    mut analysis_data: TransactionAnalysisData,
    repo: &Repository,
    rpc_client: &RateLimitedClient, // Changed type to RateLimitedClient
) -> Result<TransactionAnalysisData, HackerdexError> {
    tracing::info!(
        "Running heuristics for transaction {}",
        analysis_data.parsed_transaction.signature
    );

    // Extract unique wallet addresses from the transaction
    let mut wallet_addresses: Vec<String> = Vec::new();

    // Add all involved accounts from the transaction as potential wallet addresses
    wallet_addresses.extend(analysis_data.parsed_transaction.involved_accounts.clone());

    if wallet_addresses.is_empty() {
        return Err(HackerdexError::AnalysisError(
            "No wallet addresses found for heuristic analysis".into(),
        ));
    }

    tracing::debug!("Building context for {} wallets", wallet_addresses.len());

    // Build wallet contexts for all addresses while respecting rate limits
    let wallet_contexts =
        context_builder::build_wallet_contexts(&wallet_addresses, repo, rpc_client).await?;

    if wallet_contexts.is_empty() {
        return Err(HackerdexError::AnalysisError(
            "Failed to build wallet contexts".into(),
        ));
    }

    // Find the main wallet context (typically the fee payer or first in the list)
    let main_context = wallet_contexts.first();

    if let Some(main_context) = main_context {
        // Run heuristics on the main wallet context
        let heuristic_flags = run_all_heuristics(&analysis_data, main_context);

        // Update the analysis data with the heuristic results
        analysis_data.heuristic_flags = Some(heuristic_flags.clone());

        // Get the configured weights
        let weights_config = crate::heuristic_engine::config::HeuristicWeightsConfig::load(
            &crate::config::Config::from_env(),
        );

        // Calculate risk score based on heuristic flags and analysis data
        let risk_score = risk_scoring::calculate_transaction_risk(
            &analysis_data,
            &heuristic_flags,
            Some(&weights_config),
        );

        // Update the analysis data with the risk score
        analysis_data.risk_score = Some(risk_score.clone());

        // Add risk factors based on triggered flags
        if !risk_score.risk_factors.is_empty() {
            analysis_data.risk_factors = Some(risk_score.risk_factors.clone());

            tracing::info!(
                "Identified {} risk factors for transaction {} (Score: {:.2}, Category: {})",
                risk_score.risk_factors.len(),
                analysis_data.parsed_transaction.signature,
                risk_score.numerical_score,
                risk_score.category
            );
        }
    } else {
        return Err(HackerdexError::AnalysisError(
            "No main wallet context found for heuristic analysis".into(),
        ));
    }

    Ok(analysis_data)
}
