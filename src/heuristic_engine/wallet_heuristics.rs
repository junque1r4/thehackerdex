use super::types::{WalletContext, WalletTransaction};
use std::collections::HashMap;
use tracing;

pub fn check_high_frequency_volume(wallet_context: &WalletContext) -> (bool, f32) {
    // Constants for detection thresholds
    const TX_COUNT_24H_THRESHOLD: u32 = 50; // More than 50 txs in 24h is suspicious
    const TX_VOLUME_24H_THRESHOLD: f64 = 100_000.0; // More than 100,000 SOL in 24h is suspicious
    const TX_HOURLY_BURST_THRESHOLD: u32 = 10; // More than 10 txs per hour is a burst

    let mut score = 0.0;
    let mut is_high_frequency = false;

    // Check basic transaction count threshold
    if wallet_context.tx_count_24h >= TX_COUNT_24H_THRESHOLD {
        score += 0.3;
        is_high_frequency = true;
    }

    // Check volume threshold (high value transactions)
    if wallet_context.volume_24h >= TX_VOLUME_24H_THRESHOLD {
        score += 0.3;
        is_high_frequency = true;
    }

    // Create a merged list of transactions (both incoming and outgoing) for time analysis
    let mut all_transactions: Vec<&WalletTransaction> = Vec::new();
    all_transactions.extend(wallet_context.recent_incoming_txs.iter());
    all_transactions.extend(wallet_context.recent_outgoing_txs.iter());

    // Sort transactions by timestamp
    all_transactions.sort_by_key(|tx| tx.timestamp);

    if !all_transactions.is_empty() {
        // Group transactions by hour
        let mut hourly_counts: HashMap<i64, u32> = HashMap::new();

        for tx in &all_transactions {
            // Convert timestamp to hour (integer division by 3600)
            let hour_bucket = tx.timestamp / 3600;
            *hourly_counts.entry(hour_bucket).or_insert(0) += 1;
        }

        // Analyze hourly distribution for bursts
        let mut has_bursts = false;
        let mut max_burst = 0;

        for (_hour, count) in hourly_counts {
            if count > TX_HOURLY_BURST_THRESHOLD {
                has_bursts = true;
                max_burst = max_burst.max(count);
            }
        }

        if has_bursts {
            // Scale the score based on how much the max burst exceeds the threshold
            let burst_factor = (max_burst as f32 - TX_HOURLY_BURST_THRESHOLD as f32)
                / TX_HOURLY_BURST_THRESHOLD as f32;

            // Cap the additional score at 0.4
            let additional_score = (burst_factor * 0.4).min(0.4);
            score += additional_score;
            is_high_frequency = true;
        }
    }

    // Cap the total score at 1.0
    score = score.min(1.0);

    (is_high_frequency, score)
}

/// Categories that represent legitimate trading venues where structuring patterns
/// are likely to be false positives due to normal trading activity
pub const SAFE_TRADING_CATEGORIES: [&str; 5] = [
    "DEX",
    "Exchange",
    "DEX Aggregator",
    "Stableswap",
    "AMM", // Automated Market Maker
];

