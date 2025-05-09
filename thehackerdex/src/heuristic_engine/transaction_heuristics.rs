use crate::analysis::transaction_analysis::TransactionAnalysisData;
use crate::heuristic_engine::types::WalletContext;
use std::collections::HashMap;
use tracing;

pub fn check_direct_illicit_interaction(analysis_data: &TransactionAnalysisData) -> bool {
    // Constants
    const HIGH_RISK_LEVELS: [&str; 2] = ["High", "Critical"];

    // Check programs first
    for program_analysis in &analysis_data.program_analysis {
        if let Some(address_record) = &program_analysis.address_record {
            if HIGH_RISK_LEVELS.contains(&address_record.risk_level.as_str()) {
                // Found a high-risk program
                tracing::info!(
                    "High risk program detected: {} ({}), risk level: {}",
                    address_record.entity_name,
                    program_analysis.program_id,
                    address_record.risk_level
                );
                return true;
            }
        }
    }

    // Then check wallets
    for wallet_analysis in &analysis_data.wallet_direct_analysis {
        if let Some(address_record) = &wallet_analysis.address_record {
            if HIGH_RISK_LEVELS.contains(&address_record.risk_level.as_str()) {
                // Found a high-risk wallet
                tracing::info!(
                    "High risk wallet detected: {} ({}), risk level: {}",
                    address_record.entity_name,
                    wallet_analysis.wallet_address,
                    address_record.risk_level
                );
                return true;
            }
        }
    }

    false
}

pub fn check_interaction_with_risky_categories(analysis_data: &TransactionAnalysisData) -> f64 {
    // Constants
    const HIGH_RISK_CATEGORIES: [&str; 6] = [
        "High-Risk Gambling DApp",
        "Privacy Service",
        "Mixer Feeder Address",
        "Suspicious Exchange",
        "P2P Platform Hot Wallet",
        "Unverified Service",
    ];

    const MEDIUM_RISK_CATEGORIES: [&str; 4] = [
        "Gambling DApp",
        "Peer-to-peer Exchange",
        "Unregulated Exchange",
        "Anonymous Service",
    ];

    // Weight factors
    const HIGH_RISK_CATEGORY_WEIGHT: f64 = 0.4;
    const MEDIUM_RISK_CATEGORY_WEIGHT: f64 = 0.2;
    const CONFIDENCE_FACTOR: f64 = 0.1; // Additional weight per confidence point (1-5)

    let mut total_score = 0.0;
    let mut interactions_found = 0;

    // Check programs
    for program_analysis in &analysis_data.program_analysis {
        if let Some(address_record) = &program_analysis.address_record {
            if HIGH_RISK_CATEGORIES.contains(&address_record.category.as_str()) {
                total_score += HIGH_RISK_CATEGORY_WEIGHT;
                total_score += CONFIDENCE_FACTOR * address_record.confidence_score as f64;
                interactions_found += 1;

                tracing::debug!(
                    "High risk category program: {} ({}), category: {}, confidence: {}",
                    address_record.entity_name,
                    program_analysis.program_id,
                    address_record.category,
                    address_record.confidence_score
                );
            } else if MEDIUM_RISK_CATEGORIES.contains(&address_record.category.as_str()) {
                total_score += MEDIUM_RISK_CATEGORY_WEIGHT;
                total_score += CONFIDENCE_FACTOR * address_record.confidence_score as f64 * 0.5;
                interactions_found += 1;

                tracing::debug!(
                    "Medium risk category program: {} ({}), category: {}, confidence: {}",
                    address_record.entity_name,
                    program_analysis.program_id,
                    address_record.category,
                    address_record.confidence_score
                );
            }
        }
    }

    // Check wallets
    for wallet_analysis in &analysis_data.wallet_direct_analysis {
        if let Some(address_record) = &wallet_analysis.address_record {
            if HIGH_RISK_CATEGORIES.contains(&address_record.category.as_str()) {
                total_score += HIGH_RISK_CATEGORY_WEIGHT;
                total_score += CONFIDENCE_FACTOR * address_record.confidence_score as f64;
                interactions_found += 1;

                tracing::debug!(
                    "High risk category wallet: {} ({}), category: {}, confidence: {}",
                    address_record.entity_name,
                    wallet_analysis.wallet_address,
                    address_record.category,
                    address_record.confidence_score
                );
            } else if MEDIUM_RISK_CATEGORIES.contains(&address_record.category.as_str()) {
                total_score += MEDIUM_RISK_CATEGORY_WEIGHT;
                total_score += CONFIDENCE_FACTOR * address_record.confidence_score as f64 * 0.5;
                interactions_found += 1;

                tracing::debug!(
                    "Medium risk category wallet: {} ({}), category: {}, confidence: {}",
                    address_record.entity_name,
                    wallet_analysis.wallet_address,
                    address_record.category,
                    address_record.confidence_score
                );
            }
        }
    }

    // Normalize score to a 0.0 - 1.0 range
    // If we have multiple risky interactions, cap at 1.0
    if interactions_found > 0 {
        total_score = total_score.min(1.0);
    }

    total_score
}

