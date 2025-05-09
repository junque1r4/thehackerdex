use crate::db::Repository;
use crate::error::HackerdexError;
use crate::heuristic_engine::types::WalletContext;
use crate::rpc::client::RateLimitedClient; // Import RateLimitedClient
use solana_sdk::pubkey::Pubkey; // Import Pubkey
use std::str::FromStr;
use tokio::time::{Duration, sleep};
use tracing;

/// Maximum number of recent transactions to fetch for context
const MAX_RECENT_TXS: usize = 20;

/// Builds the necessary wallet context for heuristic analysis
/// This function fetches on-chain data and known address information
/// while respecting RPC rate limits
///
/// # Arguments
///
/// * `addresses` - A slice of wallet addresses to build context for
/// * `repo` - Database repository to query known address information
/// * `rpc_client` - Rate-limited RPC client for on-chain data fetching
///
/// # Returns
///
/// A Result containing a Vec of WalletContext objects for each address
pub async fn build_wallet_contexts(
    addresses: &[String],
    repo: &Repository,
    rpc_client: &RateLimitedClient, // Changed RpcClient to RateLimitedClient
) -> Result<Vec<WalletContext>, HackerdexError> {
    let mut contexts = Vec::with_capacity(addresses.len());
    tracing::debug!("Building wallet contexts for {} addresses", addresses.len());

    for address in addresses {
        // Add small delay between addresses to respect rate limits
        if contexts.len() > 0 {
            sleep(Duration::from_millis(100)).await;
        }

        let mut context = WalletContext::new(address.clone());

        // Check if this is a known address in our database
        if let Ok(record) = repo.get_address_details(address).await {
            context.known_address_record = Some(record);
        }

        // Fetch on-chain SOL balance
        // Pubkey parsing can fail, handle it gracefully
        let _pubkey = match Pubkey::from_str(address) {
            Ok(pk) => pk,
            Err(e) => {
                tracing::warn!("Invalid address format for {}: {}, skipping balance and tx fetch", address, e);
                // Potentially push a context with default/error values or skip this address
                contexts.push(context); // Push context with what we have so far
                continue; // Skip to the next address
            }
        };

        match rpc_client.get_balance(&address).await { // Updated to use RateLimitedClient and await
            Ok(balance) => {
                context.sol_balance = balance as f64;
            }
            Err(e) => {
                tracing::warn!("Failed to get balance for {}: {}", address, e);
            }
        }

        // Fetch recent transaction signatures
        // We'll use these to determine tx counts and creation time
        match rpc_client.get_signatures_for_address(&address, None, None, Some(MAX_RECENT_TXS)).await { // Updated to use RateLimitedClient and await
            Ok(signatures) => {
                if !signatures.is_empty() {
                    // The oldest transaction we have might be the wallet creation
                    if let Some(oldest_tx) = signatures.last() {
                        context.creation_timestamp = oldest_tx.block_time;
                    }

                    // Count transactions from last 24h and 7d
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;

                    let day_ago = now - 24 * 60 * 60;
                    let week_ago = now - 7 * 24 * 60 * 60;

                    for sig in &signatures {
                        if let Some(timestamp) = sig.block_time {
                            if timestamp >= day_ago {
                                context.tx_count_24h += 1;
                                // To be fully accurate, we should get tx amount here
                                // For now we're just counting txs
                            }

                            if timestamp >= week_ago {
                                context.tx_count_7d += 1;
                            }
                        }
                    }

                    // Fetch full transaction details for a limited number of recent transactions
                    // to analyze incoming and outgoing patterns
                    // This would be a more complex implementation requiring parsing each tx
                    // to determine if it's incoming or outgoing and extracting counterparty info

                    // For now, we'll skip this part as it would require significant RPC calls
                    // and we'd need to be very careful with rate limits
                    // In a complete implementation, we would:
                    // 1. Get full tx details for a subset of signatures
                    // 2. Parse each tx to determine direction (in/out)
                    // 3. Extract counterparty info and amounts
                    // 4. Check if counterparties are known addresses
                    // 5. Populate recent_incoming_txs and recent_outgoing_txs
                }
            }
            Err(e) => {
                tracing::warn!("Failed to get signatures for {}: {}", address, e);
            }
        }

        contexts.push(context);
    }

    tracing::info!("Built context for {} wallets", contexts.len());
    Ok(contexts)
}