pub fn check_structuring_patterns(wallet_context: &WalletContext) -> f32 {
    // Load parameters from config if available, otherwise use defaults
    // Configuration parameters that can be tuned to adjust sensitivity
    // Use hardcoded path as Config doesn't have analyze_config_path field
    let analyze_config_path = "config/analyze_config.toml";

    // Default constants for detection thresholds
    let mut min_transactions_for_structuring: usize = 5; // Need at least 5 txs to detect structuring
    let mut structuring_time_window: i64 = 24 * 3600; // 24 hours in seconds
    let mut similar_amount_threshold_ratio: f64 = 0.2; // 20% variance for "similar" amounts
    let mut small_tx_amount_threshold: f64 = 10.0; // A "small" transaction is < 10 SOL

    // Try to load configuration
    if let Ok(config_str) = std::fs::read_to_string(analyze_config_path) {
        if let Ok(config) = toml::from_str::<toml::Value>(&config_str) {
            // Extract structuring parameters if they exist
            if let Some(structuring) = config
                .get("exfiltration_rules")
                .and_then(|v| v.get("structuring"))
            {
                // Get min_transactions_for_structuring if it exists
                if let Some(min_txs) = structuring
                    .get("min_transactions_for_structuring")
                    .and_then(|v| v.as_integer())
                {
                    min_transactions_for_structuring = min_txs as usize;
                }

                // Get structuring_time_window if it exists
                if let Some(time_window) = structuring
                    .get("structuring_time_window")
                    .and_then(|v| v.as_integer())
                {
                    structuring_time_window = time_window;
                }

                // Get similar_amount_threshold_ratio if it exists
                if let Some(ratio) = structuring
                    .get("similar_amount_threshold_ratio")
                    .and_then(|v| v.as_float())
                {
                    similar_amount_threshold_ratio = ratio;
                }

                // Get small_tx_amount_threshold if it exists
                if let Some(threshold) = structuring
                    .get("small_tx_amount_threshold")
                    .and_then(|v| v.as_float())
                {
                    small_tx_amount_threshold = threshold;
                }
            }
        }
    }

    // Create separate lists for incoming and outgoing transactions while filtering out
    // transactions from safe trading categories (which are likely legitimate trading activity)
    let incoming_txs: Vec<_> = wallet_context
        .recent_incoming_txs
        .iter()
        .filter(|tx| {
            if let Some(record) = &tx.counterparty_record {
                // Skip transactions from known trading venues
                !SAFE_TRADING_CATEGORIES
                    .iter()
                    .any(|&safe_cat| record.category == safe_cat)
            } else {
                // Keep transactions with unknown counterparties
                true
            }
        })
        .cloned()
        .collect();

    let outgoing_txs: Vec<_> = wallet_context
        .recent_outgoing_txs
        .iter()
        .filter(|tx| {
            if let Some(record) = &tx.counterparty_record {
                // Skip transactions to known trading venues
                !SAFE_TRADING_CATEGORIES
                    .iter()
                    .any(|&safe_cat| record.category == safe_cat)
            } else {
                // Keep transactions with unknown counterparties
                true
            }
        })
        .cloned()
        .collect();

    tracing::debug!(
        "Filtered transactions for structuring analysis: {}/{} incoming, {}/{} outgoing remain after filtering out safe trading venues",
        incoming_txs.len(),
        wallet_context.recent_incoming_txs.len(),
        outgoing_txs.len(),
        wallet_context.recent_outgoing_txs.len(),
    );

    // Check if there are enough transactions to detect structuring after filtering
    if incoming_txs.len() < min_transactions_for_structuring
        && outgoing_txs.len() < min_transactions_for_structuring
    {
        return 0.0; // Not enough transactions to analyze after filtering
    }

    // Helper function to analyze transactions for structuring patterns
    let analyze_txs_for_structuring = |txs: &[WalletTransaction]| -> f32 {
        if txs.len() < min_transactions_for_structuring {
            return 0.0;
        }

        // Group by time windows (24-hour periods)
        let mut time_windows: HashMap<i64, Vec<&WalletTransaction>> = HashMap::new();

        for tx in txs {
            // Use day as the bucket (integer division of timestamp by seconds in a day)
            let day_bucket = tx.timestamp / structuring_time_window;
            time_windows.entry(day_bucket).or_default().push(tx);
        }

        let mut max_window_score: f32 = 0.0;

        // Analyze each time window
        for (_day, window_txs) in time_windows {
            if window_txs.len() < min_transactions_for_structuring {
                continue;
            }

            // Count small transactions
            let small_tx_count = window_txs
                .iter()
                .filter(|tx| tx.amount < small_tx_amount_threshold)
                .count();

            let small_tx_ratio = small_tx_count as f32 / window_txs.len() as f32;

            // Calculate the total amount
            let _total_amount: f64 = window_txs.iter().map(|tx| tx.amount).sum();

            // Look for similar amounts (within 20% of each other)
            // We use a custom approach here because f64 doesn't implement Hash
            let mut amount_clusters: Vec<(f64, Vec<&WalletTransaction>)> = Vec::new();

            for tx in &window_txs {
                // Find or create a cluster for this amount
                let mut found_cluster = false;

                for i in 0..amount_clusters.len() {
                    let cluster_amount = amount_clusters[i].0;
                    let ratio = if cluster_amount > tx.amount {
                        tx.amount / cluster_amount
                    } else {
                        cluster_amount / tx.amount
                    };

                    if ratio > (1.0 - similar_amount_threshold_ratio) {
                        amount_clusters[i].1.push(tx);
                        found_cluster = true;
                        break;
                    }
                }

                if !found_cluster {
                    amount_clusters.push((tx.amount, vec![tx]));
                }
            }

            // Calculate structuring indicators
            // 1. Multiple similar transactions
            let largest_cluster_size = amount_clusters
                .iter()
                .map(|(_, cluster)| cluster.len())
                .max()
                .unwrap_or(0);

            let cluster_ratio = largest_cluster_size as f32 / window_txs.len() as f32;

            // 2. Transactions spaced out evenly (time analysis)
            // Sort by timestamp
            let mut sorted_timestamps: Vec<i64> =
                window_txs.iter().map(|tx| tx.timestamp).collect();
            sorted_timestamps.sort_unstable();

            // Calculate time differences between consecutive transactions
            let mut time_diffs = Vec::with_capacity(sorted_timestamps.len() - 1);
            for i in 1..sorted_timestamps.len() {
                time_diffs.push((sorted_timestamps[i] - sorted_timestamps[i - 1]).abs());
            }

            // Check if time differences are consistent (indicating potential automation)
            let mut consistent_timing = false;
            if !time_diffs.is_empty() {
                let avg_diff: f64 = time_diffs.iter().sum::<i64>() as f64 / time_diffs.len() as f64;
                let mut consistent_count = 0;

                for diff in &time_diffs {
                    let ratio = *diff as f64 / avg_diff;
                    if ratio > 0.7 && ratio < 1.3 {
                        consistent_count += 1;
                    }
                }

                let consistency_ratio = consistent_count as f32 / time_diffs.len() as f32;
                if consistency_ratio > 0.6 {
                    consistent_timing = true;
                }
            }

            // Calculate window score
            let mut window_score = 0.0;

            // Small transactions increase structuring likelihood
            window_score += small_tx_ratio * 0.4;

            // Similar amount clusters increase structuring likelihood
            window_score += cluster_ratio * 0.4;

            // Consistent timing increases structuring likelihood
            if consistent_timing {
                window_score += 0.2;
            }

            // If multiple small transactions to the same counterparty, higher likelihood
            let mut counterparty_counts: HashMap<&str, usize> = HashMap::new();
            for tx in &window_txs {
                *counterparty_counts.entry(&tx.counterparty).or_insert(0) += 1;
            }

            let max_same_counterparty = counterparty_counts.values().max().unwrap_or(&0);
            if *max_same_counterparty > min_transactions_for_structuring {
                let counterparty_ratio = *max_same_counterparty as f32 / window_txs.len() as f32;
                window_score += counterparty_ratio * 0.2;
            }

            // Update max score across all windows
            max_window_score = max_window_score.max(window_score);
        }

        max_window_score
    };

    // Analyze both incoming and outgoing transactions
    let incoming_score = analyze_txs_for_structuring(&incoming_txs);
    let outgoing_score = analyze_txs_for_structuring(&outgoing_txs);

    // Take the higher of the two scores
    let structuring_score = incoming_score.max(outgoing_score);

    // Apply a reduction factor if a significant portion of the wallet's activity
    // involves safe trading categories (indicating it's likely a trader)
    let mut reduction_factor = 1.0;

    // Count interactions with trading venues
    let mut total_interactions = 0;
    let mut safe_category_interactions = 0;

    // Check incoming transactions
    for tx in &wallet_context.recent_incoming_txs {
        total_interactions += 1;
        if let Some(record) = &tx.counterparty_record {
            if SAFE_TRADING_CATEGORIES
                .iter()
                .any(|&safe_cat| record.category == safe_cat)
            {
                safe_category_interactions += 1;
            }
        }
    }

    // Check outgoing transactions
    for tx in &wallet_context.recent_outgoing_txs {
        total_interactions += 1;
        if let Some(record) = &tx.counterparty_record {
            if SAFE_TRADING_CATEGORIES
                .iter()
                .any(|&safe_cat| record.category == safe_cat)
            {
                safe_category_interactions += 1;
            }
        }
    }

    // If more than 30% of transactions involve trading venues,
    // reduce the score proportionally (more trading = lower score)
    if total_interactions > 0 {
        let safe_ratio = safe_category_interactions as f32 / total_interactions as f32;
        if safe_ratio > 0.3 {
            // Reduce score by up to 80% for very active traders
            reduction_factor = (1.0 - safe_ratio).max(0.2);
            tracing::debug!(
                "Reducing structuring score by factor {} due to trading activity ratio of {:.2}",
                reduction_factor,
                safe_ratio
            );
        }
    }

    // Apply reduction and cap the score at 1.0
    (structuring_score * reduction_factor).min(1.0)
}

