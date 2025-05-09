use anyhow::{anyhow, Result};
use solana_client::rpc_response::RpcConfirmedTransactionStatusWithSignature;
use solana_sdk::{
    pubkey::Pubkey,
    // signature::Signature, // For Signature itself (now unused)
};
use solana_transaction_status::{EncodedTransaction, UiMessage}; // UiTransactionTokenBalance removed as it's re-evaluated if needed
use spl_token::state::{Account as TokenAccount, Mint};
use solana_sdk::program_pack::Pack; // For unpack method
use std::{
    collections::{HashMap, HashSet},
    // str::FromStr, // For Signature::from_str (Pubkey::from_str imports it implicitly)
    time::Duration,
};
use tokio::time::sleep;
use tracing::{debug, info, warn};

// Crate-specific imports
use crate::db::models::AddressRecord; // Actual record from DB
use crate::db::Repository;
use crate::heuristic_engine::types::HeuristicFlags;
use crate::rpc::RateLimitedClient;
use crate::analysis::transaction_parser; // Corrected path

/// Represents a token balance change within a transaction for the analyzed wallet.
#[derive(Debug, Clone)]
pub struct TokenChange {
    pub mint: String, // Changed from mint_address for consistency
    pub amount: f64,
    pub is_incoming: bool,
    pub decimals: u8,
}

/// Details about a single transaction relevant to the analyzed wallet.
#[derive(Debug, Clone)]
pub struct TransactionDetails {
    pub signature: String,
    pub timestamp: i64,
    pub counterparties: Vec<String>, // Addresses involved other than the main wallet, refined later
    pub is_incoming: bool,      // From the perspective of the main wallet for this transaction leg
    pub amount: f64,            // SOL amount, or primary token amount if applicable
    pub token_changes: Vec<TokenChange>, // Specific token movements for this tx
}

/// Details about a counterparty interaction.
#[derive(Debug, Clone)]
pub struct CounterpartyDetails {
    pub address: String,
    pub known_record: Option<AddressRecord>, // Populated from DB
    pub total_amount: f64,                  // Total SOL or equivalent value exchanged
    pub interaction_count: usize,
    pub first_seen_at: i64, // Timestamp of first interaction
    pub last_seen_at: i64,  // Timestamp of last interaction
}

/// Context built from historical wallet data, used for detailed analysis.
#[derive(Debug, Clone)]
pub struct HistoricalWalletContext {
    /// Address being analyzed
    pub address: String,
    /// Total SOL volume transferred in
    pub total_sol_volume_in: f64,
    /// Total SOL volume transferred out
    pub total_sol_volume_out: f64,
    /// Counterparties from which funds were received (address -> details)
    pub funding_counterparties: HashMap<String, CounterpartyDetails>,
    /// Counterparties to which funds were sent (address -> details)
    pub spending_counterparties: HashMap<String, CounterpartyDetails>,
    /// Heuristic flags computed from the aggregated data
    pub heuristic_flags: HeuristicFlags,
    /// Timestamps of all transactions for pattern detection (may contain duplicates if tx has multiple legs)
    pub transaction_timestamps: Vec<i64>,
    /// Detailed transaction data, keyed by signature.
    pub transactions: HashMap<String, TransactionDetails>,
    /// Tracks token changes per transaction signature, then by mint.
    pub token_changes_by_tx: HashMap<String, Vec<TokenChange>>,
}

impl HistoricalWalletContext {
    pub fn new(address: String) -> Self {
        Self {
            address,
            total_sol_volume_in: 0.0,
            total_sol_volume_out: 0.0,
            funding_counterparties: HashMap::new(),
            spending_counterparties: HashMap::new(),
            heuristic_flags: HeuristicFlags::default(), // Assumes HeuristicFlags impls Default
            transaction_timestamps: Vec::new(),
            transactions: HashMap::new(),
            token_changes_by_tx: HashMap::new(),
        }
    }

