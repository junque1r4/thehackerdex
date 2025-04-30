use crate::rpc::RateLimitedClient;
// Import the db module from the workspace
use anyhow::Result;
use tracing::{info, warn};

/// Run a demonstration of the RPC client functionality
pub async fn run_rpc_demo() -> Result<()> {
    info!("Starting RPC client demonstration");

    // Create a new rate-limited client
    let client = RateLimitedClient::new(None);

    // Get Solana version information
    info!("Getting Solana version information...");
    let version = client.get_version().await?;
    info!("Solana version: {:#?}", version);

    // Sample Solana addresses to test with
    let addresses = [
        // Wormhole Core bridge contract
        "worm2ZoG2kUd4vFXhvjh93UUH596ayRfgQ2MgjNMTth",
        // Jupiter aggregator v6
        "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4",
        // A random user wallet
        "EXWkjXgJR7u2GJJ9BeF5GVxYbPg3APGrNJBCAa3JiYje",
    ];

    // Test getting account information for each address
    for address in addresses.iter() {
        info!("Fetching account info for {}...", address);
        match client.get_account_info(address).await {
            Ok(Some(account)) => {
                info!(
                    "Account found for {}: {} lamports, {} bytes of data",
                    address,
                    account.lamports,
                    account.data.len()
                );
            }
            Ok(None) => {
                warn!("No account found for {}", address);
            }
            Err(e) => {
                warn!("Error fetching account for {}: {}", address, e);
            }
        }

        // Get recent transaction signatures
        info!("Fetching recent transactions for {}...", address);
        match client
            .get_signatures_for_address(address, None, None, Some(5))
            .await
        {
            Ok(signatures) => {
                info!(
                    "Found {} recent transactions for {}",
                    signatures.len(),
                    address
                );

                // Get details for the first transaction if available
                if let Some(first_sig) = signatures.first() {
                    info!(
                        "Fetching details for transaction {}...",
                        first_sig.signature
                    );
                    match client.get_transaction(&first_sig.signature).await {
                        Ok(Some(tx)) => {
                            info!(
                                "Transaction details: slot {}, block time: {:?}",
                                tx.slot, tx.block_time
                            );
                        }
                        Ok(None) => {
                            warn!("Transaction not found: {}", first_sig.signature);
                        }
                        Err(e) => {
                            warn!("Error fetching transaction {}: {}", first_sig.signature, e);
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Error fetching transactions for {}: {}", address, e);
            }
        }

        // Add a small delay between tests to avoid rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    info!("RPC client demonstration completed");
    Ok(())
}