pub fn check_pass_through(wallet_context: &WalletContext) -> (bool, f32) {
    // Constants for detection thresholds
    const QUICK_TRANSFER_THRESHOLD: i64 = 7200; // 2 hours in seconds
    const VERY_QUICK_TRANSFER_THRESHOLD: i64 = 600; // 10 minutes in seconds
    const SIMILAR_AMOUNT_THRESHOLD: f64 = 0.85; // 85% of received amount is sent out
    const MIN_AMOUNT_THRESHOLD: f64 = 0.5; // Minimum 0.5 SOL to be considered

    let incoming_txs = &wallet_context.recent_incoming_txs;
    let outgoing_txs = &wallet_context.recent_outgoing_txs;

    // If not enough transactions, can't determine pass-through behavior
    if incoming_txs.is_empty() || outgoing_txs.is_empty() {
        return (false, 0.0);
    }

    // Sort transactions by timestamp
    let mut sorted_incoming: Vec<&WalletTransaction> = incoming_txs.iter().collect();
    let mut sorted_outgoing: Vec<&WalletTransaction> = outgoing_txs.iter().collect();

    sorted_incoming.sort_by_key(|tx| tx.timestamp);
    sorted_outgoing.sort_by_key(|tx| tx.timestamp);

    let mut pass_through_count = 0;
    let mut very_quick_pass_through_count = 0;
    let mut pass_through_amount_total = 0.0;
    let mut total_incoming_amount = 0.0;

    // Find pairs of incoming followed closely by outgoing transactions
    for in_tx in &sorted_incoming {
        // Skip very small amounts
        if in_tx.amount < MIN_AMOUNT_THRESHOLD {
            continue;
        }

        total_incoming_amount += in_tx.amount;

        // Find outgoing transactions that occurred after this incoming transaction
        let matching_out_txs: Vec<&&WalletTransaction> = sorted_outgoing
            .iter()
            .filter(|out_tx| {
                // Only consider outgoing txs that happened after this incoming tx
                out_tx.timestamp > in_tx.timestamp
                    && (out_tx.timestamp - in_tx.timestamp) < QUICK_TRANSFER_THRESHOLD
                // And within the threshold time window
            })
            .collect();

        for out_tx in matching_out_txs {
            // Check if the outgoing amount is similar to the incoming amount
            if out_tx.amount >= in_tx.amount * SIMILAR_AMOUNT_THRESHOLD {
                pass_through_count += 1;
                pass_through_amount_total += out_tx.amount;

                // Check if it's a very quick transfer (within minutes)
                if (out_tx.timestamp - in_tx.timestamp) < VERY_QUICK_TRANSFER_THRESHOLD {
                    very_quick_pass_through_count += 1;
                }

                // We've found a match for this incoming tx, stop looking
                break;
            }
        }
    }

    // Calculate a score based on findings
    let mut score = 0.0;
    let is_pass_through = pass_through_count > 0;

    if is_pass_through {
        // Base score: ratio of pass-through transactions to total incoming (max 0.5)
        if !sorted_incoming.is_empty() {
            let ratio = pass_through_count as f32 / sorted_incoming.len() as f32;
            score += ratio * 0.5;
        }

        // Additional score for very quick transfers (max 0.3)
        if pass_through_count > 0 {
            let quick_ratio = very_quick_pass_through_count as f32 / pass_through_count as f32;
            score += quick_ratio * 0.3;
        }

        // Additional score based on the percentage of total incoming funds that were passed through (max 0.2)
        if total_incoming_amount > 0.0 {
            let amount_ratio = (pass_through_amount_total / total_incoming_amount) as f32;
            score += amount_ratio * 0.2;
        }
    }

    // Cap the score at 1.0
    let final_score = score.min(1.0);

    (is_pass_through, final_score)
}

