use super::types::WalletContext;
use std::collections::{HashMap, HashSet};
use tracing;

/// Categories that represent legitimate trading venues
/// Transactions with these categories should be excluded from bidirectional flow detection
/// as they represent normal trading activity
pub const SAFE_TRADING_CATEGORIES: [&str; 5] = [
    "DEX",
    "Exchange",
    "DEX Aggregator",
    "Stableswap",
    "AMM", // Automated Market Maker
];

/// Detects bidirectional fund flows between a wallet and its counterparties
///
/// This function identifies cases where funds move back and forth between a wallet
/// and specific counterparties within a given time window, which can indicate wash trading,
/// circular transactions to obscure origins, or other suspicious patterns.
///
/// # Arguments
///
/// * `wallet_context` - Context information about the wallet being analyzed
///
/// # Returns
///
/// A tuple containing:
/// * A boolean indicating if bidirectional flow patterns were detected
/// * A float score (0.0-1.0) representing the suspicion level
pub fn check_bidirectional_flow(wallet_context: &WalletContext) -> (bool, f32) {
    // Constants for detection thresholds
    const TIME_WINDOW_SECONDS: i64 = 7 * 24 * 3600; // 7 days
    const MINIMUM_BIDIRECTIONAL_RATIO: f64 = 0.7; // At least 70% of funds "round-tripping"
    const MINIMUM_TRANSACTIONS: usize = 2; // Need at least 2 transactions in each direction
    const MINIMUM_VOLUME: f64 = 1.0; // Minimum volume in SOL to consider significant

    // Early return if not enough transaction history
    if wallet_context.recent_incoming_txs.is_empty()
        || wallet_context.recent_outgoing_txs.is_empty()
    {
        return (false, 0.0);
    }

    // Maps to track fund flow with each counterparty
    let mut funding_counterparties: HashMap<String, Vec<&super::types::WalletTransaction>> =
        HashMap::new();
    let mut spending_counterparties: HashMap<String, Vec<&super::types::WalletTransaction>> =
        HashMap::new();

    // Track the earliest timestamp we're considering
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let earliest_time = current_time - TIME_WINDOW_SECONDS;

    // Group incoming transactions by counterparty within the time window
    for tx in &wallet_context.recent_incoming_txs {
        if tx.timestamp >= earliest_time {
            funding_counterparties
                .entry(tx.counterparty.clone())
                .or_default()
                .push(tx);
        }
    }

    // Group outgoing transactions by counterparty within the time window
    for tx in &wallet_context.recent_outgoing_txs {
        if tx.timestamp >= earliest_time {
            spending_counterparties
                .entry(tx.counterparty.clone())
                .or_default()
                .push(tx);
        }
    }

    // Find counterparties that appear in both incoming and outgoing transactions
    let mut bidirectional_counterparties: HashSet<String> = HashSet::new();
    let mut bidirectional_flow_score: f32 = 0.0;
    let mut found_bidirectional_flow = false;
    let mut significant_bidirectional_pairs = 0;

    // For each funding counterparty, check if we also sent funds to them
    for (address, incoming_txs) in &funding_counterparties {
        // Skip if this isn't a counterparty we've also sent funds to
        if !spending_counterparties.contains_key(address) {
            continue;
        }

        // Get the outgoing transactions to this counterparty
        let outgoing_txs = spending_counterparties.get(address).unwrap();

        // Skip if we don't have enough transactions in either direction
        if incoming_txs.len() < MINIMUM_TRANSACTIONS || outgoing_txs.len() < MINIMUM_TRANSACTIONS {
            continue;
        }

        // Calculate total volume in both directions
        let incoming_volume: f64 = incoming_txs.iter().map(|tx| tx.amount).sum();
        let outgoing_volume: f64 = outgoing_txs.iter().map(|tx| tx.amount).sum();

        // Skip if volume is too low
        if incoming_volume < MINIMUM_VOLUME || outgoing_volume < MINIMUM_VOLUME {
            continue;
        }

        // Check if this counterparty is in our safe trading categories
        if let Some(first_tx) = incoming_txs.first() {
            if let Some(record) = &first_tx.counterparty_record {
                if SAFE_TRADING_CATEGORIES.contains(&record.category.as_str()) {
                    // Skip this counterparty as it's a legitimate trading venue
                    tracing::debug!(
                        "Skipping bidirectional flow check for {} as it's a legitimate trading venue ({})",
                        address,
                        record.category
                    );
                    continue;
                }
            }
        }

        // Also check the counterparty from outgoing transactions
        if let Some(first_tx) = outgoing_txs.first() {
            if let Some(record) = &first_tx.counterparty_record {
                if SAFE_TRADING_CATEGORIES.contains(&record.category.as_str()) {
                    // Skip this counterparty as it's a legitimate trading venue
                    tracing::debug!(
                        "Skipping bidirectional flow check for {} as it's a legitimate trading venue ({})",
                        address,
                        record.category
                    );
                    continue;
                }
            }
        }

        // Calculate ratio of funds going back and forth
        let ratio = if incoming_volume > outgoing_volume {
            outgoing_volume / incoming_volume
        } else {
            incoming_volume / outgoing_volume
        };

        // If the ratio is above our threshold, this is a bidirectional flow
        if ratio >= MINIMUM_BIDIRECTIONAL_RATIO {
            bidirectional_counterparties.insert(address.clone());
            found_bidirectional_flow = true;
            significant_bidirectional_pairs += 1;

            // Calculate a score for this pair
            let volume_factor = ((incoming_volume + outgoing_volume) / 100.0).min(1.0);
            let pair_score = ratio as f32 * volume_factor as f32;
            bidirectional_flow_score = bidirectional_flow_score.max(pair_score);

            tracing::info!(
                "Bidirectional flow detected between {} and {}: {:.2} SOL in, {:.2} SOL out, ratio {:.2}",
                wallet_context.address,
                address,
                incoming_volume,
                outgoing_volume,
                ratio
            );
        }
    }

    // If we have multiple bidirectional pairs, increase the score
    if significant_bidirectional_pairs > 1 {
        let multiplier = (1.0 + (significant_bidirectional_pairs as f32 - 1.0) * 0.1).min(1.5);
        bidirectional_flow_score *= multiplier;
    }

    // Cap the score at 1.0
    bidirectional_flow_score = bidirectional_flow_score.min(1.0);

    (found_bidirectional_flow, bidirectional_flow_score)
}
