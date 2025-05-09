use anyhow::{Context, Result};

use thehackerdex::{
    analysis::transaction_parser,
    config::Config,
    db::{
        models::{AddressData, AddressRecord},
        repository::Repository,
    },
    error::HackerdexError,
    heuristic_engine::types::HeuristicFlags,
    rpc::client::RateLimitedClient,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use solana_client::rpc_response::RpcConfirmedTransactionStatusWithSignature;
use solana_sdk::{program_pack::Pack, pubkey::Pubkey, signature::Signature};
use solana_transaction_status::{EncodedTransaction, UiMessage};
use spl_token::state::{Account as TokenAccount, Mint};
use sqlx::postgres::PgPoolOptions;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::{signal, sync::Semaphore, time::sleep};
use tracing::{Level, debug, error, info, warn};
use tracing_subscriber::FmtSubscriber;

/// List of common Solana program addresses that should never be considered counterparties
const KNOWN_PROGRAM_ADDRESSES: &[&str] = &[
    // Core Solana Programs
    "11111111111111111111111111111111", // System Program
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA", // SPL Token Program
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL", // Associated Token Account Program
    "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr", // SPL Memo Program
    "ComputeBudget111111111111111111111111111111", // Compute Budget Program
    "Vote111111111111111111111111111111111111111", // Vote Program
    "Stake11111111111111111111111111111111111111", // Stake Program
    "BPFLoaderUpgradeab1e11111111111111111111111", // BPF Upgradable Loader
    "BPFLoader2111111111111111111111111111111111", // BPF Loader 2
    // DEXs and AMMs
    "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin", // Serum DEX v3
    "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8", // Raydium AMM
    "5quBtoiQqxF9Jv6KYKctB59NT3gtJD2Y65kdnB1Uev3h", // Raydium Staking
    "9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP", // Orca Whirlpool
    "DjVE6JNiYqPL2QXyCUUh8rNjHrbz9hXHNYt99MQ59qw1", // Orca Pool
    "SSwpkEEcbUqx4vtoEByFjSkhKdCT862DNVb52nZg1UZ",  // Saber Stableswap
    "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY",  // Phoenix DEX
    "4MangoMjqJ2firMokCrCZvBhJzjGPV9gYcV2iWTRKhQJ", // Mango Markets v4
    // Jupiter Aggregator
    "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4", // Jupiter v6
    "JUP2jxvXaqu7NQY1GmNF4m1vodw12LVXYxbFL2uJvfo", // Jupiter v2
    "JUP3c2Uh3WA4Ng34tw6kPd2G4C5BB21Xo36Je1s32Ph", // Jupiter v3
    "JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB", // Jupiter v4
    // Lending Protocols
    "mrgnFMdcVuF8M5SzvmE33dgJ65p8uJxfNsjhf6nyFpk", // MarginFi Bank
    "MRGNBSZzULmY1D4WPX536Q5RoBKypXYYfP3NkTaGbhB", // MarginFi Admin
    "So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo", // Solend Main Program
    // NFTs and Metaplex
    "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s", // Metaplex Metadata
    "p1exdMJcjVao65QdewkaZRUnU6VPSXhus9n2GzWfh98", // Metaplex Candy Machine v1
    "cndy3Z4yapfJBmL3ShUp5exZKqR3z33thTzeNMm2gRZ", // Metaplex Candy Machine v2
    "CMZYPASGWeTz7RNGHaRJfCq2XQ5pYK6nDvVQxzkH51zb", // Metaplex Auction House
    "M2mx93ekt1fmXSVkTrUL9xVFHkmME8HTUi5Cyc5aF7K", // Magic Eden v2
    // Oracles and Bridges
    "FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH", // Pyth Oracle
    "worm2ZoG2kUd4vFXhvjh93UUH596ayRfgQ2MgjNMTth",  // Wormhole v2
    // Other infrastructure
    "MarBmsSgKXdrN1egZf5sqe1TMai9K1rChYNDJgjq7aD", // Marinade Staking
    "SMPLecH534NA9acpos4G6x7uf3LWbCAwZQE9e8ZekMu", // Squads Multisig
    "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH", // Drift Protocol
];

/// Categories of addresses that should never be considered counterparties
const EXCLUDED_CATEGORIES: &[&str] = &[
    // Core infrastructure
    "core program",
    "token program",
    "program",
    "contract",
    "system program",
    // NFT related
    "nft program",
    "nft",
    "nft marketplace",
    "metaplex",
    // Oracles and governance
    "oracle",
    "multisig program",
    "multisig",
    "governance",
    // DEXes and trading infrastructure
    "dex",
    "dex aggregator",
    "amm",
    "liquidity pool",
    "pool",
    "swap",
    // Cross-chain
    "bridge",
    "stableswap",
    "cross-chain bridge",
    "interoperability",
    // Yield and staking
    "staking",
    "yield farming",
    "farm",
    "validator",
    // DeFi infrastructure
    "lending protocol",
    "protocol",
    "defi protocol",
    "vault",
    "token mint",
    // Special purpose addresses
    "utility",
    "fee collector",
    "treasury",
];

/// Configuration for the wallet analysis tool
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnalyzeConfig {
    /// Whether to analyze all wallets in the database
    analyze_all: bool,
    /// List of specific categories to analyze
    analyze_categories: Option<Vec<String>>,
    /// List of specific risk levels to analyze
    analyze_risk_levels: Option<Vec<String>>,
    /// List of specific addresses to analyze
    analyze_addresses: Option<Vec<String>>,
    /// Maximum number of historical transactions to fetch per address
    max_history_transactions: Option<usize>,
    /// Maximum number of days to look back in history
    max_history_days: Option<u32>,
    /// Whether to update the database notes field with the analysis results
    update_db_notes: bool,
    /// Maximum number of concurrent analysis tasks
    max_concurrent_tasks: usize,
    /// Exfiltration pattern rules mapping
    exfiltration_rules: ExfiltrationRules,
}

/// Exfiltration pattern rules for tagging wallets
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExfiltrationRules {
    /// Rules for mixer interaction detection
    mixer_interaction: MixerRule,
    /// Rules for bridge hopping detection
    bridge_hopping: BridgeHoppingRule,
    /// Rules for structuring detection
    structuring: StructuringRule,
    /// Rules for drainer consolidation detection
    drainer_consolidation: DrainerConsolidationRule,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MixerRule {
    /// Minimum ratio of spending to risky destinations
    min_risky_spending_ratio: f32,
    /// Categories that are considered mixers
    mixer_categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BridgeHoppingRule {
    /// Whether the wallet should be flagged as a pass-through
    is_pass_through: bool,
    /// Categories that are considered bridges
    bridge_categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StructuringRule {
    /// Minimum structuring score to trigger the tag
    min_structuring_score: f32,
    /// Categories that are considered exchanges
    cex_categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DrainerConsolidationRule {
    /// Categories of funding sources to consider as drainer victims
    victim_categories: Vec<String>,
    /// Minimum ratio of funding from victim categories
    min_funding_ratio: f32,
}

/// Context built from historical wallet data
#[allow(dead_code)] // Fields will be used in full implementation
struct HistoricalWalletContext {
    /// Address being analyzed
    address: String,
    /// Total SOL volume transferred in
    total_sol_volume_in: f64,
    /// Total SOL volume transferred out
    total_sol_volume_out: f64,
    /// Set of distinct counterparty addresses funds were received from
    /// (excludes token accounts owned by the analyzed wallet)
    funding_counterparties: HashMap<String, CounterpartyDetails>,
    /// Set of distinct counterparty addresses funds were sent to
    /// (excludes token accounts owned by the analyzed wallet)
    spending_counterparties: HashMap<String, CounterpartyDetails>,
    /// Heuristic flags computed from the aggregated data
    heuristic_flags: HeuristicFlags,
    /// Timestamps of all transactions for pattern detection
    transaction_timestamps: Vec<i64>,
    /// Raw transaction data by signature
    transactions: HashMap<String, TransactionDetails>,
}

/// Represents a token balance change
#[allow(dead_code)]
struct TokenChange {
    mint_address: String,
    amount: f64,
    is_incoming: bool,
    decimals: u8,
}

/// Details about a transaction
#[allow(dead_code)] // Fields will be used in full implementation
struct TransactionDetails {
    signature: String,
    timestamp: i64,
    counterparties: Vec<String>,
    is_incoming: bool,
    amount: f64,
    // Token changes associated with this transaction
    token_changes: Vec<TokenChange>,
}

/// Details about a counterparty
#[allow(dead_code)] // Fields will be used in full implementation
struct CounterpartyDetails {
    address: String,
    known_record: Option<AddressRecord>,
    total_amount: f64,
    interaction_count: usize,
    first_seen_at: i64,
    last_seen_at: i64,
}

impl Default for AnalyzeConfig {
    fn default() -> Self {
        Self {
            analyze_all: false,
            analyze_categories: None,
            analyze_risk_levels: None,
            analyze_addresses: None,
            max_history_transactions: Some(1000),
            max_history_days: Some(90),
            update_db_notes: true,
            max_concurrent_tasks: 5,
            exfiltration_rules: ExfiltrationRules {
                mixer_interaction: MixerRule {
                    min_risky_spending_ratio: 0.7,
                    mixer_categories: vec!["Mixer".to_string(), "Anonymizing Service".to_string()],
                },
                bridge_hopping: BridgeHoppingRule {
                    is_pass_through: true,
                    bridge_categories: vec!["Bridge".to_string(), "Cross-Chain Bridge".to_string()],
                },
                structuring: StructuringRule {
                    min_structuring_score: 0.6,
                    cex_categories: vec!["Exchange".to_string(), "CEX".to_string()],
                },
                drainer_consolidation: DrainerConsolidationRule {
                    victim_categories: vec![
                        "Drainer Victim".to_string(),
                        "Exploit Victim".to_string(),
                    ],
                    min_funding_ratio: 0.7,
                },
            },
        }
    }
}

impl HistoricalWalletContext {
    fn new(address: String) -> Self {
        Self {
            address,
            total_sol_volume_in: 0.0,
            total_sol_volume_out: 0.0,
            funding_counterparties: HashMap::new(),
            spending_counterparties: HashMap::new(),
            heuristic_flags: HeuristicFlags::default(),
            transaction_timestamps: Vec::new(),
            transactions: HashMap::new(),
        }
    }

    /// Add transaction information to the historical context
    fn add_transaction(
        &mut self,
        signature: &str,
        timestamp: i64,
        is_incoming: bool,
        counterparty: &str,
        amount: f64,
    ) {
        // Track transaction details - create an entry if it doesn't exist or retrieve existing one
        let tx_details = self
            .transactions
            .entry(signature.to_string())
            .or_insert_with(|| TransactionDetails {
                signature: signature.to_string(),
                timestamp,
                counterparties: Vec::new(),
                is_incoming,
                amount,
                token_changes: Vec::new(),
            });

        // Add counterparty if not already added
        if !tx_details
            .counterparties
            .contains(&counterparty.to_string())
        {
            tx_details.counterparties.push(counterparty.to_string());
        }

        // Add to transaction timestamps (for pattern detection) - only if this is the first time
        // we're recording this transaction to avoid duplicates
        if !self.transaction_timestamps.contains(&timestamp) {
            self.transaction_timestamps.push(timestamp);
        }

        if is_incoming {
            self.total_sol_volume_in += amount;
            // Log before adding to funding_counterparties
            info!(
                "Adding address to funding_counterparties: {} with amount: {}, timestamp: {}",
                counterparty, amount, timestamp
            );
            let entry = self
                .funding_counterparties
                .entry(counterparty.to_string())
                .or_insert_with(|| CounterpartyDetails {
                    address: counterparty.to_string(),
                    known_record: None,
                    total_amount: 0.0,
                    interaction_count: 0,
                    first_seen_at: timestamp,
                    last_seen_at: timestamp,
                });
            entry.total_amount += amount;
            entry.interaction_count += 1;
            entry.last_seen_at = timestamp;
        } else {
            self.total_sol_volume_out += amount;
            // Log before adding to spending_counterparties
            info!(
                "Adding address to spending_counterparties: {} with amount: {}, timestamp: {}",
                counterparty, amount, timestamp
            );
            let entry = self
                .spending_counterparties
                .entry(counterparty.to_string())
                .or_insert_with(|| CounterpartyDetails {
                    address: counterparty.to_string(),
                    known_record: None,
                    total_amount: 0.0,
                    interaction_count: 0,
                    first_seen_at: timestamp,
                    last_seen_at: timestamp,
                });
            entry.total_amount += amount;
            entry.interaction_count += 1;
            entry.last_seen_at = timestamp;
        }
    }

    /// Add token balance change information to a transaction
    fn add_token_change(
        &mut self,
        signature: &str,
        mint_address: &str,
        amount: f64,
        is_incoming: bool,
        decimals: u8,
    ) {
        // Ensure the transaction exists in our map
        if let Some(tx_details) = self.transactions.get_mut(signature) {
            // Add the token change information
            tx_details.token_changes.push(TokenChange {
                mint_address: mint_address.to_string(),
                amount,
                is_incoming,
                decimals,
            });
        }
    }

    /// Update counterparty details with the known address record, if available
    /// Returns true if the record was for an excluded category
    fn update_counterparty(&mut self, address: &str, record: Option<AddressRecord>) -> bool {
        // Check if this is a record we should exclude
        if let Some(ref addr_record) = record {
            let category_lowercase = addr_record.category.to_lowercase();
            let should_exclude = EXCLUDED_CATEGORIES
                .iter()
                .any(|excluded_cat| category_lowercase.contains(*excluded_cat));

            if should_exclude {
                // If this is a category we should exclude, don't add/update it
                // and return true to indicate this was filtered
                debug!(
                    "Filtered counterparty with excluded category: {} ({})",
                    address, addr_record.category
                );
                return true;
            }
        }

        // Update in funding counterparties if it exists there
        if let Some(counterparty) = self.funding_counterparties.get_mut(address) {
            counterparty.known_record = record.clone();
        }

        // Update in spending counterparties if it exists there
        if let Some(counterparty) = self.spending_counterparties.get_mut(address) {
            counterparty.known_record = record;
        }

        false // Not filtered
    }
}

/// Load configuration from file or create a default one
async fn load_or_create_config(config_path: &str) -> Result<AnalyzeConfig> {
    match std::fs::read_to_string(config_path) {
        Ok(content) => {
            // Parse TOML configuration
            let config: AnalyzeConfig = toml::from_str(&content)?;
            Ok(config)
        }
        Err(_) => {
            // Create default configuration
            let config = AnalyzeConfig::default();
            let toml_str = toml::to_string_pretty(&config)?;
            std::fs::write(config_path, toml_str)?;
            info!("Created default configuration at: {}", config_path);
            Ok(config)
        }
    }
}

/// Fetch transaction signatures for a wallet up to the specified limit or time window
async fn fetch_signatures_for_wallet(
    client: &RateLimitedClient,
    address: &str,
    max_transactions: Option<usize>,
    max_days: Option<u32>,
) -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>> {
    let mut all_signatures = Vec::new();
    let max_transactions = max_transactions.unwrap_or(1000);

    // Calculate the timestamp threshold if max_days is specified
    let max_days_timestamp = max_days.map(|days| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        now - ((days as i64) * 24 * 60 * 60)
    });

    // Fetch signatures in batches using pagination
    let mut before: Option<Signature> = None;

    loop {
        debug!("Fetching signatures for {} (before: {:?})", address, before);

        // Get up to 100 signatures per call
        let batch = client
            .get_signatures_for_address(address, before.as_ref(), None, Some(100))
            .await
            .with_context(|| format!("Failed to fetch signatures for address: {}", address))?;

        if batch.is_empty() {
            debug!("No more signatures found for {}", address);
            break;
        }

        // Update the "before" signature for the next iteration
        before = batch
            .last()
            .map(|sig| Signature::from_str(&sig.signature).ok())
            .flatten();

        // Process this batch of signatures
        for sig in &batch {
            // Check if we've reached the maximum days threshold
            if let Some(threshold) = max_days_timestamp {
                if let Some(block_time) = sig.block_time {
                    if block_time < threshold {
                        debug!("Reached max days threshold for {}", address);
                        break;
                    }
                }
            }

            all_signatures.push(sig.clone());
        }

        // Check if we've reached the maximum transactions threshold
        if all_signatures.len() >= max_transactions {
            debug!("Reached max transactions threshold for {}", address);
            all_signatures.truncate(max_transactions);
            break;
        }

        // Check if there are no more signatures or we've reached the time threshold
        if batch.len() < 100 || before.is_none() {
            break;
        }
    }

    info!(
        "Fetched {} transaction signatures for {}",
        all_signatures.len(),
        address
    );

    Ok(all_signatures)
}

/// Apply exfiltration pattern rules to generate tags
/// Maps heuristic results to exfiltration pattern tags based on configured rules
///
/// This function implements the core tagging logic as defined in the task requirements.
/// It takes a HistoricalWalletContext containing aggregated transaction history and
/// heuristic flags, along with the ExfiltrationRules configuration, and returns a list
/// of exfiltration pattern tags that match the wallet's behavior.
///
/// # Arguments
///
/// * `context` - The HistoricalWalletContext containing wallet transaction history and heuristic results
/// * `rules` - The ExfiltrationRules containing thresholds and category mappings from config
///
/// # Returns
///
/// A Vec<String> containing all the exfiltration tags that apply to the wallet
///
/// # Tag Categories
///
/// * **Mixer Interaction**: Wallet sends funds to known mixer/anonymizing services
///   with high spending ratio to these services
/// * **Bridge Hopping**: Pass-through behavior involving cross-chain bridges
/// * **Structuring Outflow**: Structuring behavior detected when sending to exchanges
/// * **Drainer Consolidation**: Wallet receives funds primarily from hack/phishing victims
/// * **Peel Chain**: Sequential transactions moving funds through multiple addresses (can be disabled via feature flag)
/// * **Fund Churning**: Repeatedly moving funds between controlled addresses (can be disabled via feature flag)
/// * **Cross-Chain Exfiltration**: Using bridges to move funds across blockchains
/// * **Advanced Obfuscation**: Using sophisticated techniques to hide fund origins
/// * **Automated Exfiltration**: Regular timing patterns suggesting automated exfiltration
fn apply_exfiltration_rules(
    context: &HistoricalWalletContext,
    rules: &ExfiltrationRules,
) -> Vec<String> {
    let mut tags = Vec::new();
    let flags = &context.heuristic_flags;

    // Check for Mixer Interaction
    if flags.risky_spending_destination_ratio >= rules.mixer_interaction.min_risky_spending_ratio {
        // Check if any spending counterparties are mixers
        let has_mixer_interaction = context.spending_counterparties.values().any(|cp| {
            if let Some(record) = &cp.known_record {
                rules
                    .mixer_interaction
                    .mixer_categories
                    .contains(&record.category)
            } else {
                false
            }
        });

        if has_mixer_interaction {
            tags.push("Mixer Interaction".to_string());
        }
    }

    // Check for Bridge Hopping
    if flags.is_pass_through == rules.bridge_hopping.is_pass_through {
        // Check if any counterparties are bridges
        let has_bridge_interaction = context.spending_counterparties.values().any(|cp| {
            if let Some(record) = &cp.known_record {
                rules
                    .bridge_hopping
                    .bridge_categories
                    .contains(&record.category)
            } else {
                false
            }
        });

        if has_bridge_interaction {
            tags.push("Potential Bridge Hopping".to_string());
        }
    }

    // Check for Structuring Outflow
    if flags.structuring_score >= rules.structuring.min_structuring_score {
        // Check if any spending counterparties are exchanges
        let has_cex_interaction = context.spending_counterparties.values().any(|cp| {
            if let Some(record) = &cp.known_record {
                rules.structuring.cex_categories.contains(&record.category)
            } else {
                false
            }
        });

        if has_cex_interaction {
            tags.push("Structuring Outflow to CEX".to_string());
        }
    }

    // Check for Drainer Consolidation
    if !context.funding_counterparties.is_empty() {
        let mut victim_funding_amount = 0.0;

        for cp in context.funding_counterparties.values() {
            if let Some(record) = &cp.known_record {
                if rules
                    .drainer_consolidation
                    .victim_categories
                    .contains(&record.category)
                {
                    victim_funding_amount += cp.total_amount;
                }
            }
        }

        let victim_funding_ratio = if context.total_sol_volume_in > 0.0 {
            (victim_funding_amount / context.total_sol_volume_in) as f32
        } else {
            0.0f32
        };

        if victim_funding_ratio >= rules.drainer_consolidation.min_funding_ratio {
            tags.push("Potential Drainer Consolidation".to_string());
        }
    }

    // Define safe trading categories where transactions should not be considered suspicious
    const SAFE_TRADING_CATEGORIES: [&str; 5] = [
        "DEX",
        "Exchange",
        "DEX Aggregator",
        "Stableswap",
        "AMM", // Automated Market Maker
    ];

    // Calculate the percentage of interactions involving safe trading categories
    let mut total_interactions = 0;
    let mut safe_category_interactions = 0;

    // Check funding counterparties (incoming transactions)
    for cp in context.funding_counterparties.values() {
        total_interactions += cp.interaction_count;
        if let Some(record) = &cp.known_record {
            if SAFE_TRADING_CATEGORIES
                .iter()
                .any(|&safe_cat| record.category == safe_cat)
            {
                safe_category_interactions += cp.interaction_count;
            }
        }
    }

    // Check spending counterparties (outgoing transactions)
    for cp in context.spending_counterparties.values() {
        total_interactions += cp.interaction_count;
        if let Some(record) = &cp.known_record {
            if SAFE_TRADING_CATEGORIES
                .iter()
                .any(|&safe_cat| record.category == safe_cat)
            {
                safe_category_interactions += cp.interaction_count;
            }
        }
    }

    // Calculate the ratio of interactions with safe categories
    let safe_trading_ratio = if total_interactions > 0 {
        safe_category_interactions as f32 / total_interactions as f32
    } else {
        0.0
    };

    debug!(
        "Safe trading category ratio: {:.2} ({} out of {} interactions)",
        safe_trading_ratio, safe_category_interactions, total_interactions
    );

    // Define the threshold for "predominantly" involving safe categories
    // If more than 50% of interactions involve safe categories, consider it legitimate activity
    const SAFE_TRADING_THRESHOLD: f32 = 0.50;
    let predominantly_safe = safe_trading_ratio >= SAFE_TRADING_THRESHOLD;

    // Advanced historical pattern analysis tags
    // These come from specialized detection functions that add custom flags to the context

    // Check for Peel Chain Pattern
    // Sequential transactions that "peel off" small amounts while moving the bulk through new addresses
    // Only if this feature is enabled in feature flags
    let feature_flags = Config::from_env().load_feature_flags().unwrap_or_default();

    if feature_flags
        .analysis_features
        .peel_chain_exfiltration_enabled
    {
        if let Some(peel_chain_score) = flags.custom_flags.get("peel_chain_pattern") {
            if *peel_chain_score > 0.5 && !predominantly_safe {
                debug!(
                    "Adding 'Peel Chain Exfiltration' tag with score: {:.2} (safe trading ratio: {:.2})",
                    *peel_chain_score, safe_trading_ratio
                );
                tags.push("Peel Chain Exfiltration".to_string());
            } else if *peel_chain_score > 0.5 {
                debug!(
                    "Suppressed 'Peel Chain Exfiltration' tag with score: {:.2} due to high safe trading ratio: {:.2}",
                    *peel_chain_score, safe_trading_ratio
                );
            }
        }
    } else {
        // Feature is disabled
        if flags.custom_flags.get("peel_chain_pattern").is_some() {
            debug!(
                "Peel Chain Exfiltration analysis is disabled via feature flag. Skipping analysis."
            );
        }
    }

    // Check for Churning Pattern
    // Repeatedly moving funds between addresses likely controlled by the same entity
    // Only if this feature is enabled in feature flags
    if feature_flags.analysis_features.fund_churning_enabled {
        if let Some(churning_score) = flags.custom_flags.get("churning_pattern") {
            if *churning_score > 0.5 && !predominantly_safe {
                debug!(
                    "Adding 'Fund Churning' tag with score: {:.2} (safe trading ratio: {:.2})",
                    *churning_score, safe_trading_ratio
                );
                tags.push("Fund Churning".to_string());
            } else if *churning_score > 0.5 {
                debug!(
                    "Suppressed 'Fund Churning' tag with score: {:.2} due to high safe trading ratio: {:.2}",
                    *churning_score, safe_trading_ratio
                );
            }
        }
    } else {
        // Feature is disabled
        if flags.custom_flags.get("churning_pattern").is_some() {
            debug!("Fund Churning analysis is disabled via feature flag. Skipping analysis.");
        }
    }

    // Check for Cross-Chain Transfers
    // Moving funds across blockchain networks, often to evade tracking
    if let Some(cross_chain_score) = flags.custom_flags.get("cross_chain_transfer") {
        if *cross_chain_score > 0.5 && !predominantly_safe {
            debug!(
                "Adding 'Cross-Chain Exfiltration' tag with score: {:.2} (safe trading ratio: {:.2})",
                *cross_chain_score, safe_trading_ratio
            );
            tags.push("Cross-Chain Exfiltration".to_string());
        } else if *cross_chain_score > 0.5 {
            debug!(
                "Suppressed 'Cross-Chain Exfiltration' tag with score: {:.2} due to high safe trading ratio: {:.2}",
                *cross_chain_score, safe_trading_ratio
            );
        }
    }

    // Check for Obfuscation Techniques
    // Sophisticated methods to hide fund origins beyond simple mixing
    if let Some(obfuscation_score) = flags.custom_flags.get("obfuscation_techniques") {
        if *obfuscation_score > 0.7 && !predominantly_safe {
            // Higher threshold for this complex pattern
            debug!(
                "Adding 'Advanced Obfuscation Techniques' tag with score: {:.2} (safe trading ratio: {:.2})",
                *obfuscation_score, safe_trading_ratio
            );
            tags.push("Advanced Obfuscation Techniques".to_string());
        } else if *obfuscation_score > 0.7 {
            debug!(
                "Suppressed 'Advanced Obfuscation Techniques' tag with score: {:.2} due to high safe trading ratio: {:.2}",
                *obfuscation_score, safe_trading_ratio
            );
        }
    }

    // Check for Temporal Patterns (potential automation)
    // Regular timing patterns suggesting automated scripts for fund movement
    if let Some(temporal_score) = flags.custom_flags.get("temporal_pattern") {
        if *temporal_score > 0.7 && !predominantly_safe {
            debug!(
                "Adding 'Automated Exfiltration Pattern' tag with score: {:.2} (safe trading ratio: {:.2})",
                *temporal_score, safe_trading_ratio
            );
            tags.push("Automated Exfiltration Pattern".to_string());
        } else if *temporal_score > 0.7 {
            debug!(
                "Suppressed 'Automated Exfiltration Pattern' tag with score: {:.2} due to high safe trading ratio: {:.2}",
                *temporal_score, safe_trading_ratio
            );
        }
    }

    // Special debugging for specific wallets of interest - log detailed heuristic values
    // even if they don't meet the thresholds for tagging
    let wallets_of_interest = [
        "HAmHyzLsmNWm1gTgAP5GB9T7NbYsb6MrQgEm32kbKvcC",
        "C5rJvSNvUAFBpHZQZQcXPPsqwzW1tZhZAoYWS1Lh7VUZ",
        "4sJndLUFzt7hPqVKqpJK63WBQ8CcyHDXizQ9ek1Xnzs9",
    ];

    if wallets_of_interest.contains(&context.address.as_str()) {
        info!(
            "Analysis for watched wallet {}: structuring_score={:.2}, is_pass_through={}, risky_spending_ratio={:.2}, custom_flags={:?}",
            context.address,
            flags.structuring_score,
            flags.is_pass_through,
            flags.risky_spending_destination_ratio,
            flags.custom_flags
        );

        // For these special wallets, add tags even if they're just below the threshold
        // to help identify patterns that are close to detection limits
        if flags.structuring_score >= 0.3
            && flags.structuring_score < rules.structuring.min_structuring_score
        {
            tags.push("Below-Threshold Structuring Behavior".to_string());
        }

        if flags.risky_spending_destination_ratio >= 0.4
            && flags.risky_spending_destination_ratio
                < rules.mixer_interaction.min_risky_spending_ratio
        {
            tags.push("Below-Threshold Suspicious Spending".to_string());
        }
    }

    // Log the identified tags if any were found
    if !tags.is_empty() {
        debug!(
            "Identified exfiltration tags for {}: {:?}",
            context.address, tags
        );
    }

    tags
}

/// Apply advanced heuristics to a wallet's historical context to detect exfiltration patterns
///
/// This function analyzes the aggregated historical data for a wallet and applies various
/// heuristics designed to detect exfiltration patterns such as peel chains, churning, and
/// cross-chain transfers.
///
/// # Arguments
///
/// * `context` - The aggregated historical context for the wallet
fn apply_historical_heuristics(context: &mut HistoricalWalletContext) {
    // Apply existing heuristics to historical data
    apply_existing_heuristics(context);

    // Apply new exfiltration-specific heuristics
    detect_peel_chain_pattern(context);
    detect_temporal_patterns(context);
    detect_churning_pattern(context);
    detect_cross_chain_transfers(context);
    detect_obfuscation_techniques(context);
}

/// Apply existing heuristics from the wallet_heuristics and transaction_heuristics modules
/// but adapted to work with the full historical context
fn apply_existing_heuristics(context: &mut HistoricalWalletContext) {
    // Get references to existing fields for convenience
    let transactions = &context.transactions;
    let funding = &context.funding_counterparties;
    let spending = &context.spending_counterparties;

    // Re-apply structuring pattern detection with historical data
    if !transactions.is_empty() {
        // Constants from wallet_heuristics.rs
        const MIN_TRANSACTIONS_FOR_STRUCTURING: usize = 5;
        const SIMILAR_AMOUNT_THRESHOLD_RATIO: f64 = 0.10; // 10% difference
        const SMALL_TX_AMOUNT_THRESHOLD: f64 = 0.5; // 0.5 SOL

        // Define safe trading categories where transactions should not be considered suspicious
        const SAFE_TRADING_CATEGORIES: [&str; 5] =
            ["DEX", "Exchange", "DEX Aggregator", "Stableswap", "AMM"];

        if transactions.len() >= MIN_TRANSACTIONS_FOR_STRUCTURING {
            // Group transactions by similar amounts
            let mut amount_buckets: Vec<(f64, usize)> = Vec::new();
            let mut small_tx_count = 0;
            let mut filtered_tx_count = 0;
            let mut total_tx_count = 0;
            let mut safe_trading_txs: HashMap<String, String> = HashMap::new(); // Store signature -> category for debug
            let mut non_filtered_txs: Vec<(&String, &TransactionDetails)> = Vec::new();

            for (signature, tx) in transactions {
                total_tx_count += 1;

                // Check if any counterparty in this transaction belongs to a safe category
                let mut involves_safe_category = false;
                let mut safe_category = String::new();

                for counterparty in &tx.counterparties {
                    if let Some(counterparty_details) = context
                        .spending_counterparties
                        .get(counterparty)
                        .or_else(|| context.funding_counterparties.get(counterparty))
                    {
                        if let Some(record) = &counterparty_details.known_record {
                            if SAFE_TRADING_CATEGORIES
                                .iter()
                                .any(|&safe_cat| record.category == safe_cat)
                            {
                                involves_safe_category = true;
                                safe_category = record.category.clone();
                                filtered_tx_count += 1;
                                break;
                            }
                        }
                    }
                }

                if involves_safe_category {
                    safe_trading_txs.insert(signature.clone(), safe_category);
                    continue; // Skip this transaction in our structuring analysis
                }

                // Add to our list of transactions to analyze
                non_filtered_txs.push((signature, tx));
            }

            // Log detailed information about excluded transactions
            for (signature, category) in &safe_trading_txs {
                debug!(
                    "Excluding transaction {} from structuring analysis as it involves a legitimate trading venue: {}",
                    signature, category
                );
            }

            debug!(
                "Structuring analysis: filtered {}/{} transactions involving safe trading categories ({:.1}%)",
                filtered_tx_count,
                total_tx_count,
                if total_tx_count > 0 {
                    (filtered_tx_count as f32 / total_tx_count as f32) * 100.0
                } else {
                    0.0
                }
            );

            // Process remaining transactions that don't involve safe trading venues
            for (signature, tx) in &non_filtered_txs {
                let amount = if tx.is_incoming { 0.0 } else { tx.amount }; // Only consider outgoing

                if amount <= SMALL_TX_AMOUNT_THRESHOLD && amount > 0.0 {
                    small_tx_count += 1;
                    debug!(
                        "Small transaction detected: {} amount: {:.4} SOL",
                        signature, amount
                    );
                }

                // Round to 2 decimal places for grouping
                let rounded = (amount * 100.0).round() / 100.0;

                // Find or create bucket
                let mut found = false;
                for bucket in &mut amount_buckets {
                    if (bucket.0 - rounded).abs() < SIMILAR_AMOUNT_THRESHOLD_RATIO * bucket.0 {
                        bucket.1 += 1;
                        found = true;
                        break;
                    }
                }

                if !found {
                    amount_buckets.push((rounded, 1));
                }
            }

            // Calculate structuring score based on:
            // 1. Percentage of small transactions
            // 2. Repetition of same/similar amounts

            // Skip the structuring score calculation if we don't have enough transactions after filtering
            let non_filtered_tx_count = total_tx_count - filtered_tx_count;
            if non_filtered_tx_count < MIN_TRANSACTIONS_FOR_STRUCTURING {
                debug!(
                    "Not enough transactions (only {}) after filtering safe trading categories for structuring analysis (minimum required: {})",
                    non_filtered_tx_count, MIN_TRANSACTIONS_FOR_STRUCTURING
                );
                // Reset structuring score since most transactions are with legitimate trading venues
                context.heuristic_flags.structuring_score = 0.0;
                return;
            }

            let small_tx_ratio = if non_filtered_tx_count > 0 {
                small_tx_count as f32 / non_filtered_tx_count as f32
            } else {
                0.0
            };

            // Find groups with multiple similar transactions
            let similar_amount_groups = amount_buckets
                .iter()
                .filter(|&(_, count)| *count >= 3)
                .count();

            // Log buckets with similar amounts for debugging
            if !amount_buckets.is_empty() {
                for (amount, count) in &amount_buckets {
                    if *count >= 3 {
                        debug!(
                            "Similar amount bucket detected: {:.4} SOL, count: {}",
                            amount, count
                        );
                    }
                }
            }

            let similar_amount_ratio = if amount_buckets.is_empty() {
                0.0
            } else {
                similar_amount_groups as f32 / amount_buckets.len() as f32
            };

            // Apply a reduction factor based on the percentage of safe trading transactions
            // The more legitimate trading transactions, the lower the final score should be
            let safe_trading_ratio = if total_tx_count > 0 {
                filtered_tx_count as f32 / total_tx_count as f32
            } else {
                0.0
            };

            // Reduction factor: higher percentage of safe trading = more reduction
            let reduction_factor = (1.0 - safe_trading_ratio).max(0.1); // Never reduce by more than 90%

            // Combine factors with weights
            let raw_structuring_score =
                (small_tx_ratio * 0.7 + similar_amount_ratio * 0.3).min(1.0);

            // Apply the reduction based on safe trading transactions
            let structuring_score = raw_structuring_score * reduction_factor;

            // Only update if the new score is higher than existing
            context.heuristic_flags.structuring_score = context
                .heuristic_flags
                .structuring_score
                .max(structuring_score);

            debug!(
                "Historical structuring analysis: small_tx_ratio={:.2}, similar_amount_ratio={:.2}, safe_trading_ratio={:.2}, reduction_factor={:.2}, raw_score={:.2}, final_score={:.2}, tx_analyzed={}/{}",
                small_tx_ratio,
                similar_amount_ratio,
                safe_trading_ratio,
                reduction_factor,
                raw_structuring_score,
                structuring_score,
                non_filtered_tx_count,
                total_tx_count
            );
        }
    }

    // Re-apply pass-through detection with historical data
    if !funding.is_empty() && !spending.is_empty() {
        // Constants
        const QUICK_TRANSFER_THRESHOLD: i64 = 3600; // 1 hour in seconds
        const SIMILAR_AMOUNT_THRESHOLD: f64 = 0.90; // 90% of funds transferred

        // Check if significant portion of incoming funds were quickly sent out
        let mut quick_transfer_volume = 0.0;
        let mut matched_transfers = 0;

        // For each incoming transaction, check if there's an outgoing one soon after
        for funding_cp in funding.values() {
            let funding_amount = funding_cp.total_amount;

            // Find all spending transactions that happened shortly after this funding
            for spending_cp in spending.values() {
                // Compare first seen of spending with last seen of funding
                if spending_cp.first_seen_at - funding_cp.last_seen_at <= QUICK_TRANSFER_THRESHOLD {
                    let transfer_amount = spending_cp.total_amount.min(funding_amount);
                    quick_transfer_volume += transfer_amount;
                    matched_transfers += 1;
                }
            }
        }

        // Calculate what percentage of total incoming funds were quickly transferred out
        let pass_through_ratio = if context.total_sol_volume_in > 0.0 {
            quick_transfer_volume / context.total_sol_volume_in
        } else {
            0.0
        };

        // Update pass_through flag if significant funds were quickly moved
        if pass_through_ratio >= SIMILAR_AMOUNT_THRESHOLD && matched_transfers >= 2 {
            context.heuristic_flags.is_pass_through = true;

            debug!(
                "Historical pass-through analysis: ratio={:.2}, matched_transfers={}, flag=true",
                pass_through_ratio, matched_transfers
            );
        }
    }

    // Analyze funding sources with historical context
    analyze_historical_funding_sources(context);

    // Analyze spending destinations with historical context
    analyze_historical_spending_destinations(context);
}

/// Analyzes funding sources over the entire historical period
fn analyze_historical_funding_sources(context: &mut HistoricalWalletContext) {
    let funding = &context.funding_counterparties;
    if funding.is_empty() {
        return;
    }

    let mut high_risk_funding_amount = 0.0;

    for cp in funding.values() {
        if let Some(record) = &cp.known_record {
            if record.risk_level == "High" || record.risk_level == "Critical" {
                high_risk_funding_amount += cp.total_amount;
            }
        }
    }

    // Calculate ratio of funds coming from high-risk sources
    let risky_funding_ratio = if context.total_sol_volume_in > 0.0 {
        (high_risk_funding_amount / context.total_sol_volume_in) as f32
    } else {
        0.0
    };

    // Update the heuristic flag
    context.heuristic_flags.risky_funding_source_ratio = risky_funding_ratio;

    debug!(
        "Historical funding source analysis: high_risk_amount={:.2}, total_in={:.2}, ratio={:.2}",
        high_risk_funding_amount, context.total_sol_volume_in, risky_funding_ratio
    );
}

/// Analyzes spending destinations over the entire historical period
fn analyze_historical_spending_destinations(context: &mut HistoricalWalletContext) {
    let spending = &context.spending_counterparties;
    if spending.is_empty() {
        return;
    }

    let mut high_risk_spending_amount = 0.0;

    for cp in spending.values() {
        if let Some(record) = &cp.known_record {
            if record.risk_level == "High" || record.risk_level == "Critical" {
                high_risk_spending_amount += cp.total_amount;
            }
        }
    }

    // Calculate ratio of funds going to high-risk destinations
    let risky_spending_ratio = if context.total_sol_volume_out > 0.0 {
        (high_risk_spending_amount / context.total_sol_volume_out) as f32
    } else {
        0.0
    };

    // Update the heuristic flag
    context.heuristic_flags.risky_spending_destination_ratio = risky_spending_ratio;

    debug!(
        "Historical spending analysis: high_risk_amount={:.2}, total_out={:.2}, ratio={:.2}",
        high_risk_spending_amount, context.total_sol_volume_out, risky_spending_ratio
    );
}

/// Detect peel chain patterns in the transaction history
///
/// A peel chain is a series of transactions where funds are "peeled" off gradually
/// from a larger amount, with the remainder going to a new address controlled by the same entity.
/// This is a common technique used to obscure fund movements.
fn detect_peel_chain_pattern(context: &mut HistoricalWalletContext) {
    // Constants
    const MIN_CHAIN_LENGTH: usize = 2; // Minimum transactions to form a peel chain
    const MAX_TIME_BETWEEN_PEELS: i64 = 7200; // 2 hours max between peels

    // Define safe trading categories where transactions should not be considered suspicious
    const SAFE_TRADING_CATEGORIES: [&str; 5] = [
        "DEX",
        "Exchange",
        "DEX Aggregator",
        "Stableswap",
        "AMM", // Automated Market Maker
    ];

    let tx_by_time = context.transaction_timestamps.clone();
    if tx_by_time.len() < MIN_CHAIN_LENGTH {
        return;
    }

    // Sort transactions by timestamp
    let mut sorted_timestamps = tx_by_time.clone();
    sorted_timestamps.sort();

    // Look for sequences of transactions where:
    // 1. A smaller amount is sent out
    // 2. A larger remainder amount is sent to a new address
    // 3. This pattern repeats several times

    let mut potential_peels = 0;
    let mut chain_lengths: Vec<usize> = Vec::new();
    let mut current_chain_length = 1;

    for i in 1..sorted_timestamps.len() {
        let curr_timestamp = sorted_timestamps[i];
        let prev_timestamp = sorted_timestamps[i - 1];

        // If transactions are close in time
        if curr_timestamp - prev_timestamp <= MAX_TIME_BETWEEN_PEELS {
            // Find the transactions associated with these timestamps
            let mut involves_safe_category = false;

            // Check if this transaction involves counterparties in safe categories
            if let Some(tx_details) = context
                .transactions
                .values()
                .find(|tx| tx.timestamp == curr_timestamp)
            {
                for counterparty in &tx_details.counterparties {
                    // Look up counterparty details from funding or spending counterparties
                    let counterparty_details = context
                        .funding_counterparties
                        .get(counterparty)
                        .or_else(|| context.spending_counterparties.get(counterparty));

                    if let Some(details) = counterparty_details {
                        if let Some(record) = &details.known_record {
                            // Check if this counterparty belongs to a safe trading category
                            if SAFE_TRADING_CATEGORIES
                                .iter()
                                .any(|&safe_cat| record.category.contains(safe_cat))
                            {
                                involves_safe_category = true;
                                debug!(
                                    "Skipping transaction in peel chain analysis: tx timestamp={}, counterparty={}, category={}",
                                    curr_timestamp, counterparty, record.category
                                );
                                break;
                            }
                        }
                    }
                }
            }

            // Only increment chain length if this isn't involving a safe category
            if !involves_safe_category {
                current_chain_length += 1;
            } else {
                // Skip this transaction for chain calculation purposes
                // but preserve the current chain - we're not breaking the chain,
                // just not counting this particular transaction
                debug!(
                    "Continuing chain without incrementing length due to safe trading category transaction at timestamp={}",
                    curr_timestamp
                );
                continue;
            }
        } else {
            // Chain broken by time gap
            if current_chain_length >= MIN_CHAIN_LENGTH {
                chain_lengths.push(current_chain_length);
                potential_peels += 1;
            }
            current_chain_length = 1;
        }
    }

    // Check the final chain
    if current_chain_length >= MIN_CHAIN_LENGTH {
        chain_lengths.push(current_chain_length);
        potential_peels += 1;
    }

    // Calculate peel chain score based on the number and length of potential chains
    if potential_peels > 0 {
        let avg_chain_length =
            chain_lengths.iter().sum::<usize>() as f32 / chain_lengths.len() as f32;
        let longest_chain = *chain_lengths.iter().max().unwrap_or(&0);

        // Score is based on number of chains and their average length
        // Modified to be more sensitive to shorter chains
        let peel_chain_score =
            ((potential_peels as f32) * 0.4 + (avg_chain_length / 6.0) * 0.6).min(1.0);

        // Add as a custom flag if the feature is enabled
        let feature_flags = Config::from_env().load_feature_flags().unwrap_or_default();
        if feature_flags
            .analysis_features
            .peel_chain_exfiltration_enabled
        {
            context
                .heuristic_flags
                .custom_flags
                .insert("peel_chain_pattern".to_string(), peel_chain_score);

            debug!(
                "Peel chain analysis: potential_chains={}, avg_length={:.1}, longest={}, score={:.2}",
                potential_peels, avg_chain_length, longest_chain, peel_chain_score
            );
        } else {
            debug!(
                "Peel chain analysis computed but not inserted (feature disabled): potential_chains={}, avg_length={:.1}, score={:.2}",
                potential_peels, avg_chain_length, peel_chain_score
            );
        }
    } else {
        // Remove the flag if it exists but doesn't meet the thresholds after filtering
        context
            .heuristic_flags
            .custom_flags
            .remove("peel_chain_pattern");
    }
}

/// Detect temporal patterns in transactions that might indicate automation or coordination
fn detect_temporal_patterns(context: &mut HistoricalWalletContext) {
    let timestamps = &context.transaction_timestamps;
    if timestamps.len() < 5 {
        // Need at least 5 transactions for meaningful pattern analysis
        return;
    }

    // Sort timestamps
    let mut sorted_timestamps = timestamps.clone();
    sorted_timestamps.sort();

    // Calculate time differences between consecutive transactions
    let mut time_diffs: Vec<i64> = Vec::with_capacity(sorted_timestamps.len() - 1);
    for i in 1..sorted_timestamps.len() {
        time_diffs.push(sorted_timestamps[i] - sorted_timestamps[i - 1]);
    }

    // Check for regular intervals (potential automation)
    let mut similar_intervals = 0;
    let mut has_regular_pattern = false;

    // Group similar intervals (within 10% of each other)
    let mut interval_groups: HashMap<i64, usize> = HashMap::new();
    for diff in &time_diffs {
        // Round to nearest minute for grouping
        let rounded_diff = (diff / 60) * 60;
        *interval_groups.entry(rounded_diff).or_default() += 1;
    }

    // Find the most common interval
    if let Some(max_entry) = interval_groups.iter().max_by_key(|&(_, count)| *count) {
        let max_count = *max_entry.1;
        let pattern_ratio = max_count as f32 / time_diffs.len() as f32;

        // If more than 40% of intervals are similar, it might indicate automated behavior
        if pattern_ratio > 0.4 && max_count >= 3 {
            has_regular_pattern = true;
            similar_intervals = max_count;
        }
    }

    // Check for time-of-day patterns
    // Convert timestamps to hours of day
    let hours_of_day: Vec<u8> = sorted_timestamps
        .iter()
        .map(|&ts| {
            // Convert timestamp to hour of day (0-23)
            let hour = (ts % 86400) / 3600;
            hour as u8
        })
        .collect();

    // Count transactions by hour
    let mut hour_counts = [0u32; 24];
    for hour in hours_of_day {
        hour_counts[hour as usize] += 1;
    }

    // Find the hours with the most transactions
    let max_hour_count = *hour_counts.iter().max().unwrap_or(&0);
    let time_of_day_ratio = max_hour_count as f32 / timestamps.len() as f32;

    // Combined temporal pattern score
    let regular_interval_score = if has_regular_pattern {
        (similar_intervals as f32 / time_diffs.len() as f32).min(1.0)
    } else {
        0.0
    };

    let time_of_day_score = if time_of_day_ratio > 0.5 && timestamps.len() >= 10 {
        time_of_day_ratio
    } else {
        0.0
    };

    // Combined score with higher weight on regular intervals
    let temporal_pattern_score = (regular_interval_score * 0.7 + time_of_day_score * 0.3).min(1.0);

    // Only add if we detected something significant
    if temporal_pattern_score > 0.3 {
        context
            .heuristic_flags
            .custom_flags
            .insert("temporal_pattern".to_string(), temporal_pattern_score);

        debug!(
            "Temporal pattern analysis: regular_intervals={}, pattern_ratio={:.2}, time_score={:.2}, final_score={:.2}",
            similar_intervals, regular_interval_score, time_of_day_score, temporal_pattern_score
        );
    }
}

/// Detect churning patterns where funds are moved between controlled addresses
/// to obscure the trail of funds
fn detect_churning_pattern(context: &mut HistoricalWalletContext) {
    // 2.1: Define safe trading categories that should be excluded from churning detection
    const SAFE_TRADING_CATEGORIES: [&str; 5] = [
        "DEX",
        "Exchange",
        "DEX Aggregator",
        "Stableswap",
        "AMM", // Automated Market Maker
    ];

    let funding = &context.funding_counterparties;
    let spending = &context.spending_counterparties;

    // We need both funding and spending to detect churning
    if funding.is_empty() || spending.is_empty() {
        return;
    }

    // Look for addresses that appear in both funding and spending
    // but exclude addresses that belong to legitimate trading venues
    let mut bidirectional_addresses: HashSet<String> = HashSet::new();
    let mut bidirectional_volume = 0.0;

    // 2.2: Filter out addresses from safe trading categories
    for (addr, funding_details) in funding.iter() {
        // Skip if this address isn't also in spending
        if !spending.contains_key(addr) {
            continue;
        }

        // Skip if this is a legitimate trading venue (from funding side)
        if let Some(record) = &funding_details.known_record {
            if SAFE_TRADING_CATEGORIES.contains(&record.category.as_str()) {
                debug!(
                    "Skipping funding counterparty {} as it's a legitimate trading venue ({})",
                    addr, record.category
                );
                continue;
            }
        }

        // Also check from spending side
        if let Some(spending_details) = spending.get(addr) {
            if let Some(record) = &spending_details.known_record {
                if SAFE_TRADING_CATEGORIES.contains(&record.category.as_str()) {
                    debug!(
                        "Skipping spending counterparty {} as it's a legitimate trading venue ({})",
                        addr, record.category
                    );
                    continue;
                }
            }
        }

        // This address passed all filters, add it to bidirectional addresses
        bidirectional_addresses.insert(addr.clone());
    }

    // 2.3: Calculate volume only for filtered addresses
    for addr in &bidirectional_addresses {
        if let Some(funding_details) = funding.get(addr) {
            bidirectional_volume += funding_details.total_amount;
        }

        if let Some(spending_details) = spending.get(addr) {
            bidirectional_volume += spending_details.total_amount;
        }
    }

    // Calculate what percentage of total volume is involved in bidirectional transfers
    let total_volume = context.total_sol_volume_in + context.total_sol_volume_out;
    let churning_ratio = if total_volume > 0.0 {
        bidirectional_volume / total_volume
    } else {
        0.0
    };

    // 2.4: Calculate final churning score based on filtered results
    let bidirectional_factor = (bidirectional_addresses.len() as f32).min(5.0) / 5.0;
    let churning_score = (churning_ratio as f32 * 0.7 + bidirectional_factor * 0.3).min(1.0);

    // 2.5: Update or remove the churning_pattern flag based on refined score
    if churning_score > 0.2 && bidirectional_addresses.len() >= 2 {
        // Only add churning pattern if feature is enabled
        let feature_flags = Config::from_env().load_feature_flags().unwrap_or_default();
        if feature_flags.analysis_features.fund_churning_enabled {
            context
                .heuristic_flags
                .custom_flags
                .insert("churning_pattern".to_string(), churning_score);

            debug!(
                "Churning pattern analysis: bidirectional_addresses={}, volume={:.2}, ratio={:.2}, score={:.2}",
                bidirectional_addresses.len(),
                bidirectional_volume,
                churning_ratio,
                churning_score
            );
        } else {
            debug!(
                "Churning pattern analysis computed but not inserted (feature disabled): bidirectional_addresses={}, volume={:.2}, score={:.2}",
                bidirectional_addresses.len(),
                bidirectional_volume,
                churning_score
            );
        }
    } else {
        // Remove the flag if it exists but doesn't meet the thresholds after filtering
        context
            .heuristic_flags
            .custom_flags
            .remove("churning_pattern");

        debug!(
            "No significant churning pattern detected after filtering legitimate venues: addresses={}, volume={:.2}, ratio={:.2}, score={:.2}",
            bidirectional_addresses.len(),
            bidirectional_volume,
            churning_ratio,
            churning_score
        );
    }
}

/// Detect potential cross-chain transfers through bridge services
fn detect_cross_chain_transfers(context: &mut HistoricalWalletContext) {
    // List of categories that indicate cross-chain services
    const BRIDGE_CATEGORIES: [&str; 5] = [
        "Bridge",
        "Cross-Chain Bridge",
        "Token Bridge",
        "Chain Bridge",
        "Cross-Chain Service",
    ];

    let spending = &context.spending_counterparties;

    // Count interactions with bridge services
    let mut bridge_interactions = 0;
    let mut bridge_volume = 0.0;

    for details in spending.values() {
        if let Some(record) = &details.known_record {
            let category = record.category.to_lowercase();
            if BRIDGE_CATEGORIES
                .iter()
                .any(|&c| category.contains(&c.to_lowercase()))
            {
                bridge_interactions += 1;
                bridge_volume += details.total_amount;
            }
        }
    }

    // Calculate what percentage of outgoing funds went to bridge services
    let bridge_ratio = if context.total_sol_volume_out > 0.0 {
        bridge_volume / context.total_sol_volume_out
    } else {
        0.0
    };

    // Only add if we found significant bridge interaction
    if bridge_interactions > 0 && bridge_ratio > 0.1 {
        // Score based on volume ratio and number of interactions
        let cross_chain_score = (bridge_ratio as f32 * 0.6
            + (bridge_interactions as f32).min(5.0) / 5.0 * 0.4)
            .min(1.0);

        context
            .heuristic_flags
            .custom_flags
            .insert("cross_chain_transfer".to_string(), cross_chain_score);

        debug!(
            "Cross-chain transfer analysis: bridge_interactions={}, volume={:.2}, ratio={:.2}, score={:.2}",
            bridge_interactions, bridge_volume, bridge_ratio, cross_chain_score
        );
    }
}

/// Detect various obfuscation techniques that might indicate attempts to hide funds
fn detect_obfuscation_techniques(context: &mut HistoricalWalletContext) {
    // Check for common obfuscation indicators
    const PRIVACY_CATEGORIES: [&str; 8] = [
        "Mixer",
        "Tumbler",
        "Privacy Protocol",
        "Anonymous Service",
        "Privacy Service",
        "Obfuscation Service",
        "Privacy Tool",
        "Anonymizing Service",
    ];

    let spending = &context.spending_counterparties;

    // Count interactions with privacy services
    let mut privacy_interactions = 0;
    let mut privacy_volume = 0.0;

    for details in spending.values() {
        if let Some(record) = &details.known_record {
            let category = record.category.to_lowercase();
            if PRIVACY_CATEGORIES
                .iter()
                .any(|&c| category.contains(&c.to_lowercase()))
            {
                privacy_interactions += 1;
                privacy_volume += details.total_amount;
            }
        }
    }

    // Calculate obfuscation score
    // Base factors
    let mut obfuscation_factors = Vec::new();

    // 1. Privacy service usage
    if privacy_interactions > 0 {
        let privacy_factor = if context.total_sol_volume_out > 0.0 {
            (privacy_volume / context.total_sol_volume_out) as f32
        } else {
            0.0
        };
        obfuscation_factors.push(privacy_factor);
    }

    // 2. Incorporate churning factor if detected (and feature is enabled)
    let feature_flags = Config::from_env().load_feature_flags().unwrap_or_default();

    if feature_flags.analysis_features.fund_churning_enabled {
        if let Some(churning_score) = context.heuristic_flags.custom_flags.get("churning_pattern") {
            obfuscation_factors.push(*churning_score);
        }
    }

    // 3. Incorporate peel chain factor if detected (and feature is enabled)
    if feature_flags
        .analysis_features
        .peel_chain_exfiltration_enabled
    {
        if let Some(peel_chain_score) = context
            .heuristic_flags
            .custom_flags
            .get("peel_chain_pattern")
        {
            obfuscation_factors.push(*peel_chain_score);
        }
    }

    // 4. High structuring score also indicates obfuscation
    if context.heuristic_flags.structuring_score > 0.5 {
        obfuscation_factors.push(context.heuristic_flags.structuring_score);
    }

    // Calculate final obfuscation score
    if !obfuscation_factors.is_empty() {
        // Average the factors but give more weight to privacy service usage
        let weighted_sum = obfuscation_factors.iter().sum::<f32>();
        let obfuscation_score = (weighted_sum / obfuscation_factors.len() as f32).min(1.0);

        if obfuscation_score > 0.3 {
            context
                .heuristic_flags
                .custom_flags
                .insert("obfuscation_techniques".to_string(), obfuscation_score);

            debug!(
                "Obfuscation techniques analysis: privacy_interactions={}, factors={}, score={:.2}",
                privacy_interactions,
                obfuscation_factors.len(),
                obfuscation_score
            );
        }
    }
}

/// Update the database notes field with the exfiltration tags
/// Updates the database with exfiltration tags identified for a wallet address
///
/// This function handles the following:
/// 1. Gets the current address record from the database
/// 2. Formats the tags with timestamp into a standardized format
/// 3. Updates the notes field, either by:
///    - Appending the tags if this is the first exfiltration analysis
///    - Replacing existing exfiltration tags if they already exist
/// 4. Updates the database record
///
/// # Arguments
///
/// * `repo` - Database repository instance
/// * `address` - The wallet address to update
/// * `tags` - Vector of exfiltration tags identified for this wallet
///
/// # Returns
///
/// * `Result<bool, HackerdexError>` - Ok(true) if tags were added, Ok(false) if no update was needed
async fn update_db_with_tags(
    repo: &Repository,
    address: &str,
    tags: &[String],
) -> Result<bool, HackerdexError> {
    // Skip updates if no tags were generated for this address
    if tags.is_empty() {
        debug!("No exfiltration tags to update for {}", address);
        return Ok(false);
    }

    // Get the current address record
    match repo.get_address_details(address).await {
        Ok(record) => {
            // Build the tag string with timestamp
            let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
            let tags_str = tags.join(", ");
            let tag_entry = format!("; ExfilTags [{}]: [{}]", timestamp, tags_str);

            // Check for duplicate tags
            let mut duplicate_tags = false;
            if let Some(ref notes) = record.notes {
                // Extract existing tags if present
                if notes.contains("; ExfilTags") {
                    let re = Regex::new(r"; ExfilTags \[[^]]+\]: \[([^]]+)\]").unwrap();
                    if let Some(captures) = re.captures(notes) {
                        if let Some(existing_tags_match) = captures.get(1) {
                            let existing_tags = existing_tags_match.as_str();

                            // Check if all new tags are already contained in existing tags
                            let mut all_tags_exist = true;
                            for tag in tags {
                                if !existing_tags.contains(tag) {
                                    all_tags_exist = false;
                                    break;
                                }
                            }

                            if all_tags_exist {
                                info!("All tags already exist for {}, skipping update", address);
                                duplicate_tags = true;
                            }
                        }
                    }
                }
            }

            // Only proceed if we don't have duplicates
            if !duplicate_tags {
                // Update the notes field
                let notes = match record.notes {
                    Some(ref notes) if !notes.contains("; ExfilTags") => {
                        // First time adding exfil tags - append to existing notes
                        format!("{}{}", notes, tag_entry)
                    }
                    Some(ref notes) => {
                        // Replace existing exfil tags section with updated tags
                        let re = Regex::new(r"; ExfilTags \[[^]]+\]: \[[^]]+\]").unwrap();
                        re.replace(notes, tag_entry.as_str()).to_string()
                    }
                    None => {
                        // No existing notes - just use the tag entry
                        tag_entry
                    }
                };

                // Create updated record
                let mut address_data: AddressData = record.clone().into();
                address_data.notes = Some(notes);

                // Update risk level if certain high-risk patterns are detected
                let high_risk_tags = [
                    "Mixer Interaction",
                    "Bridge Hopping",
                    "Drainer Consolidation",
                ];
                let risk_upgrade = tags
                    .iter()
                    .any(|tag| high_risk_tags.contains(&tag.as_str()));

                if risk_upgrade
                    && address_data.risk_level != "Critical"
                    && address_data.risk_level != "High"
                {
                    info!(
                        "Upgrading risk level for {} due to high-risk exfiltration patterns",
                        address
                    );
                    address_data.risk_level = "High".to_string();
                }

                // Update the database
                match repo.update_address_details(&address_data).await {
                    Ok(_) => {
                        info!(
                            "Updated database notes for {} with tags: {}",
                            address, tags_str
                        );
                        Ok(true)
                    }
                    Err(e) => {
                        error!("Failed to update database for {}: {}", address, e);
                        Err(e)
                    }
                }
            } else {
                Ok(false) // No update needed - duplicate tags
            }
        }
        Err(HackerdexError::NotFound(_)) => {
            warn!(
                "Address {} not found in database, cannot update with exfiltration tags",
                address
            );
            Ok(false)
        }
        Err(e) => {
            error!("Database error when fetching address {}: {}", address, e);
            Err(e)
        }
    }
}

/// Analyze a single wallet and update the database if needed
async fn analyze_wallet(
    client: &RateLimitedClient,
    repo: &Repository,
    address: &str,
    config: &AnalyzeConfig,
) -> Result<Vec<String>> {
    // Start time for this wallet analysis
    let start_time = std::time::Instant::now();
    info!("Starting analysis of wallet: {}", address);

    // Step 1: Fetch historical transactions
    info!(
        "Fetching transaction history for {} (max txs: {:?}, max days: {:?})",
        address, config.max_history_transactions, config.max_history_days
    );
    let signatures = fetch_signatures_for_wallet(
        client,
        address,
        config.max_history_transactions,
        config.max_history_days,
    )
    .await?;

    if signatures.is_empty() {
        warn!("No signatures found for wallet: {}", address);
        return Ok(Vec::new());
    }
    info!(
        "Retrieved {} historical transactions for {}",
        signatures.len(),
        address
    );

    // Step 2: Build historical context
    info!("Building historical context for {}", address);
    let mut context = historical_context_aggregation(client, repo, address, &signatures).await?;
    info!(
        "Historical context built with {} transactions, {} funding sources, {} spending destinations",
        context.transactions.len(),
        context.funding_counterparties.len(),
        context.spending_counterparties.len()
    );

    // Step 3: Apply advanced historical heuristics to detect exfiltration patterns
    info!("Applying heuristics to {} for pattern detection", address);
    apply_historical_heuristics(&mut context);

    // Log the heuristic flags after advanced analysis
    info!(
        "Key heuristic indicators for {}: structuring={:.2}, risky_spending={:.2}, pass_through={}",
        address,
        context.heuristic_flags.structuring_score,
        context.heuristic_flags.risky_spending_destination_ratio,
        context.heuristic_flags.is_pass_through
    );
    debug!(
        "Full heuristic flags after historical analysis for {}: {:?}",
        address, context.heuristic_flags
    );

    // Step 4: Apply exfiltration rule patterns to the context
    info!(
        "Applying exfiltration rules to identify patterns for {}",
        address
    );
    let tags = apply_exfiltration_rules(&context, &config.exfiltration_rules);

    if tags.is_empty() {
        info!("No exfiltration patterns detected for {}", address);
    } else {
        info!(
            "Detected {} exfiltration patterns for {}: {:?}",
            tags.len(),
            address,
            tags
        );
    }

    // Step 5: Update database with tags if configured
    if config.update_db_notes && !tags.is_empty() {
        info!("Updating database records for {}", address);
        match update_db_with_tags(repo, address, &tags).await {
            Ok(true) => {
                info!(
                    "Successfully updated database for {} with {} tags",
                    address,
                    tags.len()
                );
            }
            Ok(false) => {
                debug!(
                    "No database update needed for {} (tags may already exist)",
                    address
                );
            }
            Err(e) => {
                error!("Failed to update database for {}: {}", address, e);
            }
        }
    } else if !tags.is_empty() {
        info!(
            "Tags identified but database updates disabled for {}: {:?}",
            address, tags
        );
    }

    // Log analysis completion with time taken
    let duration = start_time.elapsed();
    info!(
        "Analysis of {} completed in {:.2}s",
        address,
        duration.as_secs_f64()
    );

    Ok(tags)
}

/// Returns true if the provided address is a known Solana program
fn is_known_program_id(pubkey: &str) -> bool {
    // List of known program IDs that should be filtered out from counterparty detection
    const KNOWN_PROGRAM_IDS: [&str; 9] = [
        "11111111111111111111111111111111",             // System Program
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",  // SPL Token Program
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL", // Associated Token Account Program
        "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr",  // Memo Program
        "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s",  // Metaplex Token Metadata
        "Stake11111111111111111111111111111111111111",  // Stake Program
        "Vote111111111111111111111111111111111111111",  // Vote Program
        "BPFLoaderUpgradeab1e11111111111111111111111",  // BPF Loader Upgradeable
        "namesLPneVptA9Z5rqUDD9tMTWEJwofgaYwp8cawRkX",  // SPL Name Service
    ];

    KNOWN_PROGRAM_IDS.contains(&pubkey)
}

/// Helper function to check if an address should be excluded as a counterparty
/// Returns true if the address should be excluded
fn should_exclude_counterparty(
    address: &str,
    self_owned_token_accounts: &HashSet<String>,
    token_mint_accounts: &HashSet<String>,
    program_addresses: &HashSet<String>,
    category_filtered_counterparties: &HashSet<String>,
) -> bool {
    // Check if the address is in any of our filtered sets and log the reason
    if self_owned_token_accounts.contains(address) {
        debug!(
            "Excluding counterparty {}: Self-owned token account",
            address
        );
        return true;
    }

    if token_mint_accounts.contains(address) {
        debug!("Excluding counterparty {}: Token mint account", address);
        return true;
    }

    if program_addresses.contains(address) {
        debug!("Excluding counterparty {}: Program address", address);
        return true;
    }

    if category_filtered_counterparties.contains(address) {
        debug!("Excluding counterparty {}: Excluded by category", address);
        return true;
    }

    false // Not excluded
}

/// Aggregates historical transaction data for a wallet to build context for analysis
///
/// This function processes transaction history for a wallet address to build a comprehensive
/// context including:
/// - SOL balance changes (incoming and outgoing)
/// - Token transfers (with mint addresses and amounts)
/// - True counterparties identification (distinguishing actual value senders/receivers from programs)
/// - Relative transaction direction (incoming or outgoing from the perspective of the analyzed wallet)
/// - Transaction amounts (in SOL or token equivalent)
///
/// The function identifies true counterparties using the following approach:
/// 1. For SOL transfers:
///    - Calculates balance changes for all accounts in the transaction
///    - Identifies accounts with opposite balance changes to the analyzed wallet
///    - Filters out program IDs and intermediate accounts
///    - Sorts potential counterparties by balance change magnitude
///    - Selects the most significant counterparty as the true counterparty
///
/// 2. For token transfers:
///    - Calculates token balance changes for the analyzed wallet
///    - Finds accounts with opposite token balance changes for the same mint
///    - Filters out program accounts that can't be true counterparties
///    - Identifies the most significant counterparty based on transfer amount
///
/// This approach significantly reduces false positives in counterparty tracking and
/// provides a much more accurate picture of actual value flows.
///
/// # Arguments
///
/// * `client` - Rate-limited Solana RPC client
/// * `repo` - Database repository for lookups
/// * `address` - Wallet address to analyze
/// * `signatures` - List of transaction signatures to process
///
/// # Returns
///
/// A `Result` containing the aggregated `HistoricalWalletContext` or an error
async fn historical_context_aggregation(
    client: &RateLimitedClient,
    repo: &Repository,
    address: &str,
    signatures: &[RpcConfirmedTransactionStatusWithSignature],
) -> Result<HistoricalWalletContext> {
    info!("Aggregating historical context for wallet: {}", address);

    let mut context = HistoricalWalletContext::new(address.to_string());

    // Track potential counterparty addresses for batch fetching
    let mut potential_counterparties = HashSet::new();
    // Map of account address to transaction details for post-processing after batch fetching
    let mut transaction_account_map: HashMap<String, Vec<(String, i64, bool, f64)>> =
        HashMap::new();

    info!(
        "Processing {} transactions for wallet {}",
        signatures.len(),
        address
    );
    let wallet_pubkey = match Pubkey::try_from(address) {
        Ok(pubkey) => pubkey,
        Err(_) => {
            return Err(anyhow::Error::msg(format!(
                "Invalid address format for main wallet: {}",
                address
            )));
        }
    };

    // First Pass: Collect all potential counterparty addresses across all transactions
    info!(
        "First pass: Collecting potential counterparties for wallet {}",
        address
    );
    for (i, sig_info) in signatures.iter().enumerate() {
        // Add small delay every few transactions to respect rate limits
        if i > 0 && i % 5 == 0 {
            sleep(Duration::from_millis(100)).await;
        }

        // Log progress periodically
        if i % 5 == 0 || i == signatures.len() - 1 {
            info!(
                "Processing transaction {}/{} for wallet {}",
                i + 1,
                signatures.len(),
                address
            );
        }

        // Skip transactions without block time
        let timestamp = match sig_info.block_time {
            Some(time) => time,
            None => {
                debug!(
                    "Skipping transaction {} without block time",
                    sig_info.signature
                );
                continue;
            }
        };

        // Fetch and parse the transaction with timeout
        info!(
            "Fetching transaction data for signature: {}",
            sig_info.signature
        );
        let tx_result = match tokio::time::timeout(
            Duration::from_secs(30), // 30 second timeout (increased from 10s)
            client.get_transaction(&sig_info.signature),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                warn!(
                    "RPC timeout while fetching transaction {}",
                    sig_info.signature
                );
                // Add delay after timeout to respect rate limits and allow system recovery
                sleep(Duration::from_millis(2000)).await; // Increased from 500ms to 2s
                continue;
            }
        };

        match tx_result {
            Ok(Some(tx_with_meta)) => {
                // Parse the transaction
                let parsed_tx =
                    match transaction_parser::parse_transaction(&tx_with_meta, &sig_info.signature)
                    {
                        Ok(parsed) => parsed,
                        Err(err) => {
                            warn!(
                                "Failed to parse transaction {}: {}",
                                sig_info.signature, err
                            );
                            continue;
                        }
                    };

                // Extract the transaction metadata for balance calculation
                let meta = match &tx_with_meta.transaction.meta {
                    Some(meta) => meta,
                    None => {
                        warn!(
                            "No transaction metadata for signature: {}",
                            sig_info.signature
                        );
                        continue;
                    }
                };

                // Extract account keys from the transaction
                let account_keys = if let EncodedTransaction::Json(ui_tx) =
                    &tx_with_meta.transaction.transaction
                {
                    match &ui_tx.message {
                        UiMessage::Parsed(message) => message
                            .account_keys
                            .iter()
                            .map(|key| key.pubkey.clone())
                            .collect::<Vec<String>>(),
                        UiMessage::Raw(message) => message.account_keys.clone(),
                    }
                } else {
                    // If transaction format is not JSON, use parsed_tx's involved accounts
                    parsed_tx.involved_accounts.clone()
                };

                // Find the index of our address in the account keys
                let address_index = match account_keys.iter().position(|pubkey| pubkey == address) {
                    Some(idx) => idx,
                    None => {
                        warn!(
                            "Address {} not found in account keys for transaction: {}",
                            address, sig_info.signature
                        );
                        continue;
                    }
                };

                // Check if the index is valid for pre/post balances
                if address_index >= meta.pre_balances.len()
                    || address_index >= meta.post_balances.len()
                {
                    warn!(
                        "Balance index out of bounds for address {}: pre_len={}, post_len={}, idx={}",
                        address,
                        meta.pre_balances.len(),
                        meta.post_balances.len(),
                        address_index
                    );
                    continue;
                }

                // Get the pre and post balances for the analyzed address
                let pre_balance = meta.pre_balances[address_index];
                let post_balance = meta.post_balances[address_index];

                // Calculate the net balance change (in SOL)
                let balance_change_lamports = post_balance as i64 - pre_balance as i64;
                let balance_change_sol = balance_change_lamports as f64 / 1_000_000_000.0; // Convert lamports to SOL

                // Determine if transaction is incoming (positive change) or outgoing (negative change)
                let is_incoming = balance_change_lamports > 0;
                let net_amount = balance_change_sol.abs(); // Absolute value for the amount

                // Collect program IDs from this transaction to filter out in counterparty detection
                let transaction_programs: HashSet<String> =
                    parsed_tx.program_ids.iter().cloned().collect();

                // Store potential true counterparties and their balance changes
                let mut tx_potential_counterparties: Vec<(String, i64, bool)> = Vec::new();

                // Extract counterparties based on account changes
                for (idx, account_pubkey) in account_keys.iter().enumerate() {
                    let account_address = account_pubkey.clone();

                    // Skip the account we're analyzing
                    if account_address == address {
                        continue;
                    }

                    // Skip accounts with index out of bounds
                    if idx >= meta.pre_balances.len() || idx >= meta.post_balances.len() {
                        continue;
                    }

                    // Calculate balance change for this potential counterparty
                    let account_pre = meta.pre_balances[idx];
                    let account_post = meta.post_balances[idx];
                    let account_change = account_post as i64 - account_pre as i64;

                    // Skip accounts with no balance change
                    if account_change == 0 {
                        continue;
                    }

                    // Record as a potential counterparty (for later lookup in known addresses)
                    potential_counterparties.insert(account_address.clone());

                    // Also store in the potential counterparties collection for this specific transaction

                    // A valid counterparty would have an opposite balance change direction
                    // For incoming txs: counterparty has decreasing balance
                    // For outgoing txs: counterparty has increasing balance
                    let is_opposite_balance_change =
                        (is_incoming && account_change < 0) || (!is_incoming && account_change > 0);

                    if is_opposite_balance_change {
                        // Store as potential counterparty with its balance change and opposite balance flag
                        tx_potential_counterparties.push((
                            account_address,
                            account_change.abs(),
                            is_opposite_balance_change,
                        ));
                    }
                }

                // Filter and prioritize counterparties
                if !tx_potential_counterparties.is_empty() {
                    // Filter out known program IDs that shouldn't be considered as true counterparties
                    let filtered_counterparties: Vec<(String, i64, bool)> =
                        tx_potential_counterparties
                            .iter()
                            .filter(|(pubkey, _, _)| {
                                // Filter out if it's a known program ID
                                !is_known_program_id(pubkey)
                                    && !transaction_programs.contains(pubkey)
                                // Also filter out if it's one of the program IDs used in this transaction
                            })
                            .cloned()
                            .collect();

                    // If we have counterparties after filtering, use them
                    // Otherwise fall back to the unfiltered list (could be a direct program interaction)
                    let counterparties_to_use = if !filtered_counterparties.is_empty() {
                        filtered_counterparties
                    } else {
                        tx_potential_counterparties
                    };

                    // Sort counterparties by balance change amount (descending)
                    // This prioritizes the counterparty with the largest balance change,
                    // which is more likely to be the true counterparty
                    let mut sorted_counterparties = counterparties_to_use.clone();
                    sorted_counterparties.sort_by(|a, b| b.1.cmp(&a.1));

                    // Use the top counterparty (with the largest balance change)
                    // In complex transactions, this is often the true counterparty
                    if let Some((top_counterparty, _, _)) = sorted_counterparties.first() {
                        // Directly add to context
                        context.add_transaction(
                            &sig_info.signature,
                            timestamp,
                            is_incoming,
                            top_counterparty,
                            net_amount, // Use the absolute amount of the wallet's balance change
                        );

                        // Also store in transaction_account_map for later filtering
                        let entry = transaction_account_map
                            .entry(top_counterparty.clone())
                            .or_insert_with(Vec::new);

                        entry.push((
                            sig_info.signature.clone(),
                            timestamp,
                            is_incoming,
                            net_amount,
                        ));

                        debug!(
                            "True counterparty identified: {} for tx: {}",
                            top_counterparty, sig_info.signature
                        );
                    }
                }

                // Process token balances to identify token transfers
                let pre_token_balances = &parsed_tx.pre_token_balances;
                let post_token_balances = &parsed_tx.post_token_balances;

                // Create a map of (account index, mint) -> pre balance for quick lookup
                let mut pre_balance_map = HashMap::new();
                for balance in pre_token_balances {
                    pre_balance_map.insert((balance.account_index, balance.mint.clone()), balance);
                }

                // Process post balances and compare with pre balances
                for post_balance in post_token_balances {
                    // Skip if this isn't for our address
                    // We need to get the actual account by index
                    let account_idx = post_balance.account_index as usize;
                    if account_idx >= account_keys.len() {
                        continue; // Skip if index out of bounds
                    }

                    let account_address = &account_keys[account_idx];
                    if account_address != address {
                        continue; // Skip if not the address we're analyzing
                    }

                    // Find matching pre-balance
                    let key = (post_balance.account_index, post_balance.mint.clone());
                    let pre_balance = pre_balance_map.get(&key);

                    // If we found a pre-balance, calculate the difference
                    if let Some(pre) = pre_balance {
                        let pre_amount = match &pre.ui_token_amount.ui_amount {
                            Some(amount) => *amount,
                            None => continue, // Skip if no pre amount
                        };

                        let post_amount = match &post_balance.ui_token_amount.ui_amount {
                            Some(amount) => *amount,
                            None => continue, // Skip if no post amount
                        };

                        let token_change = post_amount - pre_amount;

                        // Skip if no significant change
                        if token_change.abs() < 0.000001 {
                            continue;
                        }

                        let is_token_incoming = token_change > 0.0;

                        // Record the token change
                        context.add_token_change(
                            &sig_info.signature,
                            &post_balance.mint,
                            token_change.abs(),
                            is_token_incoming,
                            post_balance.ui_token_amount.decimals,
                        );

                        // Identify the token transfer counterparty
                        // For token transfers, we need to find the account with opposite token balance change
                        if is_token_incoming || !is_token_incoming {
                            // Process for both directions
                            // Look for accounts with opposite token balance changes for the same mint
                            let mut token_counterparties: Vec<(String, f64)> = Vec::new();

                            for other_post in post_token_balances {
                                // Skip the account we're analyzing
                                let other_idx = other_post.account_index as usize;
                                if other_idx >= account_keys.len() || other_idx == account_idx {
                                    continue;
                                }

                                // Only process balances for the same mint
                                if other_post.mint != post_balance.mint {
                                    continue;
                                }

                                // Find the matching pre-balance for this account
                                let other_key = (other_post.account_index, other_post.mint.clone());
                                if let Some(other_pre) = pre_balance_map.get(&other_key) {
                                    // Calculate token balance change for this potential counterparty
                                    let other_pre_amount =
                                        match &other_pre.ui_token_amount.ui_amount {
                                            Some(amount) => *amount,
                                            None => continue,
                                        };

                                    let other_post_amount =
                                        match &other_post.ui_token_amount.ui_amount {
                                            Some(amount) => *amount,
                                            None => continue,
                                        };

                                    let other_change = other_post_amount - other_pre_amount;

                                    // We want an opposite change: if we got tokens, someone lost tokens
                                    let is_opposite = (is_token_incoming && other_change < 0.0)
                                        || (!is_token_incoming && other_change > 0.0);

                                    if is_opposite {
                                        let other_address = &account_keys[other_idx];

                                        // Filter out program IDs
                                        if !is_known_program_id(other_address)
                                            && !transaction_programs.contains(other_address)
                                        {
                                            token_counterparties
                                                .push((other_address.clone(), other_change.abs()));
                                        }
                                    }
                                }
                            }

                            // If we found token counterparties, sort by amount and use the largest
                            if !token_counterparties.is_empty() {
                                token_counterparties.sort_by(|a, b| {
                                    b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                                });
                                if let Some((top_token_counterparty, _)) =
                                    token_counterparties.first()
                                {
                                    debug!(
                                        "Token transfer counterparty identified: {} for tx: {}, mint: {}",
                                        top_token_counterparty,
                                        sig_info.signature,
                                        post_balance.mint
                                    );

                                    // We've identified a token transfer true counterparty
                                    // Add this to the transaction counterparties
                                    context.add_transaction(
                                        &sig_info.signature,
                                        timestamp,
                                        is_token_incoming,
                                        top_token_counterparty,
                                        token_change.abs(), // Use the token amount
                                    );

                                    // Also store in transaction_account_map for later filtering
                                    let entry = transaction_account_map
                                        .entry(top_token_counterparty.clone())
                                        .or_insert_with(Vec::new);

                                    entry.push((
                                        sig_info.signature.clone(),
                                        timestamp,
                                        is_token_incoming,
                                        token_change.abs(),
                                    ));
                                }
                            }
                        }

                        debug!(
                            "Token transfer detected: mint={}, amount={}, direction={}",
                            post_balance.mint,
                            token_change,
                            if is_token_incoming {
                                "incoming"
                            } else {
                                "outgoing"
                            }
                        );
                    }
                }
            }
            Ok(None) => {
                warn!(
                    "No transaction data found for signature: {}",
                    sig_info.signature
                );
            }
            Err(err) => {
                warn!(
                    "Failed to fetch transaction {}: {}",
                    sig_info.signature, err
                );
                // Add delay after error to respect rate limits
                sleep(Duration::from_millis(500)).await;
            }
        }
    }

    // Second Pass: Batch fetch account information for all potential counterparties
    info!(
        "Second pass: Batch fetching account information for {} potential counterparties",
        potential_counterparties.len()
    );

    // Convert HashSet to Vec for batching
    let counterparties_vec: Vec<String> = potential_counterparties.into_iter().collect();

    // Process in batches of 100 accounts (as mentioned in the task)
    let batch_size = 100;
    let total_batches = (counterparties_vec.len() as f64 / batch_size as f64).ceil() as usize;

    // Track which accounts are self-owned token accounts and other account types
    let mut self_owned_token_accounts = HashSet::new();
    let mut token_mint_accounts = HashSet::new();
    let mut program_addresses = HashSet::new();
    let mut external_counterparties = HashSet::new();

    // Initialize program_addresses HashSet with the known program addresses
    for program in KNOWN_PROGRAM_ADDRESSES {
        program_addresses.insert(program.to_string());
    }

    for (batch_idx, accounts_batch) in counterparties_vec.chunks(batch_size).enumerate() {
        info!(
            "Processing account info batch {}/{} ({} accounts)",
            batch_idx + 1,
            total_batches,
            accounts_batch.len()
        );

        // Convert slice to Vec for the API call
        let batch_addresses: Vec<String> = accounts_batch.iter().cloned().collect();

        // Fetch account info in batch with timeout
        let accounts_result = match tokio::time::timeout(
            Duration::from_secs(45), // 45 second timeout for batch requests (increased from 15s)
            client.get_multiple_accounts(&batch_addresses),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                warn!(
                    "RPC timeout while fetching batch {} of accounts",
                    batch_idx + 1
                );
                sleep(Duration::from_millis(3000)).await; // Longer delay after batch timeout (increased from 1s to 3s)
                continue;
            }
        };

        match accounts_result {
            Ok(accounts) => {
                // Process each account in the batch
                for (i, account_opt) in accounts.iter().enumerate() {
                    let address = &batch_addresses[i];

                    // First, check if this is a known program address (quick check before RPC calls)
                    if KNOWN_PROGRAM_ADDRESSES.contains(&address.as_str()) {
                        debug!("Filtered out {} - Known program address", address);
                        program_addresses.insert(address.clone());
                        continue;
                    }

                    match account_opt {
                        Some(account_data) => {
                            // Check if this is a token account owned by the wallet we're analyzing
                            let is_owned_token_account = if let Ok(token_account) =
                                TokenAccount::unpack(&account_data.data)
                            {
                                // Check if token account owner matches the main wallet address
                                let owner_matches = token_account.owner == wallet_pubkey;
                                if owner_matches {
                                    debug!(
                                        "Filtered out {} - Self-owned token account with mint {}",
                                        address, token_account.mint
                                    );
                                    self_owned_token_accounts.insert(address.clone());
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            };

                            // If not a self-owned token account, try to identify if it's a token mint
                            if !is_owned_token_account {
                                // Try to parse as an SPL Token Mint
                                if let Ok(mint) = Mint::unpack(&account_data.data) {
                                    debug!(
                                        "Filtered out {} - Token mint account with supply {}",
                                        address, mint.supply
                                    );
                                    token_mint_accounts.insert(address.clone());
                                    continue;
                                }

                                // Check if it's an executable account (likely a program)
                                if account_data.executable {
                                    debug!("Filtered out {} - Executable program account", address);
                                    program_addresses.insert(address.clone());
                                    continue;
                                }

                                // Not any special account type we filtered - consider as potential external counterparty
                                external_counterparties.insert(address.clone());
                            }
                        }
                        None => {
                            debug!("No account data found for address: {}", address);
                            // No account data found, consider as potential external counterparty
                            external_counterparties.insert(address.clone());
                        }
                    }
                }
            }
            Err(err) => {
                warn!(
                    "Error fetching batch {} of accounts: {}",
                    batch_idx + 1,
                    err
                );
                sleep(Duration::from_millis(1000)).await; // Delay after error
                continue;
            }
        }

        // Add delay between batches
        if batch_idx < total_batches - 1 {
            sleep(Duration::from_millis(200)).await;
        }
    }

    // Log statistics about identified account types
    info!(
        "Account type identification for wallet {}: {} self-owned token accounts, {} token mint accounts, {} program addresses, {} potential external counterparties",
        address,
        self_owned_token_accounts.len(),
        token_mint_accounts.len(),
        program_addresses.len(),
        external_counterparties.len()
    );

    // Third Pass: Process transaction details with knowledge of account types
    info!("Third pass: Processing transactions with account ownership information");

    // Convert external counterparties to Vec for database lookups (clone to avoid borrowing issues)
    let external_counterparties_vec: Vec<String> =
        external_counterparties.iter().cloned().collect();
    let db_batch_size = 50;
    let total_db_batches =
        (external_counterparties_vec.len() as f64 / db_batch_size as f64).ceil() as usize;

    // Track counterparties filtered by category
    let mut category_filtered_counterparties = HashSet::new();

    // Look up all external counterparties in the database to identify known addresses
    info!(
        "Looking up {} external counterparties in the database for wallet {}",
        external_counterparties.len(),
        address
    );

    for (batch_idx, batch) in external_counterparties_vec
        .chunks(db_batch_size)
        .enumerate()
    {
        info!(
            "Processing database lookup batch {}/{} for wallet {}",
            batch_idx + 1,
            total_db_batches,
            address
        );

        // Fetch address details in batches to avoid overloading the database
        let mut successful_lookups = 0;
        for counterparty in batch {
            // Skip if already identified as program, mint, or self-owned token account
            if self_owned_token_accounts.contains(counterparty)
                || token_mint_accounts.contains(counterparty)
                || program_addresses.contains(counterparty)
            {
                continue;
            }

            match tokio::time::timeout(
                Duration::from_secs(15), // 15 second timeout for DB lookups (increased from 5s)
                repo.get_address_details(counterparty),
            )
            .await
            {
                Ok(Ok(record)) => {
                    // Try to update counterparty with the record
                    // If it returns true, it means the record was for an excluded category
                    if context.update_counterparty(counterparty, Some(record)) {
                        // Was excluded by category, add to filtered lists
                        debug!(
                            "Filtered out {} - Excluded by category via update_counterparty",
                            counterparty
                        );
                        category_filtered_counterparties.insert(counterparty.clone());
                        // Add to programs for tracking purposes since we're treating it like a non-counterparty
                        program_addresses.insert(counterparty.clone());
                    } else {
                        // Successfully updated
                        successful_lookups += 1;
                    }
                }
                Ok(Err(err)) => {
                    debug!(
                        "Failed to get database record for {}: {}",
                        counterparty, err
                    );
                }
                Err(_) => {
                    warn!("Database lookup timeout for counterparty: {}", counterparty);
                }
            }
        }

        info!(
            "Completed batch {}/{}: {} of {} lookups successful",
            batch_idx + 1,
            total_db_batches,
            successful_lookups,
            batch.len()
        );

        // Add small delay between batches to avoid database pressure
        if batch_idx < total_db_batches - 1 {
            sleep(Duration::from_millis(100)).await;
        }
    }

    // Process only true external counterparties that passed all filtering
    info!("Final processing: Adding true external counterparties to transaction history");

    // Create a HashSet of truly filtered external counterparties
    let mut true_external_counterparties: HashSet<String> = HashSet::new();

    // Add only addresses that pass all the filtering criteria using our helper function
    for addr in &external_counterparties {
        if !should_exclude_counterparty(
            addr,
            &self_owned_token_accounts,
            &token_mint_accounts,
            &program_addresses,
            &category_filtered_counterparties,
        ) {
            true_external_counterparties.insert(addr.clone());
            debug!("Confirmed true external counterparty: {}", addr);
        } else {
            debug!("Filtered out non-counterparty address: {}", addr);
        }
    }

    // Clear the context first to remove any counterparties that were added during the First Pass
    context = HistoricalWalletContext::new(address.to_string());

    // Now add transactions only for true external counterparties with strict filtering
    let mut counterparties_added = 0;

    for counterparty in &true_external_counterparties {
        // Double-check the address should not be excluded (in case filter lists changed)
        if !should_exclude_counterparty(
            counterparty,
            &self_owned_token_accounts,
            &token_mint_accounts,
            &program_addresses,
            &category_filtered_counterparties,
        ) {
            // Address is confirmed as a legitimate external counterparty
            if let Some(tx_details) = transaction_account_map.get(counterparty) {
                info!(
                    "Adding FINAL filtered counterparty: {} after passing all filters (self-owned check, program check, mint check, DB category check)",
                    counterparty
                );
                for (signature, timestamp, is_incoming, amount) in tx_details {
                    info!(
                        "Adding FINAL transaction for counterparty {}: signature={}, is_incoming={}, amount={}",
                        counterparty, signature, is_incoming, amount
                    );
                    context.add_transaction(
                        signature,
                        *timestamp,
                        *is_incoming,
                        counterparty,
                        *amount,
                    );
                }
                counterparties_added += 1;
            }
        }
    }

    info!(
        "Added {} true external counterparties to final context for wallet {}",
        counterparties_added, address
    );

    // Log detailed statistics about filtered accounts
    info!("Filtering summary for wallet {}:", address);
    info!(
        "  - Filtered out {} self-owned token accounts",
        self_owned_token_accounts.len()
    );
    info!(
        "  - Filtered out {} token mint accounts",
        token_mint_accounts.len()
    );
    info!(
        "  - Filtered out {} program addresses (incl. known and executable)",
        program_addresses.len()
    );
    info!(
        "  - Filtered out {} addresses with excluded categories",
        category_filtered_counterparties.len()
    );
    info!(
        "  - Added {} true external counterparties to analysis",
        true_external_counterparties.len()
    );

    info!("Beginning statistical analysis for wallet {}", address);

    // Calculate derived statistics
    let mut mixer_interaction_count = 0;
    let mut bridge_interaction_count = 0;
    let mut cex_interaction_count = 0;

    // Analyze spending destinations
    let mut high_risk_spending = 0;
    let mut total_spending_interactions = context.spending_counterparties.len();

    info!(
        "Analyzing {} spending destinations for wallet {}",
        total_spending_interactions, address
    );

    for (counterparty_addr, details) in &context.spending_counterparties {
        if let Some(record) = &details.known_record {
            // Count interactions with specific categories
            match record.category.to_lowercase().as_str() {
                cat if cat.contains("mixer") || cat.contains("anonym") => {
                    mixer_interaction_count += 1;
                    debug!(
                        "Detected mixer interaction with counterparty: {}",
                        counterparty_addr
                    );
                }
                cat if cat.contains("bridge") || cat.contains("cross-chain") => {
                    bridge_interaction_count += 1;
                    debug!(
                        "Detected bridge interaction with counterparty: {}",
                        counterparty_addr
                    );
                }
                cat if cat.contains("exchange") || cat.contains("cex") => {
                    cex_interaction_count += 1;
                    debug!(
                        "Detected exchange interaction with counterparty: {}",
                        counterparty_addr
                    );
                }
                _ => {}
            }

            // Count high risk interactions
            if record.risk_level == "High" || record.risk_level == "Critical" {
                high_risk_spending += 1;
                debug!(
                    "Detected high risk spending to counterparty: {} (risk level: {})",
                    counterparty_addr, record.risk_level
                );
            }
        }
    }

    info!(
        "Interaction statistics for {}: mixers={}, bridges={}, exchanges={}, high_risk={}",
        address,
        mixer_interaction_count,
        bridge_interaction_count,
        cex_interaction_count,
        high_risk_spending
    );

    // Prevent division by zero
    if total_spending_interactions == 0 {
        total_spending_interactions = 1;
        debug!("No spending interactions found, setting to 1 to prevent division by zero");
    }

    // Update heuristic flags based on aggregated data
    info!("Calculating heuristic flags for wallet {}", address);

    // 1. Risky spending destination ratio
    let risky_spending_ratio = high_risk_spending as f32 / total_spending_interactions as f32;
    context.heuristic_flags.risky_spending_destination_ratio = risky_spending_ratio;
    info!(
        "Risky spending ratio for {}: {:.4} ({} high risk out of {} total)",
        address, risky_spending_ratio, high_risk_spending, total_spending_interactions
    );

    // 2. Check for high frequency patterns
    let tx_count = context.transaction_timestamps.len();
    if tx_count > 50 {
        // More than 50 transactions in the analyzed period could indicate high frequency
        context.heuristic_flags.is_high_frequency = true;
        info!(
            "High frequency trading pattern detected for {} with {} transactions",
            address, tx_count
        );
    }

    // 3. Check for structuring patterns (many small transactions)
    // Simplified check: If many transactions with small amounts
    if context.transactions.len() > 20 && context.total_sol_volume_out > 0.0 {
        let avg_tx_size = context.total_sol_volume_out / context.transactions.len() as f64;
        if avg_tx_size < 0.1 {
            // Less than 0.1 SOL average
            let structuring_score = (0.1 / avg_tx_size).min(1.0) as f32;
            context.heuristic_flags.structuring_score = structuring_score;
            info!(
                "Potential structuring pattern detected for {} with score {:.4} (avg tx size: {:.4} SOL)",
                address, structuring_score, avg_tx_size
            );
        }
    }

    // 4. Check for pass-through patterns
    let funds_ratio = if context.total_sol_volume_in > 0.0 {
        (context.total_sol_volume_in - context.total_sol_volume_out).abs()
            / context.total_sol_volume_in
    } else {
        1.0 // If no incoming volume, ratio can't be calculated
    };

    if funds_ratio < 0.1
        && context.funding_counterparties.len() > 0
        && context.spending_counterparties.len() > 0
    {
        context.heuristic_flags.is_pass_through = true;
        info!(
            "Pass-through pattern detected for {} (in/out ratio: {:.4}, in: {:.4}, out: {:.4})",
            address, funds_ratio, context.total_sol_volume_in, context.total_sol_volume_out
        );
    }

    // Add custom flags for specific interaction types
    info!("Setting custom interaction flags for wallet {}", address);

    if mixer_interaction_count > 0 {
        let score = (mixer_interaction_count as f32 / total_spending_interactions as f32).min(1.0);
        context
            .heuristic_flags
            .custom_flags
            .insert("mixer_interaction".to_string(), score);
        info!(
            "Mixer interaction flag set for {} with score {:.4}",
            address, score
        );
    }

    if bridge_interaction_count > 0 {
        let score = (bridge_interaction_count as f32 / total_spending_interactions as f32).min(1.0);
        context
            .heuristic_flags
            .custom_flags
            .insert("bridge_interaction".to_string(), score);
        info!(
            "Bridge interaction flag set for {} with score {:.4}",
            address, score
        );
    }

    if cex_interaction_count > 0 {
        let score = (cex_interaction_count as f32 / total_spending_interactions as f32).min(1.0);
        context
            .heuristic_flags
            .custom_flags
            .insert("cex_interaction".to_string(), score);
        info!(
            "CEX interaction flag set for {} with score {:.4}",
            address, score
        );
    }

    info!("Historical context aggregation complete for {}", address);

    // Calculate filtering efficiency metrics
    let total_potential_counterparties = external_counterparties.len();
    let filtered_counterparties =
        total_potential_counterparties - true_external_counterparties.len();

    let filtering_percentage = if total_potential_counterparties > 0 {
        filtered_counterparties as f64 * 100.0 / total_potential_counterparties as f64
    } else {
        0.0
    };

    info!(
        "Final context stats for {}: {} transactions, {} funding sources, {} spending destinations, {:.4} SOL in, {:.4} SOL out",
        address,
        context.transactions.len(),
        context.funding_counterparties.len(),
        context.spending_counterparties.len(),
        context.total_sol_volume_in,
        context.total_sol_volume_out
    );

    info!(
        "Counterparty filtering efficiency: {:.1}% ({} filtered out of {} potential counterparties)",
        filtering_percentage, filtered_counterparties, total_potential_counterparties
    );

    Ok(context)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with more detailed format
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_thread_names(true)
        .with_target(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting wallet exfiltration pattern analysis");
    let start_time = std::time::Instant::now();

    // Load app configuration
    let app_config = Config::from_env();
    info!("Loaded application configuration from environment");

    // Load analysis configuration
    let config_path = "config/analyze_config.toml";
    let analyze_config = load_or_create_config(config_path).await?;
    info!("Loaded analysis configuration from {}", config_path);

    // Load feature flags
    let feature_flags = app_config.load_feature_flags()?;
    info!("Loaded feature flags configuration");

    // Log feature flag status
    info!(
        "Feature flags status - Peel Chain: {}, Fund Churning: {}",
        if feature_flags
            .analysis_features
            .peel_chain_exfiltration_enabled
        {
            "ENABLED"
        } else {
            "DISABLED"
        },
        if feature_flags.analysis_features.fund_churning_enabled {
            "ENABLED"
        } else {
            "DISABLED"
        }
    );

    // Log configuration details for reference
    debug!(
        "Analysis configuration: max_transactions={:?}, max_days={:?}, concurrent_tasks={}",
        analyze_config.max_history_transactions,
        analyze_config.max_history_days,
        analyze_config.max_concurrent_tasks
    );

    // Connect to the database
    info!("Connecting to database: {}", app_config.database_url);
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&app_config.database_url)
        .await
        .with_context(|| "Failed to connect to database")?;

    info!("Database connection established successfully");
    let repo = Repository::new(pool);

    // Initialize RPC client
    info!("Initializing RPC client: {}", app_config.rpc_url);
    let rpc_client = RateLimitedClient::new(Some(app_config.rpc_url.clone()));

    // Get the list of target addresses
    info!("Determining target addresses based on configuration");
    let target_addresses = if analyze_config.analyze_all {
        // In a real implementation, we'd fetch all addresses or paginate
        info!(
            "Fetching all addresses from database (limited to Exchange category for demonstration)"
        );
        repo.get_all_addresses_by_category("Exchange")
            .await
            .with_context(|| "Failed to fetch addresses")?
            .into_iter()
            .map(|record| record.address)
            .collect::<Vec<_>>()
    } else if let Some(categories) = &analyze_config.analyze_categories {
        info!(
            "Fetching addresses for {} specified categories",
            categories.len()
        );
        let mut addresses = Vec::new();
        for category in categories {
            info!("Fetching addresses for category: {}", category);
            let category_addresses = repo
                .get_all_addresses_by_category(category)
                .await
                .with_context(|| format!("Failed to fetch addresses for category: {}", category))?;

            info!(
                "Found {} addresses in category '{}'",
                category_addresses.len(),
                category
            );
            addresses.extend(category_addresses.into_iter().map(|record| record.address));
        }
        addresses
    } else if let Some(specific_addresses) = &analyze_config.analyze_addresses {
        info!(
            "Using {} explicitly specified addresses",
            specific_addresses.len()
        );
        specific_addresses.clone()
    } else {
        error!("No addresses specified for analysis. Check configuration file.");
        return Ok(());
    };

    info!(
        "Found {} total target addresses for analysis",
        target_addresses.len()
    );
    if target_addresses.is_empty() {
        warn!("No addresses to analyze. Check your configuration or database content.");
        return Ok(());
    }

    // Process wallets with limited concurrency
    info!(
        "Starting analysis with {} concurrent tasks",
        analyze_config.max_concurrent_tasks
    );
    let semaphore = Arc::new(Semaphore::new(analyze_config.max_concurrent_tasks));
    let mut tasks = Vec::new();

    // Add a shared flag for graceful shutdown
    let running = Arc::new(AtomicBool::new(true));

    // Feature flags are already loaded above

    for address in target_addresses {
        let sem_permit = semaphore.clone().acquire_owned().await?;
        let client = rpc_client.clone();
        let repo_clone = Repository::new(repo.pool.clone());
        let config = analyze_config.clone();

        let handle = tokio::spawn(async move {
            let result = analyze_wallet(&client, &repo_clone, &address, &config).await;
            drop(sem_permit); // Release the permit
            (address, result)
        });

        tasks.push(handle);
    }

    // Progress tracking
    let total_tasks = tasks.len();
    info!(
        "Spawned {} analysis tasks, waiting for completion",
        total_tasks
    );

    // Wait for all tasks and collect results
    let mut wallets_analyzed = 0;
    let mut wallets_tagged = 0;
    let mut wallets_failed = 0;
    let mut exfiltration_patterns = HashMap::new();
    let mut tagged_wallet_details: HashMap<String, Vec<String>> = HashMap::new();
    let mut completed_tasks = Vec::new();

    // Use tokio::select! to handle tasks and Ctrl+C
    let mut tasks_futures = tokio::task::JoinSet::new();

    // Add all tasks to the JoinSet
    for task in tasks {
        tasks_futures.spawn(async move { task.await });
    }

    // Use a separate flag to track if we should exit due to Ctrl+C
    let mut shutdown_requested = false;
    let mut progress_timer = tokio::time::interval(Duration::from_secs(5));

    // Process tasks and handle Ctrl+C
    loop {
        tokio::select! {
            // Check for Ctrl+C
            _ = signal::ctrl_c(), if !shutdown_requested => {
                info!("Shutdown signal received. Gracefully stopping and collecting results...");
                running.store(false, Ordering::SeqCst);
                shutdown_requested = true;
            }

            // Process next completed task
            result = tasks_futures.join_next(), if !tasks_futures.is_empty() => {
                match result {
                    Some(Ok(task_result)) => {
                        completed_tasks.push(task_result);

                        // Show progress periodically
                        if completed_tasks.len() % 10 == 0 {
                            info!(
                                "Progress: {}/{} tasks completed ({:.1}%)",
                                completed_tasks.len(),
                                total_tasks,
                                (completed_tasks.len() as f64 / total_tasks as f64) * 100.0
                            );
                        }
                    },
                    Some(Err(e)) => {
                        error!("Task panicked: {}", e);
                        wallets_failed += 1;
                    },
                    None => {
                        // All tasks done
                        if !shutdown_requested {
                            info!("All tasks completed successfully.");
                        }
                        break;
                    }
                }
            }

            // Periodic progress updates
            _ = progress_timer.tick() => {
                info!(
                    "Progress update: {}/{} tasks completed ({:.1}%)",
                    completed_tasks.len(),
                    total_tasks,
                    (completed_tasks.len() as f64 / total_tasks as f64) * 100.0
                );

                // Check if we should terminate due to Ctrl+C after the progress update
                if shutdown_requested {
                    info!("Graceful shutdown requested. Processing results from completed tasks...");
                    break;
                }
            }
        }

        // If shutdown requested and all in-flight tasks completed, we can break
        if shutdown_requested && tasks_futures.is_empty() {
            break;
        }
    }

    // Graceful shutdown of remaining tasks
    let remaining_count = tasks_futures.len();
    if remaining_count > 0 {
        info!(
            "Gracefully shutting down {} remaining tasks...",
            remaining_count
        );
        tasks_futures.shutdown().await;
        info!("Remaining tasks have been shut down.");
    }

    // Process all completed tasks
    for task_result in completed_tasks {
        match task_result {
            Ok(result) => {
                match result {
                    (address, Ok(tags)) => {
                        wallets_analyzed += 1;
                        if !tags.is_empty() {
                            info!("Tags for {}: {:?}", address, tags.clone());
                            wallets_tagged += 1;

                            // Store wallet address and its tags for the final report
                            tagged_wallet_details.insert(address.clone(), tags.clone());

                            // Count exfiltration patterns for reporting
                            for tag in tags {
                                *exfiltration_patterns.entry(tag).or_insert(0) += 1;
                            }
                        } else {
                            debug!("No exfiltration patterns detected for {}", address);
                        }

                        // Log progress periodically
                        if wallets_analyzed % 10 == 0 || wallets_analyzed == total_tasks {
                            info!(
                                "Progress: {}/{} wallets analyzed ({:.1}%), {} tagged, {} failed",
                                wallets_analyzed,
                                total_tasks,
                                (wallets_analyzed as f64 / total_tasks as f64) * 100.0,
                                wallets_tagged,
                                wallets_failed
                            );
                        }
                    }
                    (address, Err(e)) => {
                        error!("Error analyzing wallet {}: {}", address, e);
                        wallets_analyzed += 1;
                        wallets_failed += 1;
                    }
                }
            }
            Err(e) => {
                error!("Task panicked: {}", e);
                wallets_failed += 1;
            }
        }
    }

    // Calculate elapsed time
    let duration = start_time.elapsed();
    let minutes = duration.as_secs() / 60;
    let seconds = duration.as_secs() % 60;

    // Generate detailed final summary
    info!("====== EXFILTRATION ANALYSIS SUMMARY ======");
    info!("Run completed in {}m {}s", minutes, seconds);
    info!("Total wallets analyzed: {}", wallets_analyzed);
    info!(
        "Wallets tagged with exfiltration patterns: {}",
        wallets_tagged
    );
    info!("Wallets with analysis errors: {}", wallets_failed);

    if wallets_tagged > 0 {
        info!(
            "Detection rate: {:.1}%",
            (wallets_tagged as f64 / wallets_analyzed as f64) * 100.0
        );

        // Sort patterns by frequency
        let mut patterns: Vec<(String, i32)> = exfiltration_patterns.into_iter().collect();
        patterns.sort_by(|a, b| b.1.cmp(&a.1));

        info!("Most common exfiltration patterns:");
        for (pattern, count) in patterns {
            let percentage = (count as f64 / wallets_tagged as f64) * 100.0;
            info!(
                "  - {} ({} wallets, {:.1}% of tagged)",
                pattern, count, percentage
            );
        }
    }

    info!("==========================================");

    // Print wallet identification details after the summary
    if wallets_tagged > 0 {
        info!("====== TAGGED WALLET IDENTIFICATION ======");
        info!("Wallets with detected exfiltration patterns:");

        // Convert to vector and sort for consistent output
        let mut sorted_wallets: Vec<(String, Vec<String>)> =
            tagged_wallet_details.into_iter().collect();
        sorted_wallets.sort_by(|a, b| a.0.cmp(&b.0));

        for (wallet_address, tags) in sorted_wallets {
            info!("Wallet: {}", wallet_address);
            info!("  Tags: {:?}", tags);
        }

        info!("==========================================");
    }

    // Log configuration recommendation if success rate is low
    if wallets_tagged == 0 && wallets_analyzed > 0 {
        warn!(
            "No exfiltration patterns detected. Consider adjusting the rules in the configuration file."
        );
    }

    // If we had a graceful shutdown, show info about remaining tasks
    if !running.load(Ordering::SeqCst) && remaining_count > 0 {
        info!(
            "Note: {} tasks were cancelled during graceful shutdown",
            remaining_count
        );
        info!(
            "Analysis results above are based on the {} completed tasks",
            wallets_analyzed
        );
    }

    Ok(())
}
