use anyhow::Result;
use thehackerdex::rpc::client::RateLimitedClient;
use serde_json::Value;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Write};
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

// Function to ensure output is immediately displayed
fn flush_println(msg: &str) {
    println!("{}", msg);
    io::stdout().flush().unwrap_or(());
}

/// The target number of wallets to collect before stopping
const TARGET_WALLET_COUNT: usize = 1000;

/// The number of signatures to fetch in each request
const BATCH_SIZE: usize = 100;

/// The delay between requests to avoid overloading the RPC server
const REQUEST_DELAY_MS: u64 = 2000;

/// Collect wallets by analyzing transactions from a known address
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for logging with immediate output
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stdout)
        .init();

    flush_println("Starting wallet collector...");

    // The known address to start collecting from (e.g., a major protocol or exchange hot wallet)
    let known_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";

    // Initialize Solana RPC client with a custom endpoint
    let rpc_url = std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| {
        "https://mainnet.helius-rpc.com/?api-key=10382f6d-824b-4962-9933-f695d6680b98".to_string()
    });

    flush_println(&format!("Using RPC URL: {}", rpc_url));

    // Try creating a client with a timeout to check connectivity
    flush_println("Initializing RPC client and testing connection...");
    let client = RateLimitedClient::new(Some(rpc_url));

    // Convert the known address string to a Pubkey
    let known_pubkey = match Pubkey::from_str(known_address) {
        Ok(key) => key,
        Err(e) => {
            flush_println(&format!("Error parsing address: {}", e));
            return Err(anyhow::Error::msg(format!(
                "Invalid Solana address: {}",
                known_address
            )));
        }
    };

    // Test the RPC connection by getting version info
    flush_println("Testing RPC connection...");
    match client.get_version().await {
        Ok(version) => flush_println(&format!(
            "Connection successful! Solana RPC version: {:?}",
            version
        )),
        Err(e) => {
            flush_println(&format!("Failed to connect to RPC server: {}", e));
            flush_println("Please check your internet connection and RPC URL.");
            flush_println(
                "You may need to set a different RPC endpoint using the SOLANA_RPC_URL environment variable.",
            );
            return Err(anyhow::Error::msg("Failed to connect to Solana RPC server"));
        }
    }

    flush_println(&format!(
        "Starting wallet collection from address: {}",
        known_address
    ));
    info!("Starting wallet collection from address: {}", known_address);

    // Set to store unique wallet addresses
    let mut collected_wallets: HashSet<String> = HashSet::new();

    // Add the known address to the set
    collected_wallets.insert(known_address.to_string());

    // Variable to track the last signature for pagination
    // Start from the specified transaction signature
    let starting_tx =
        "2A3piwNB6tgeewTibr1WU5KQDGuWBKHyWn3M58D5x6VxzGnCQA9eCRkb98erLSNvN5vbkAfQsTyMRCwk6PfB7Mke";
    let mut last_signature: Option<Signature> = match Signature::from_str(starting_tx) {
        Ok(sig) => {
            info!("Starting collection from transaction: {}", starting_tx);
            Some(sig)
        }
        Err(e) => {
            warn!(
                "Could not parse starting transaction signature: {}, starting from latest",
                e
            );
            None
        }
    };

    // Continue collecting until we reach the target number of wallets
    while collected_wallets.len() < TARGET_WALLET_COUNT {
        info!(
            "Collected {} wallets. Fetching more...",
            collected_wallets.len()
        );

        // Fetch recent transaction signatures for the known address with pagination
        println!(
            "Fetching transaction signatures for address: {}",
            known_pubkey.to_string()
        );

        let before_sig = last_signature
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "None".to_string());
        println!("Pagination: before signature = {}", before_sig);

        let signatures_result = client
            .get_signatures_for_address(
                &known_pubkey.to_string(),
                last_signature.as_ref(),
                None,
                Some(BATCH_SIZE),
            )
            .await;

        // Handle potential errors from the RPC call
        let signatures = match signatures_result {
            Ok(sigs) => {
                println!("Successfully fetched {} signatures", sigs.len());
                sigs
            }
            Err(e) => {
                println!("Error fetching signatures: {}", e);
                error!("Error fetching signatures: {}", e);
                // Wait before retrying
                println!("Waiting 10 seconds before retry...");
                sleep(Duration::from_secs(10)).await;
                continue;
            }
        };

        // If no signatures were returned, wait and try again
        if signatures.is_empty() {
            info!("No more signatures found. Waiting...");
            sleep(Duration::from_secs(60)).await;
            continue;
        }

        // Update the last signature for pagination
        if let Some(last_sig_info) = signatures.last() {
            last_signature = Signature::from_str(&last_sig_info.signature).ok();
        }

        // Process each signature to extract wallet addresses
        println!("Processing {} transaction signatures", signatures.len());

        for (i, sig_info) in signatures.iter().enumerate() {
            println!(
                "[{}/{}] Getting transaction: {}",
                i + 1,
                signatures.len(),
                sig_info.signature
            );

            // Get transaction details for each signature
            let tx_result = client.get_transaction(&sig_info.signature).await;

            match tx_result {
                Ok(Some(tx)) => {
                    println!("  Transaction retrieved successfully");

                    // Get current wallet count before extraction
                    let before_count = collected_wallets.len();

                    // Extract all possible wallet addresses from the transaction
                    extract_addresses_from_transaction(&tx, &mut collected_wallets);

                    // Get new wallet count after extraction
                    let after_count = collected_wallets.len();
                    let new_wallets = after_count - before_count;

                    println!(
                        "  Found {} new wallet addresses (total now: {})",
                        new_wallets, after_count
                    );
                }
                Ok(None) => {
                    println!("  Transaction not found: {}", sig_info.signature);
                    info!("Transaction not found: {}", sig_info.signature);
                }
                Err(e) => {
                    println!("  Error fetching transaction: {}", e);
                    error!("Error fetching transaction {}: {}", sig_info.signature, e);
                }
            }

            // Avoid rate limiting
            println!("  Waiting {}ms before next request", REQUEST_DELAY_MS);
            sleep(Duration::from_millis(REQUEST_DELAY_MS)).await;
        }

        // Periodically save collected wallets
        if collected_wallets.len() % 100 == 0 || collected_wallets.len() >= TARGET_WALLET_COUNT {
            save_wallets(&collected_wallets, "solana_wallets.txt")?;
        }
    }

    info!(
        "Finished. Collected {} unique wallets.",
        collected_wallets.len()
    );

    // Save the collected wallets to a file
    save_wallets(&collected_wallets, "solana_wallets.txt")?;

    Ok(())
}

