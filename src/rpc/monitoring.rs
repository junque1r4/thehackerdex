use crate::config::monitoring::{MonitoringConfig, MonitoringStrategy};
use crate::rpc::client::RateLimitedClient;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use solana_client::rpc_response::RpcConfirmedTransactionStatusWithSignature;
use solana_sdk::signature::Signature;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::time;
use tracing::{debug, error, info, warn};

/// Tracks the last seen signature for each monitored address
type SignatureMap = HashMap<String, Option<String>>;

/// Serializable state for persistence
#[derive(Debug, Serialize, Deserialize)]
struct PersistentState {
    /// Last seen signatures per address
    last_signatures: SignatureMap,
}

/// Transaction fetcher that implements monitoring strategies
pub struct TransactionFetcher {
    /// The RPC client for blockchain interactions
    client: RateLimitedClient,

    /// Monitoring configuration
    config: MonitoringConfig,

    /// Last seen signatures per address to avoid re-processing
    last_signatures: Arc<Mutex<SignatureMap>>,

    /// Last poll time to track interval adherence
    last_poll_time: Arc<Mutex<Instant>>,

    /// Whether the fetcher is actively running
    running: Arc<Mutex<bool>>,

    /// Path to state file for persistence
    state_file: PathBuf,
}

impl TransactionFetcher {
    /// Create a new transaction fetcher with the given config and client
    pub fn new(config: MonitoringConfig, client: RateLimitedClient) -> Self {
        Self {
            client,
            config,
            last_signatures: Arc::new(Mutex::new(HashMap::new())),
            last_poll_time: Arc::new(Mutex::new(Instant::now())),
            running: Arc::new(Mutex::new(false)),
            state_file: PathBuf::from("monitoring_state.json"),
        }
    }

    /// Create a new transaction fetcher with custom state file path
    pub fn with_state_file(
        config: MonitoringConfig,
        client: RateLimitedClient,
        state_file: PathBuf,
    ) -> Self {
        Self {
            client,
            config,
            last_signatures: Arc::new(Mutex::new(HashMap::new())),
            last_poll_time: Arc::new(Mutex::new(Instant::now())),
            running: Arc::new(Mutex::new(false)),
            state_file,
        }
    }

    /// Load signature state from disk
    async fn load_state(&self) -> Result<()> {
        if !self.state_file.exists() {
            debug!(
                "No state file found at {:?}, starting with empty state",
                self.state_file
            );
            return Ok(());
        }

        match fs::read_to_string(&self.state_file) {
            Ok(content) => {
                match serde_json::from_str::<PersistentState>(&content) {
                    Ok(state) => {
                        let mut last_signatures = self.last_signatures.lock().await;
                        *last_signatures = state.last_signatures;
                        info!(
                            "Loaded signature state for {} addresses",
                            last_signatures.len()
                        );
                        Ok(())
                    }
                    Err(e) => {
                        warn!("Failed to parse state file: {}", e);
                        Ok(()) // Continue with empty state
                    }
                }
            }
            Err(e) => {
                warn!("Failed to read state file: {}", e);
                Ok(()) // Continue with empty state
            }
        }
    }

    /// Save signature state to disk
    async fn save_state(&self) -> Result<()> {
        let last_signatures = self.last_signatures.lock().await;

        let state = PersistentState {
            last_signatures: last_signatures.clone(),
        };

        let json =
            serde_json::to_string_pretty(&state).context("Failed to serialize monitoring state")?;

        fs::write(&self.state_file, json).context("Failed to write monitoring state to file")?;

        debug!(
            "Saved signature state for {} addresses",
            last_signatures.len()
        );

        Ok(())
    }

    /// Start the transaction fetching process
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if *running {
            warn!("Transaction fetcher is already running");
            return Ok(());
        }

        // Load previous state before starting
        self.load_state().await?;

        *running = true;
        drop(running);

        info!(
            "Starting transaction fetcher with strategy: {:?}",
            self.config.strategy
        );