/// Detects a pattern where a wallet receives funds and immediately disperses them to multiple other addresses
///
/// This function identifies rapid fund dispersal patterns, which are common in money laundering and
/// certain types of scams. A typical pattern involves a wallet receiving a significant amount and
/// then quickly sending smaller amounts to multiple other wallets to obscure the flow of funds.
///
/// # Arguments
///
/// * `_transaction_data` - Analysis data for the transaction (currently unused but kept for future enhancements)
/// * `wallet_context` - Context information for the wallet being analyzed
///
/// # Returns
///
/// A tuple containing a boolean indicating if rapid dispersal was detected and a score representing
/// the severity (0.0 to 1.0)
pub fn check_rapid_dispersal(
    _transaction_data: &TransactionAnalysisData,
    wallet_context: &WalletContext,
) -> (bool, f32) {
    // Constants
    const DISPERSAL_TIME_THRESHOLD: i64 = 3600; // 1 hour in seconds
    const MINIMUM_RECIPIENTS: usize = 3; // At least this many recipients to be considered dispersal
    const SUSPICIOUS_RECIPIENTS: usize = 5; // More recipients = more suspicious
    const MIN_AMOUNT_THRESHOLD: f64 = 0.1; // Minimum SOL amount to consider for dispersal pattern

    // Track how much has been dispersed recently
    let mut total_dispersed = 0.0;

    // Number of unique recipients in recent outgoing transactions
    let mut recent_recipients = std::collections::HashSet::new();

    // Check for any recent incoming transactions followed by multiple outgoing ones
    if wallet_context.recent_incoming_txs.is_empty()
        || wallet_context.recent_outgoing_txs.is_empty()
    {
        // Not enough transaction history to determine dispersal
        return (false, 0.0);
    }

    // Find the most recent significant incoming transaction
    if let Some(latest_significant_incoming) = wallet_context
        .recent_incoming_txs
        .iter()
        .filter(|tx| tx.amount >= MIN_AMOUNT_THRESHOLD)
        .max_by_key(|tx| tx.timestamp)
    {
        // Check for outgoing transactions after this incoming one
        let outgoing_after_receipt = wallet_context
            .recent_outgoing_txs
            .iter()
            .filter(|tx| {
                tx.timestamp > latest_significant_incoming.timestamp
                    && tx.timestamp - latest_significant_incoming.timestamp
                        <= DISPERSAL_TIME_THRESHOLD
            })
            .collect::<Vec<_>>();

        // Add recipients to our tracking set
        for tx in &outgoing_after_receipt {
            recent_recipients.insert(tx.counterparty.clone());
            total_dispersed += tx.amount;
        }

        // Calculate what percentage of the incoming amount was dispersed
        let dispersal_percentage = if latest_significant_incoming.amount > 0.0 {
            total_dispersed / latest_significant_incoming.amount
        } else {
            0.0
        };

        // Log suspicious activity
        if recent_recipients.len() >= MINIMUM_RECIPIENTS && dispersal_percentage > 0.7 {
            tracing::info!(
                "Rapid dispersal detected for wallet {}: dispersed {:.2} SOL to {} recipients within {} seconds of receiving {:.2} SOL",
                wallet_context.address,
                total_dispersed,
                recent_recipients.len(),
                outgoing_after_receipt
                    .iter()
                    .map(|tx| tx.timestamp - latest_significant_incoming.timestamp)
                    .min()
                    .unwrap_or(0),
                latest_significant_incoming.amount
            );

            // Calculate a score based on the number of recipients and dispersal percentage
            let recipient_factor = (recent_recipients.len().min(SUSPICIOUS_RECIPIENTS) as f32)
                / (SUSPICIOUS_RECIPIENTS as f32);
            let dispersal_factor = dispersal_percentage as f32;
            let score = (recipient_factor + dispersal_factor) / 2.0;

            return (true, score);
        }
    }

    (false, 0.0)
}