    /// Adds or updates a transaction's details, focusing on SOL transfers or primary interactions.
    pub fn add_transaction(
        &mut self,
        signature: &str,
        timestamp: i64,
        is_incoming: bool, // For the main wallet
        counterparty: &str,
        amount: f64, // SOL amount or primary token amount
    ) {
        let tx_details = self
            .transactions
            .entry(signature.to_string())
            .or_insert_with(|| TransactionDetails {
                signature: signature.to_string(),
                timestamp,
                counterparties: Vec::new(),
                is_incoming, // This might be too simplistic if a tx has multiple legs.
                amount,      // This amount is specific to this leg with this counterparty.
                token_changes: Vec::new(), // Token changes are added separately.
            });

        if !tx_details.counterparties.contains(&counterparty.to_string()) {
            tx_details.counterparties.push(counterparty.to_string());
        }
        
        // Update overall SOL volume and counterparty aggregates
        // This logic assumes `amount` is SOL. Token amounts are handled by `add_token_change`.
        if is_incoming {
            self.total_sol_volume_in += amount;
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
            entry.last_seen_at = timestamp.max(entry.last_seen_at);
            entry.first_seen_at = timestamp.min(entry.first_seen_at);
        } else {
            self.total_sol_volume_out += amount;
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
            entry.last_seen_at = timestamp.max(entry.last_seen_at);
            entry.first_seen_at = timestamp.min(entry.first_seen_at);
        }
        self.transaction_timestamps.push(timestamp); // Record all timestamps for frequency analysis
    }

    /// Adds a specific token change event associated with a transaction.
    pub fn add_token_change(
        &mut self,
        signature: &str,
        mint: &str,
        amount: f64,
        is_incoming: bool, // For the main wallet's token account
        decimals: u8,
    ) {
        let token_change = TokenChange {
            mint: mint.to_string(),
            amount,
            is_incoming,
            decimals,
        };

        self.token_changes_by_tx
            .entry(signature.to_string())
            .or_default()
            .push(token_change.clone());
        
        // Also update the main transaction entry if it exists
        if let Some(tx_details) = self.transactions.get_mut(signature) {
            tx_details.token_changes.push(token_change);
        }
    }

    /// Updates counterparty details with a known address record from the database.
    /// Returns `true` if the counterparty's category suggests it should be filtered
    /// (e.g., "Irrelevant", "Internal", "Own Account").
    pub fn update_counterparty_record(&mut self, counterparty_address: &str, record: AddressRecord) -> bool {
        let excluded_categories: HashSet<String> = [
            "Irrelevant".to_string(),
            "Internal".to_string(), // Might represent internal ledger accounts of an exchange
            "Own Account".to_string(), // Explicitly self-owned, though should be caught earlier
            "Solana Program".to_string(), // System programs, etc.
            "Token Mint".to_string(),
            // Add more categories that don't represent true external counterparties
        ].iter().cloned().collect();

        if excluded_categories.contains(&record.category) {
            debug!("Counterparty {} excluded by category: {}", counterparty_address, record.category);
            return true; // Indicates exclusion
        }

        let mut updated = false;
        if let Some(details) = self.funding_counterparties.get_mut(counterparty_address) {
            details.known_record = Some(record.clone());
            updated = true;
        }
        if let Some(details) = self.spending_counterparties.get_mut(counterparty_address) {
            details.known_record = Some(record); // No clone needed if it's the last use
            updated = true;
        }
        if updated {
            debug!("Updated known_record for counterparty {}", counterparty_address);
        } else {
            // This case should ideally not happen if add_transaction was called first
            // Or, this counterparty was only seen in token transfers not yet fully integrated
            // into funding/spending_counterparties. For now, we log.
            warn!("Attempted to update known_record for {} but it was not found in funding/spending lists.", counterparty_address);
        }
        false // Not excluded by category
    }
}