        match self.config.strategy {
            MonitoringStrategy::Polling => self.run_polling_strategy().await,
            MonitoringStrategy::WebSocket => {
                error!("WebSocket strategy not yet implemented");
                Err(anyhow::anyhow!(
                    "WebSocket monitoring strategy is not yet implemented"
                ))
            }
        }
    }

    /// Stop the transaction fetching process
    pub async fn stop(&self) {
        let mut running = self.running.lock().await;
        if *running {
            // Save state before stopping
            if let Err(e) = self.save_state().await {
                error!("Failed to save signature state: {}", e);
            }
            *running = false;
            info!("Transaction fetcher stopped");
        }
    }

    /// Polling strategy implementation for transaction fetching
    async fn run_polling_strategy(&self) -> Result<()> {
        info!(
            "Starting polling strategy with interval {} seconds",
            self.config.polling_interval_seconds
        );

        // Initialize tracking map for last seen signatures
        let addresses_to_monitor = self.get_monitored_addresses();
        {
            let mut last_signatures = self.last_signatures.lock().await;
            for address in &addresses_to_monitor {
                if !last_signatures.contains_key(address) {
                    last_signatures.insert(address.clone(), None);
                }
            }
        }

        // Continue polling while running flag is true
        while *self.running.lock().await {
            let poll_start = Instant::now();
            *self.last_poll_time.lock().await = poll_start;

            debug!("Polling for {} addresses", addresses_to_monitor.len());

            let fetched_transactions = self.fetch_new_transactions(&addresses_to_monitor).await?;

            if !fetched_transactions.is_empty() {
                info!("Fetched {} new transactions", fetched_transactions.len());

                // Save state after each successful fetch that found new transactions
                if let Err(e) = self.save_state().await {
                    warn!("Failed to save signature state: {}", e);
                }
            } else {
                debug!("No new transactions found in this polling cycle");
            }

            // Calculate time to next poll
            let elapsed = poll_start.elapsed();
            let interval = self.config.polling_interval();

            // Wait until next polling interval if we haven't spent the whole interval time yet
            if elapsed < interval {
                let wait_time = interval - elapsed;
                debug!("Waiting {}s until next polling cycle", wait_time.as_secs());
                time::sleep(wait_time).await;
            }
        }

        Ok(())
    }

    /// Get the combined list of addresses to monitor
    fn get_monitored_addresses(&self) -> HashSet<String> {
        let addresses = self.config.watch_addresses.clone();

        // The design allows us to add more address sources in the future
        // For example, addresses from a database or dynamically tracked addresses

        debug!("Monitoring {} addresses", addresses.len());
        addresses
    }

    /// Fetch new transactions for monitored addresses
    async fn fetch_new_transactions(
        &self,
        addresses: &HashSet<String>,
    ) -> Result<Vec<FetchedTransaction>> {
        let mut new_transactions = Vec::new();
        let mut last_signatures = self.last_signatures.lock().await;

        for address in addresses {
            debug!("Checking for new transactions for address: {}", address);

            let before_signature = match last_signatures.get(address).cloned().flatten() {
                Some(sig_str) => match Signature::from_str(&sig_str) {
                    Ok(sig) => Some(sig),
                    Err(err) => {
                        warn!("Invalid signature in state for {}: {}", address, err);
                        None
                    }
                },
                None => None,
            };

            // Get signatures for address, limited to recent transactions
            let signatures = match self
                .client
                .get_signatures_for_address(address, before_signature.as_ref(), None, Some(50))
                .await
            {
                Ok(sigs) => sigs,
                Err(err) => {
                    warn!("Failed to fetch signatures for {}: {}", address, err);
                    continue;
                }
            };

            debug!("Found {} new signatures for {}", signatures.len(), address);

            if !signatures.is_empty() {
                // Update the last seen signature for this address
                if let Some(latest) = signatures.first() {
                    last_signatures.insert(address.clone(), Some(latest.signature.clone()));
                }

                // For each signature, fetch the full transaction
                for status in signatures {
                    match self.fetch_transaction_details(&status.signature).await {
                        Ok(Some(tx_details)) => {
                            new_transactions.push(FetchedTransaction {
                                address: address.clone(),
                                signature: status.signature.clone(),
                                status,
                                transaction: tx_details,
                            });
                        }
                        Ok(None) => {
                            debug!("Transaction not found for signature: {}", status.signature);
                        }
                        Err(err) => {
                            warn!("Failed to fetch transaction {}: {}", status.signature, err);
                        }
                    }
                }
            }
        }

        Ok(new_transactions)
    }

    /// Fetch detailed transaction information for a signature
    async fn fetch_transaction_details(
        &self,
        signature: &str,
    ) -> Result<
        Option<solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta>,
    > {
        self.client.get_transaction(signature).await
    }
}

/// Represents a fetched transaction with all relevant details
pub struct FetchedTransaction {
    /// The address the transaction was found for
    pub address: String,

    /// The transaction signature (ID)
    pub signature: String,

    /// Basic transaction status information
    pub status: RpcConfirmedTransactionStatusWithSignature,

    /// Detailed transaction information
    pub transaction:
        solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta,
}

/// Helper functions to convert between String and Signature types
trait SignatureConversion {
    fn from_str(s: &str) -> Result<Signature>;
}

impl SignatureConversion for Signature {
    fn from_str(s: &str) -> Result<Signature> {
        s.parse::<Signature>().context("Failed to parse signature")
    }
}