/// Detects a pattern where a wallet receives funds from multiple suspicious sources in a short period
///
/// This function identifies fund consolidation patterns, which might indicate a collector of stolen funds
/// or a centralized point in a layering scheme. The function examines recent incoming transactions for
/// a wallet and checks if they originate from multiple suspicious sources within a short time period.
///
/// # Arguments
///
/// * `_transaction_data` - Analysis data for the transaction (currently unused but kept for future enhancements)
/// * `wallet_context` - Context information for the wallet being analyzed
///
/// # Returns
///
/// A tuple containing a boolean indicating if fund consolidation was detected and a score representing
/// the severity (0.0 to 1.0)
pub fn check_fund_consolidation(
    _transaction_data: &TransactionAnalysisData,
    wallet_context: &WalletContext,
) -> (bool, f32) {
    // Constants
    const CONSOLIDATION_TIME_WINDOW: i64 = 86400; // 24 hours in seconds
    const MINIMUM_SOURCES: usize = 3; // At least this many sources to be considered consolidation
    const SUSPICIOUS_SOURCES: usize = 5; // More sources = more suspicious
    const MIN_AMOUNT_THRESHOLD: f64 = 0.05; // Minimum SOL amount per source

    // We'll need to track sources by their timestamp to group them by time window
    let mut sources_by_time: HashMap<i64, Vec<&crate::heuristic_engine::types::WalletTransaction>> =
        HashMap::new();
    let mut total_suspicious_amount = 0.0;
    let mut risky_source_count = 0;

    // Check if there are enough incoming transactions to analyze
    if wallet_context.recent_incoming_txs.len() < MINIMUM_SOURCES {
        return (false, 0.0);
    }

    // Group sources by time windows (daily buckets for simplicity)
    for tx in &wallet_context.recent_incoming_txs {
        // Only consider transactions above the minimum threshold
        if tx.amount < MIN_AMOUNT_THRESHOLD {
            continue;
        }

        // Group by day (86400 seconds)
        let day_bucket = tx.timestamp / CONSOLIDATION_TIME_WINDOW;

        sources_by_time
            .entry(day_bucket)
            .or_insert_with(Vec::new)
            .push(tx);
    }

    // Find the time window with the most sources
    if let Some((_, sources)) = sources_by_time
        .iter()
        .max_by_key(|(_, sources)| sources.len())
    {
        // Count how many of those sources are risky
        for source in sources {
            // Check if the source is known to be risky
            if let Some(record) = &source.counterparty_record {
                if record.risk_level == "High"
                    || record.risk_level == "Critical"
                    || record.category.contains("Suspicious")
                    || record.category.contains("Mixer")
                {
                    risky_source_count += 1;
                    total_suspicious_amount += source.amount;
                }
            }
        }

        // If we have multiple risky sources in one time window, that's suspicious
        if risky_source_count >= MINIMUM_SOURCES {
            let total_sources = sources.len();
            let risky_percentage = (risky_source_count as f32) / (total_sources as f32);

            tracing::info!(
                "Fund consolidation detected for wallet {}: received funds from {} risky sources ({:.1}% of all sources) totaling {:.2} SOL within a 24-hour period",
                wallet_context.address,
                risky_source_count,
                risky_percentage * 100.0,
                total_suspicious_amount
            );

            // Calculate a score based on number of sources and percentage of risky ones
            let source_factor =
                (risky_source_count.min(SUSPICIOUS_SOURCES) as f32) / (SUSPICIOUS_SOURCES as f32);
            let risk_ratio_factor = risky_percentage;
            let score = (source_factor + risk_ratio_factor) / 2.0;

            return (true, score);
        }
    }

    (false, 0.0)
}