/// Checks if a wallet is new and potentially suspicious based on its age and activity
///
/// This function analyzes both the creation date of the wallet and its transaction patterns
/// to determine if it represents a potentially suspicious new wallet. New wallets with high
/// activity levels are particularly suspicious in the context of illicit activities.
///
/// # Arguments
///
/// * `wallet_context` - Context information about the wallet being analyzed
///
/// # Returns
///
/// A tuple containing:
/// * A boolean indicating if this is a suspicious new wallet
/// * A score (0.0-1.0) representing the suspicion level
pub fn check_wallet_age_and_activity(wallet_context: &WalletContext) -> (bool, f32) {
    // Constants for detection thresholds
    const NEW_WALLET_DAYS_THRESHOLD: u32 = 30; // Wallets less than 30 days old are considered "new"
    const VERY_NEW_WALLET_DAYS_THRESHOLD: u32 = 7; // Wallets less than 7 days old are "very new"
    const HIGH_TX_COUNT_FOR_NEW_WALLET: u32 = 20; // 20+ transactions for a new wallet is suspicious
    const HIGH_VOLUME_FOR_NEW_WALLET: f64 = 10000.0; // 10,000+ SOL volume for a new wallet is suspicious

    // Check if this is a new wallet (based on creation timestamp)
    let is_new_wallet = wallet_context.is_new_wallet(NEW_WALLET_DAYS_THRESHOLD);

    // If it's not new, return early
    if !is_new_wallet {
        return (false, 0.0);
    }

    let is_very_new = wallet_context.is_new_wallet(VERY_NEW_WALLET_DAYS_THRESHOLD);
    let mut score = 0.0;
    let mut is_suspicious = false;

    // Check if this new wallet has high activity
    let has_high_tx_count = wallet_context.tx_count_24h > HIGH_TX_COUNT_FOR_NEW_WALLET
        || wallet_context.tx_count_7d > HIGH_TX_COUNT_FOR_NEW_WALLET * 3;

    let has_high_volume = wallet_context.volume_24h > HIGH_VOLUME_FOR_NEW_WALLET
        || wallet_context.volume_7d > HIGH_VOLUME_FOR_NEW_WALLET * 3.0;

    // Base score just for being new
    if is_very_new {
        score += 0.3; // Very new wallets get a higher base score
    } else {
        score += 0.1; // Regular new wallets get a small base score
    }

    // Add to the score based on activity levels
    if has_high_tx_count {
        score += 0.3;
        is_suspicious = true;

        tracing::debug!(
            "New wallet {} with high transaction count: {} in 24h, {} in 7d",
            wallet_context.address,
            wallet_context.tx_count_24h,
            wallet_context.tx_count_7d
        );
    }

    if has_high_volume {
        score += 0.4;
        is_suspicious = true;

        tracing::debug!(
            "New wallet {} with high volume: {} SOL in 24h, {} SOL in 7d",
            wallet_context.address,
            wallet_context.volume_24h,
            wallet_context.volume_7d
        );
    }

    // A new wallet that receives funds and immediately sends them out is more suspicious
    if is_suspicious {
        let (is_pass_through, pass_through_score) = check_pass_through(wallet_context);
        if is_pass_through {
            // If the wallet is also a pass-through, add a portion of its pass-through score
            score += pass_through_score * 0.3;

            tracing::debug!(
                "New suspicious wallet {} also exhibits pass-through behavior",
                wallet_context.address
            );
        }
    }

    // Cap the score at 1.0
    let final_score = score.min(1.0);

    (is_suspicious && is_new_wallet, final_score)
}