// Define known program addresses at the module level
// These are programs that are part of the Solana infrastructure or common SPL programs
// and are often filtered out when identifying true counterparties.
const KNOWN_PROGRAM_ADDRESSES: [&str; 9] = [
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


/// Returns true if the provided address is a known Solana program.
/// This function checks against a predefined list of common Solana program IDs.
pub fn is_known_program_id(pubkey: &str) -> bool {
    KNOWN_PROGRAM_ADDRESSES.contains(&pubkey)
}

/// Helper function to check if an address should be excluded as a counterparty.
/// Returns true if the address is in any of the provided exclusion sets:
/// - Self-owned token accounts
/// - Token mint accounts
/// - Program addresses (generic, including those dynamically identified)
/// - Counterparties filtered out due to their category (e.g., "Irrelevant")
pub fn should_exclude_counterparty(
    address: &str,
    self_owned_token_accounts: &HashSet<String>,
    token_mint_accounts: &HashSet<String>,
    program_addresses: &HashSet<String>, // This set can include KNOWN_PROGRAM_ADDRESSES and dynamically found ones
    category_filtered_counterparties: &HashSet<String>,
) -> bool {
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

    // This checks against both pre-defined known programs and any dynamically identified program addresses
    if program_addresses.contains(address) || is_known_program_id(address) {
        debug!("Excluding counterparty {}: Program address", address);
        return true;
    }

    if category_filtered_counterparties.contains(address) {
        debug!("Excluding counterparty {}: Excluded by category", address);
        return true;
    }

    false // Not excluded
}


/// Aggregates historical transaction data for a wallet to build context for analysis.
///
/// This function processes transaction history for a wallet address to build a comprehensive
/// context including:
/// - SOL balance changes (incoming and outgoing)
/// - Token transfers (with mint addresses and amounts)
/// - True counterparties identification (distinguishing actual value senders/receivers from programs)
/// - Relative transaction direction (incoming or outgoing from the perspective of the analyzed wallet)
/// - Transaction amounts (in SOL or token equivalent)
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
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)] // This function is inherently complex
pub async fn historical_context_aggregation(
    client: &RateLimitedClient,
    repo: &Repository,
    address: &str,
    signatures: &[RpcConfirmedTransactionStatusWithSignature],
) -> Result<HistoricalWalletContext> {
    info!("Aggregating historical context for wallet: {}", address);

    let mut context = HistoricalWalletContext::new(address.to_string());

    let mut potential_counterparties = HashSet::new();
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
            return Err(anyhow!(
                "Invalid address format for main wallet: {}",
                address
            ));
        }
    };

    info!(
        "First pass: Collecting potential counterparties for wallet {}",
        address
    );
    for (i, sig_info) in signatures.iter().enumerate() {
        if i > 0 && i % 5 == 0 {
            sleep(Duration::from_millis(100)).await;
        }

        if i % 5 == 0 || i == signatures.len() - 1 {
            info!(
                "Processing transaction {}/{} for wallet {}",
                i + 1,
                signatures.len(),
                address
            );
        }

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

        info!(
            "Fetching transaction data for signature: {}",
            sig_info.signature
        );
        let tx_result = match tokio::time::timeout(
            Duration::from_secs(30),
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
                sleep(Duration::from_millis(2000)).await;
                continue;
            }
        };

        match tx_result {
            Ok(Some(tx_with_meta)) => {
                let parsed_tx =
                    match transaction_parser::parse_transaction(&tx_with_meta, &sig_info.signature) // signature is already String
                    {
                        Ok(parsed) => parsed,
                        Err(err) => {
                            warn!(
                                "Failed to parse transaction {}: {}\nTransaction meta: {:?}", // Added more context
                                sig_info.signature, err, tx_with_meta.transaction.meta
                            );
                            continue;
                        }
                    };

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
                    parsed_tx.involved_accounts.clone()
                };

                let address_index = match account_keys.iter().position(|pubkey| pubkey == address) {
                    Some(idx) => idx,
                    None => {
                         // It's possible the main address is not directly in account_keys if it's an ATA,
                         // but involved_accounts should catch it. If not, then it's a problem.
                        warn!(
                            "Address {} not found in account keys for transaction: {}. Account keys: {:?}. Involved_accounts: {:?}.",
                            address, sig_info.signature, account_keys, parsed_tx.involved_accounts
                        );
                        continue;
                    }
                };

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

                let pre_balance = meta.pre_balances[address_index];
                let post_balance = meta.post_balances[address_index];
                let balance_change_lamports = post_balance as i64 - pre_balance as i64;
                let balance_change_sol = balance_change_lamports as f64 / 1_000_000_000.0;
                let is_incoming = balance_change_lamports > 0;
                let net_amount = balance_change_sol.abs();

                let transaction_programs: HashSet<String> =
                    parsed_tx.program_ids.iter().cloned().collect();
                let mut tx_potential_counterparties: Vec<(String, i64, bool)> = Vec::new();

                for (idx, account_pubkey) in account_keys.iter().enumerate() {
                    let account_address_str = account_pubkey.clone();
                    if account_address_str == address {
                        continue;
                    }
                    if idx >= meta.pre_balances.len() || idx >= meta.post_balances.len() {
                        continue;
                    }
                    let account_pre = meta.pre_balances[idx];
                    let account_post = meta.post_balances[idx];
                    let account_change = account_post as i64 - account_pre as i64;

                    if account_change == 0 {
                        continue;
                    }
                    potential_counterparties.insert(account_address_str.clone());
                    let is_opposite_balance_change =
                        (is_incoming && account_change < 0) || (!is_incoming && account_change > 0);

                    if is_opposite_balance_change {
                        tx_potential_counterparties.push((
                            account_address_str,
                            account_change.abs(),
                            is_opposite_balance_change,
                        ));
                    }
                }

                if !tx_potential_counterparties.is_empty() {
                    let filtered_counterparties: Vec<(String, i64, bool)> =
                        tx_potential_counterparties
                            .iter()
                            .filter(|(pubkey, _, _)| {
                                !is_known_program_id(pubkey)
                                    && !transaction_programs.contains(pubkey)
                            })
                            .cloned()
                            .collect();

                    let counterparties_to_use = if !filtered_counterparties.is_empty() {
                        filtered_counterparties
                    } else {
                        tx_potential_counterparties // Fallback if filtering removes all
                    };

                    let mut sorted_counterparties = counterparties_to_use; // No clone needed here
                    sorted_counterparties.sort_by(|a, b| b.1.cmp(&a.1));

                    if let Some((top_counterparty, _, _)) = sorted_counterparties.first() {
                        context.add_transaction(
                            &sig_info.signature, // signature is already String
                            timestamp,
                            is_incoming,
                            top_counterparty,
                            net_amount,
                        );
                        transaction_account_map
                            .entry(top_counterparty.clone())
                            .or_insert_with(Vec::new)
                            .push((
                                sig_info.signature.clone(), // clone if String is needed
                                timestamp,
                                is_incoming,
                                net_amount,
                            ));
                        debug!(
                            "True SOL counterparty identified: {} for tx: {}",
                            top_counterparty, sig_info.signature
                        );
                    }
                }

                let pre_token_balances = &parsed_tx.pre_token_balances;
                let post_token_balances = &parsed_tx.post_token_balances;


                let mut pre_balance_map = HashMap::new();
                for balance in pre_token_balances {
                    pre_balance_map.insert((balance.account_index, balance.mint.clone()), balance);
                }

                for post_balance in post_token_balances {
                    let account_idx = post_balance.account_index as usize;
                    if account_idx >= account_keys.len() {
                        continue;
                    }
                    let account_address_str = &account_keys[account_idx];
                    if account_address_str != address {
                        continue;
                    }

                    let key = (post_balance.account_index, post_balance.mint.clone());
                    if let Some(pre) = pre_balance_map.get(&key) {
                        let pre_amount = match pre.ui_token_amount.ui_amount {
                            Some(amount) => amount,
                            None => continue,
                        };
                        let post_amount = match post_balance.ui_token_amount.ui_amount {
                            Some(amount) => amount,
                            None => continue,
                        };
                        let token_change = post_amount - pre_amount;

                        if token_change.abs() < 0.000001 { // Tolerance for float comparison
                            continue;
                        }
                        let is_token_incoming = token_change > 0.0;
                        context.add_token_change(
                            &sig_info.signature, // signature is already String
                            &post_balance.mint,
                            token_change.abs(),
                            is_token_incoming,
                            post_balance.ui_token_amount.decimals,
                        );

                        let mut token_counterparties: Vec<(String, f64)> = Vec::new();
                        for other_post in post_token_balances {
                            let other_idx = other_post.account_index as usize;
                            if other_idx >= account_keys.len() || other_idx == account_idx {
                                continue;
                            }
                            if other_post.mint != post_balance.mint {
                                continue;
                            }
                            let other_key = (other_post.account_index, other_post.mint.clone());
                            if let Some(other_pre) = pre_balance_map.get(&other_key) {
                                let other_pre_amount = match other_pre.ui_token_amount.ui_amount {
                                    Some(amount) => amount,
                                    None => continue,
                                };
                                let other_post_amount = match other_post.ui_token_amount.ui_amount {
                                    Some(amount) => amount,
                                    None => continue,
                                };
                                let other_change = other_post_amount - other_pre_amount;
                                let is_opposite = (is_token_incoming && other_change < 0.0)
                                    || (!is_token_incoming && other_change > 0.0);

                                if is_opposite {
                                    let other_address = &account_keys[other_idx];
                                    if !is_known_program_id(other_address)
                                        && !transaction_programs.contains(other_address)
                                    {
                                        token_counterparties
                                            .push((other_address.clone(), other_change.abs()));
                                    }
                                }
                            }
                        }

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
                                context.add_transaction( // Overwrite/add SOL transaction if this is more specific
                                    &sig_info.signature, // signature is already String
                                    timestamp,
                                    is_token_incoming,
                                    top_token_counterparty,
                                    token_change.abs(),
                                );
                                transaction_account_map
                                    .entry(top_token_counterparty.clone())
                                    .or_insert_with(Vec::new)
                                    .push((
                                        sig_info.signature.clone(), // clone if String is needed
                                        timestamp,
                                        is_token_incoming,
                                        token_change.abs(),
                                    ));
                            }
                        }
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
                sleep(Duration::from_millis(500)).await;
            }
        }
    }

    info!(
        "Second pass: Batch fetching account information for {} potential counterparties",
        potential_counterparties.len()
    );
    let counterparties_vec: Vec<String> = potential_counterparties.into_iter().collect();
    let batch_size = 100; // Solana's getMultipleAccounts has a limit of 100
    let total_batches = (counterparties_vec.len() as f64 / batch_size as f64).ceil() as usize;

    let mut self_owned_token_accounts = HashSet::new();
    let mut token_mint_accounts = HashSet::new();
    let mut program_addresses_identified = HashSet::new(); // Renamed to avoid conflict with module const
    let mut external_counterparties = HashSet::new();

    // Pre-populate with known program addresses
    for prog_addr in KNOWN_PROGRAM_ADDRESSES.iter() {
        program_addresses_identified.insert(prog_addr.to_string());
    }


    for (batch_idx, accounts_batch_str) in counterparties_vec.chunks(batch_size).enumerate() {
        info!(
            "Processing account info batch {}/{} ({} accounts)",
            batch_idx + 1,
            total_batches,
            accounts_batch_str.len()
        );
        
        // accounts_batch_str is already &[String], which get_multiple_accounts expects.
        // No need to convert to Pubkey here, RateLimitedClient handles it.

        let accounts_result = match tokio::time::timeout(
            Duration::from_secs(45),
            client.get_multiple_accounts(accounts_batch_str), // Pass accounts_batch_str directly
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                warn!(
                    "RPC timeout while fetching batch {} of accounts",
                    batch_idx + 1
                );
                sleep(Duration::from_millis(3000)).await;
                continue;
            }
        };

        match accounts_result {
            Ok(accounts_data) => {
                for (i, account_opt) in accounts_data.iter().enumerate() {
                    let current_address_str = &accounts_batch_str[i];

                    if KNOWN_PROGRAM_ADDRESSES.contains(&current_address_str.as_str()) { // Using as_str() for comparison
                        debug!("Filtered out {} - Known program address (initial check)", current_address_str);
                        program_addresses_identified.insert(current_address_str.clone());
                        continue;
                    }

                    match account_opt {
                        Some(account_data) => {
                            if let Ok(token_account) = TokenAccount::unpack(&account_data.data) {
                                if token_account.owner == wallet_pubkey {
                                    debug!(
                                        "Filtered out {} - Self-owned token account with mint {}",
                                        current_address_str, token_account.mint
                                    );
                                    self_owned_token_accounts.insert(current_address_str.clone());
                                    continue; // Skip further checks if self-owned
                                }
                            }
                            
                            // Check if it's a mint after checking if it's a self-owned token account
                            if Mint::unpack(&account_data.data).is_ok() {
                                debug!(
                                    "Filtered out {} - Token mint account",
                                    current_address_str
                                );
                                token_mint_accounts.insert(current_address_str.clone());
                                continue; // Skip further checks if it's a mint
                            }

                            if account_data.executable {
                                debug!("Filtered out {} - Executable program account", current_address_str);
                                program_addresses_identified.insert(current_address_str.clone());
                                continue; // Skip further checks if executable
                            }
                            
                            // If none of the above, consider it an external counterparty for now
                            external_counterparties.insert(current_address_str.clone());

                        }
                        None => {
                            debug!("No account data found for address: {}", current_address_str);
                            external_counterparties.insert(current_address_str.clone());
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
            }
        }
        if batch_idx < total_batches - 1 {
            sleep(Duration::from_millis(200)).await;
        }
    }


    info!(
        "Account type identification for wallet {}: {} self-owned, {} mints, {} programs, {} external candidates",
        address,
        self_owned_token_accounts.len(),
        token_mint_accounts.len(),
        program_addresses_identified.len(),
        external_counterparties.len()
    );

    info!("Third pass: Processing transactions with account ownership information");
    let external_counterparties_vec: Vec<String> =
        external_counterparties.iter().cloned().collect();
    let db_batch_size = 50;
    let total_db_batches =
        (external_counterparties_vec.len() as f64 / db_batch_size as f64).ceil() as usize;
    let mut category_filtered_counterparties = HashSet::new();

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
        for counterparty_str in batch {
            if self_owned_token_accounts.contains(counterparty_str)
                || token_mint_accounts.contains(counterparty_str)
                || program_addresses_identified.contains(counterparty_str) // Check combined program list
                || KNOWN_PROGRAM_ADDRESSES.contains(&counterparty_str.as_str())
            {
                continue;
            }

            match tokio::time::timeout(
                Duration::from_secs(15),
                repo.get_address_details(counterparty_str),
            )
            .await
            {
                Ok(Ok(actual_record)) => { // Assuming get_address_details returns Result<AddressRecord>
                    // If successful, actual_record is AddressRecord.
                    // update_counterparty_record will update the counterparty and return true if excluded.
                    if context.update_counterparty_record(counterparty_str, actual_record) {
                        debug!(
                            "Counterparty {} excluded by DB category via update_counterparty_record",
                            counterparty_str
                        );
                        category_filtered_counterparties.insert(counterparty_str.clone());
                        // Treat as a non-true-counterparty for filtering purposes.
                        program_addresses_identified.insert(counterparty_str.clone());
                    }
                    // If not excluded, its known_record is updated within update_counterparty_record.
                }
                Ok(Err(db_err)) => { // This is a HackerdexError (anyhow::Error) from get_address_details.
                                     // This could be a "not found" error or a genuine database access error.
                    debug!(
                        "DB lookup for counterparty {} resulted in error (could be Not Found or other issue): {}",
                        counterparty_str, db_err
                    );
                    // In this case, the counterparty's known_record in the context will remain None.
                }
                Err(_timeout_err) => { // This is a TimeoutError from tokio::time::timeout
                    warn!("Database lookup timeout for counterparty: {}", counterparty_str);
                    // known_record in the context will remain None.
                }
            }
        }
        if batch_idx < total_db_batches - 1 {
            sleep(Duration::from_millis(100)).await;
        }
    }
    
    // Rebuild context with strictly filtered counterparties
    let mut final_context = HistoricalWalletContext::new(address.to_string());
    // Copy over token changes directly, as they are not counterparty-dependent in the same way
    for (tx_sig, changes) in &context.token_changes_by_tx {
        for change in changes {
            final_context.add_token_change(&tx_sig, &change.mint, change.amount, change.is_incoming, change.decimals);
        }
    }


    let mut true_external_counterparties_added = 0;
    for (counterparty, txs) in &transaction_account_map {
        if !should_exclude_counterparty(
            counterparty,
            &self_owned_token_accounts,
            &token_mint_accounts,
            &program_addresses_identified, // Use the comprehensive set
            &category_filtered_counterparties,
        ) {
            // This counterparty is deemed a true external party
            true_external_counterparties_added +=1;
            for (signature, timestamp, is_incoming, amount) in txs {
                final_context.add_transaction(
                    signature,
                    *timestamp,
                    *is_incoming,
                    counterparty,
                    *amount,
                );
            }
            // Update the final context with any known DB record for this true counterparty
            if let Some(details) = context.funding_counterparties.get(counterparty)
                .or_else(|| context.spending_counterparties.get(counterparty)) {
                if let Some(record) = &details.known_record {
                     // Ensure the record is properly propagated to the final_context's version of counterparty details
                     // This might involve re-adding to funding/spending_counterparties in final_context
                     // or having a specific method on final_context to set this.
                     // For now, let's assume add_transaction also initializes CounterpartyDetails in final_context,
                     // and we just need to set the known_record if it was found.
                     match final_context.funding_counterparties.get_mut(counterparty) {
                         Some(fd) => fd.known_record = Some(record.clone()),
                         None => {} // Not a funding counterparty in final_context
                     }
                     match final_context.spending_counterparties.get_mut(counterparty) {
                         Some(sd) => sd.known_record = Some(record.clone()),
                         None => {} // Not a spending counterparty in final_context
                     }
                }
                // If details.known_record was None, it remains None in final_context by default.
            }
        }
    }
    
    context = final_context; // Replace old context with the newly filtered one

    info!(
        "Added {} true external counterparties with {} total transactions to final context for wallet {}",
        true_external_counterparties_added, context.transactions.len(), address
    );

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
        program_addresses_identified.len() // Using the comprehensive list
    );
    info!(
        "  - Filtered out {} addresses with excluded categories from DB",
        category_filtered_counterparties.len()
    );


    info!("Beginning statistical analysis for wallet {}", address);
    let mut mixer_interaction_count = 0;
    let mut bridge_interaction_count = 0;
    let mut cex_interaction_count = 0;
    let mut high_risk_spending = 0;
    let total_spending_interactions = context.spending_counterparties.len();

    for (_counterparty_addr, details) in &context.spending_counterparties {
        if let Some(record) = &details.known_record {
            match record.category.to_lowercase().as_str() {
                cat if cat.contains("mixer") || cat.contains("anonym") => mixer_interaction_count += 1,
                cat if cat.contains("bridge") || cat.contains("cross-chain") => bridge_interaction_count += 1,
                cat if cat.contains("exchange") || cat.contains("cex") => cex_interaction_count += 1,
                _ => {}
            }
            if record.risk_level == "High" || record.risk_level == "Critical" {
                high_risk_spending += 1;
            }
        }
    }

    info!(
        "Interaction statistics for {}: mixers={}, bridges={}, exchanges={}, high_risk_spending={}",
        address,
        mixer_interaction_count,
        bridge_interaction_count,
        cex_interaction_count,
        high_risk_spending
    );
    
    let effective_total_spending_interactions = if total_spending_interactions == 0 { 1 } else { total_spending_interactions };

    context.heuristic_flags.risky_spending_destination_ratio =
        high_risk_spending as f32 / effective_total_spending_interactions as f32;

    if context.transaction_timestamps.len() > 50 {
        context.heuristic_flags.is_high_frequency = true;
    }

    if context.transactions.len() > 20 && context.total_sol_volume_out > 0.000001 { // Use a small epsilon for float comparison
        let num_txns_for_avg = context.transactions.len() as f64; // transactions is not empty here
        let avg_tx_size = context.total_sol_volume_out / num_txns_for_avg;
        if avg_tx_size < 0.1 && avg_tx_size > 0.000001 { // Avoid division by zero or tiny values for score
            context.heuristic_flags.structuring_score = (0.1_f64 / avg_tx_size).min(1.0_f64) as f32; // Specify float types
        }
    }

    let funds_ratio = if context.total_sol_volume_in > 0.000001 { // Check for minimal volume_in
        (context.total_sol_volume_in - context.total_sol_volume_out).abs()
            / context.total_sol_volume_in
    } else {
        1.0 // No incoming volume, cannot determine pass-through based on this ratio
    };

    if funds_ratio < 0.1 // Less than 10% difference between in and out
        && !context.funding_counterparties.is_empty() 
        && !context.spending_counterparties.is_empty()
    {
        context.heuristic_flags.is_pass_through = true;
    }

    if mixer_interaction_count > 0 {
        context.heuristic_flags.custom_flags.insert(
            "mixer_interaction".to_string(),
            (mixer_interaction_count as f32 / effective_total_spending_interactions as f32).min(1.0),
        );
    }
    if bridge_interaction_count > 0 {
        context.heuristic_flags.custom_flags.insert(
            "bridge_interaction".to_string(),
            (bridge_interaction_count as f32 / effective_total_spending_interactions as f32).min(1.0),
        );
    }
    if cex_interaction_count > 0 {
        context.heuristic_flags.custom_flags.insert(
            "cex_interaction".to_string(),
            (cex_interaction_count as f32 / effective_total_spending_interactions as f32).min(1.0),
        );
    }
    
    info!("Historical context aggregation complete for {}", address);
    info!(
        "Final context stats for {}: {} transactions, {} funding sources, {} spending destinations, {:.4} SOL in, {:.4} SOL out",
        address,
        context.transactions.len(),
        context.funding_counterparties.len(),
        context.spending_counterparties.len(),
        context.total_sol_volume_in,
        context.total_sol_volume_out
    );

    Ok(context)
}