/// Extract wallet addresses from a transaction
fn extract_addresses_from_transaction(
    tx: &solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta,
    collected_wallets: &mut HashSet<String>,
) {
    // Helper function to recursively search for public keys in JSON Value
    fn extract_pubkeys_from_value(value: &Value, wallets: &mut HashSet<String>) {
        match value {
            Value::String(s) if is_likely_pubkey(s) => {
                wallets.insert(s.clone());
            }
            Value::Object(obj) => {
                for (_key, val) in obj {
                    extract_pubkeys_from_value(val, wallets);
                }
            }
            Value::Array(arr) => {
                for item in arr {
                    extract_pubkeys_from_value(item, wallets);
                }
            }
            _ => {}
        }
    }

    // Serialize the entire transaction to JSON
    if let Ok(tx_json) = serde_json::to_value(tx) {
        // Extract transaction signature safely from the JSON structure
        let tx_signature = tx_json
            .get("transaction")
            .and_then(|t| t.get("signatures"))
            .and_then(|sigs| sigs.as_array())
            .and_then(|arr| arr.first())
            .and_then(|sig| sig.as_str())
            .unwrap_or("unknown");

        debug!("Processing transaction: {}", tx_signature);
        extract_pubkeys_from_value(&tx_json, collected_wallets);
    } else {
        warn!("Failed to convert transaction to JSON");
    }
}

/// Check if a string is likely to be a Solana public key (address)
fn is_likely_pubkey(s: &str) -> bool {
    // Basic check: Solana addresses are Base58 encoded and 32-44 characters long
    if s.len() < 32 || s.len() > 44 {
        return false;
    }

    // Check if it contains only Base58 characters (alphanumeric excluding 0, O, I, and l)
    let is_base58 = s.chars().all(|c| {
        (c >= 'a' && c <= 'z' && c != 'l')
            || (c >= 'A' && c <= 'Z' && c != 'O' && c != 'I')
            || (c >= '1' && c <= '9')
    });

    // Try parsing it as a Pubkey for final validation
    if is_base58 {
        return Pubkey::from_str(s).is_ok();
    }

    false
}

/// Save collected wallet addresses to a file
fn save_wallets(wallets: &HashSet<String>, filename: &str) -> Result<()> {
    let mut file = File::create(filename)?;

    for wallet in wallets {
        writeln!(file, "{}", wallet)?;
    }

    info!("Saved {} wallets to {}", wallets.len(), filename);
    Ok(())
}