/// Analyzes a wallet's recent funding sources to identify potential risk
///
/// This function examines the recent incoming transactions to a wallet and evaluates
/// the risk level of the sources of those funds, checking for known risky addresses
/// and suspicious patterns in the funding chain.
///
/// # Arguments
///
/// * `wallet_context` - Context information about the wallet being analyzed
///
/// # Returns
///
/// A float representing the ratio (0.0-1.0) of funds coming from risky sources
pub fn analyze_funding_sources(wallet_context: &WalletContext) -> f32 {
    // If no incoming transactions, we can't analyze funding sources
    if wallet_context.recent_incoming_txs.is_empty() {
        return 0.0;
    }

    // Track totals for calculating ratios
    let mut total_incoming_amount: f64 = 0.0;
    let mut risky_incoming_amount: f64 = 0.0;
    let mut high_risk_incoming_amount: f64 = 0.0;

    // Analyze each funding source (incoming transaction)
    for tx in &wallet_context.recent_incoming_txs {
        total_incoming_amount += tx.amount;

        // Check if the counterparty (source) is a known entity
        if tx.is_known_counterparty {
            if let Some(record) = &tx.counterparty_record {
                // Categorize risk based on the record
                match record.risk_level.as_str() {
                    "Critical" => {
                        // Critical risk sources are weighted fully
                        risky_incoming_amount += tx.amount;
                        high_risk_incoming_amount += tx.amount;

                        tracing::debug!(
                            "Critical risk funding source detected: {} sent {} SOL to {}",
                            tx.counterparty,
                            tx.amount,
                            wallet_context.address
                        );
                    }
                    "High" => {
                        // High risk sources are weighted fully
                        risky_incoming_amount += tx.amount;
                        high_risk_incoming_amount += tx.amount;

                        tracing::debug!(
                            "High risk funding source detected: {} sent {} SOL to {}",
                            tx.counterparty,
                            tx.amount,
                            wallet_context.address
                        );
                    }
                    "Medium" => {
                        // Medium risk sources are weighted partially
                        risky_incoming_amount += tx.amount * 0.5;

                        tracing::debug!(
                            "Medium risk funding source detected: {} sent {} SOL to {}",
                            tx.counterparty,
                            tx.amount,
                            wallet_context.address
                        );
                    }
                    _ => {
                        // Low or unknown risk are not counted towards risky amounts
                    }
                }

                // Also consider specific high-risk categories regardless of risk level
                match record.category.as_str() {
                    "Mixer" | "Mixer Feeder Address" | "Known Hacker" | "Sanctioned Entity" => {
                        if !high_risk_incoming_amount.gt(&0.0) {
                            // Only add this amount if it hasn't already been counted as high risk
                            risky_incoming_amount += tx.amount;
                            high_risk_incoming_amount += tx.amount;

                            tracing::debug!(
                                "High risk category funding source: {} ({}) sent {} SOL to {}",
                                record.entity_name,
                                tx.counterparty,
                                tx.amount,
                                wallet_context.address
                            );
                        }
                    }
                    "High-Risk Gambling DApp" | "Privacy Service" | "Suspicious Exchange" => {
                        if risky_incoming_amount.lt(&(tx.amount * 0.5)) {
                            // Only add this amount if it hasn't already been counted as medium risk
                            risky_incoming_amount += tx.amount * 0.5;

                            tracing::debug!(
                                "Medium risk category funding source: {} ({}) sent {} SOL to {}",
                                record.entity_name,
                                tx.counterparty,
                                tx.amount,
                                wallet_context.address
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Calculate the ratio of risky funding sources
    let risky_ratio = if total_incoming_amount > 0.0 {
        (risky_incoming_amount / total_incoming_amount) as f32
    } else {
        0.0
    };

    // If there are high-risk sources contributing significant amounts, increase the score
    if high_risk_incoming_amount > 0.0 && total_incoming_amount > 0.0 {
        let high_risk_ratio = (high_risk_incoming_amount / total_incoming_amount) as f32;

        // If more than 20% of funds are from high-risk sources, ensure score is at least 0.6
        if high_risk_ratio > 0.2 {
            return risky_ratio.max(0.6);
        }
    }

    risky_ratio
}

/// Analyzes a wallet's recent spending destinations to identify potential risk
///
/// This function examines the recent outgoing transactions from a wallet and evaluates
/// the risk level of the destinations of those funds, checking for known risky addresses
/// and suspicious patterns in the spending behavior.
///
/// # Arguments
///
/// * `wallet_context` - Context information about the wallet being analyzed
///
/// # Returns
///
/// A float representing the ratio (0.0-1.0) of funds going to risky destinations
pub fn analyze_spending_destinations(wallet_context: &WalletContext) -> f32 {
    // If no outgoing transactions, we can't analyze spending destinations
    if wallet_context.recent_outgoing_txs.is_empty() {
        return 0.0;
    }

    // Track totals for calculating ratios
    let mut total_outgoing_amount: f64 = 0.0;
    let mut risky_outgoing_amount: f64 = 0.0;
    let mut high_risk_outgoing_amount: f64 = 0.0;

    // Count distinct destinations to identify patterns
    let mut destination_counts: HashMap<&str, (f64, bool)> = HashMap::new();

    // Analyze each spending destination (outgoing transaction)
    for tx in &wallet_context.recent_outgoing_txs {
        total_outgoing_amount += tx.amount;

        // Track distinct destinations and amounts
        let (total_amount, is_risky) = destination_counts
            .entry(&tx.counterparty)
            .or_insert((0.0, false));
        *total_amount += tx.amount;

        // Check if the counterparty (destination) is a known entity
        if tx.is_known_counterparty {
            if let Some(record) = &tx.counterparty_record {
                // Categorize risk based on the record
                match record.risk_level.as_str() {
                    "Critical" => {
                        // Critical risk destinations are weighted fully
                        risky_outgoing_amount += tx.amount;
                        high_risk_outgoing_amount += tx.amount;
                        *is_risky = true;

                        tracing::debug!(
                            "Critical risk spending destination detected: {} sent {} SOL to {}",
                            wallet_context.address,
                            tx.amount,
                            tx.counterparty
                        );
                    }
                    "High" => {
                        // High risk destinations are weighted fully
                        risky_outgoing_amount += tx.amount;
                        high_risk_outgoing_amount += tx.amount;
                        *is_risky = true;

                        tracing::debug!(
                            "High risk spending destination detected: {} sent {} SOL to {}",
                            wallet_context.address,
                            tx.amount,
                            tx.counterparty
                        );
                    }
                    "Medium" => {
                        // Medium risk destinations are weighted partially
                        risky_outgoing_amount += tx.amount * 0.5;
                        *is_risky = true;

                        tracing::debug!(
                            "Medium risk spending destination detected: {} sent {} SOL to {}",
                            wallet_context.address,
                            tx.amount,
                            tx.counterparty
                        );
                    }
                    _ => {
                        // Low or unknown risk are not counted towards risky amounts
                    }
                }

                // Also consider specific high-risk categories regardless of risk level
                match record.category.as_str() {
                    "Mixer" | "Mixer Feeder Address" | "Known Hacker" | "Sanctioned Entity" => {
                        if !high_risk_outgoing_amount.gt(&0.0) {
                            // Only add this amount if it hasn't already been counted as high risk
                            risky_outgoing_amount += tx.amount;
                            high_risk_outgoing_amount += tx.amount;
                            *is_risky = true;

                            tracing::debug!(
                                "High risk category spending destination: {} sent {} SOL to {} ({})",
                                wallet_context.address,
                                tx.amount,
                                record.entity_name,
                                tx.counterparty
                            );
                        }
                    }
                    "High-Risk Gambling DApp" | "Privacy Service" | "Suspicious Exchange" => {
                        if risky_outgoing_amount.lt(&(tx.amount * 0.5)) {
                            // Only add this amount if it hasn't already been counted as medium risk
                            risky_outgoing_amount += tx.amount * 0.5;
                            *is_risky = true;

                            tracing::debug!(
                                "Medium risk category spending destination: {} sent {} SOL to {} ({})",
                                wallet_context.address,
                                tx.amount,
                                record.entity_name,
                                tx.counterparty
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Calculate the ratio of risky spending destinations
    let risky_ratio = if total_outgoing_amount > 0.0 {
        (risky_outgoing_amount / total_outgoing_amount) as f32
    } else {
        0.0
    };

    // Check for fund dispersal pattern (sending to many different destinations)
    let distinct_destinations = destination_counts.len();
    let risky_destinations = destination_counts
        .values()
        .filter(|(_, is_risky)| *is_risky)
        .count();

    // If there are many distinct destinations and some are risky, increase the score
    if distinct_destinations > 5 && risky_destinations > 0 {
        let dispersal_factor = (risky_destinations as f32 / distinct_destinations as f32) * 0.2;
        return (risky_ratio + dispersal_factor).min(1.0);
    }

    // If there are high-risk destinations receiving significant amounts, increase the score
    if high_risk_outgoing_amount > 0.0 && total_outgoing_amount > 0.0 {
        let high_risk_ratio = (high_risk_outgoing_amount / total_outgoing_amount) as f32;

        // If more than 20% of funds go to high-risk destinations, ensure score is at least 0.6
        if high_risk_ratio > 0.2 {
            return risky_ratio.max(0.6);
        }
    }

    risky_ratio
}
