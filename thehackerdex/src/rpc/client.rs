use anyhow::{Context, Result};
use governor::{
    Quota, RateLimiter,
    clock::DefaultClock,
    middleware::NoOpMiddleware,
    state::{InMemoryState, NotKeyed},
};
use rand::random;
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::RpcTransactionConfig,
    rpc_response::{RpcConfirmedTransactionStatusWithSignature, RpcVersionInfo},
};
use solana_sdk::{
    account::Account, commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Signature,
};
use solana_transaction_status::UiTransactionEncoding;
use solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta;
use std::{
    num::NonZeroU32,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Default Solana mainnet RPC endpoint
pub const DEFAULT_RPC_ENDPOINT: &str = "https://api.mainnet-beta.solana.com";

/// Rate limits for Solana public RPC endpoint, documented at:
/// https://docs.solana.com/api/http
const MAX_REQUESTS_PER_10S: u32 = 500;
const MAX_REQUESTS_PER_10S_PER_METHOD: u32 = 50;
#[allow(dead_code)]
const MAX_CONNECTIONS_PER_IP: u32 = 40;
#[allow(dead_code)]
const MAX_CONNECTION_RATE_PER_10S: u32 = 40;

/// RateLimit failure backoff strategy
const INITIAL_BACKOFF_MS: u64 = 1000; // 1 second (increased from 500ms)
const MAX_BACKOFF_MS: u64 = 30000; // 30 seconds (increased from 15 seconds)
const BACKOFF_FACTOR: f64 = 1.5;

/// Represents a rate-limited Solana RPC client with retry capabilities.
#[derive(Clone)]
pub struct RateLimitedClient {
    /// The underlying RPC client from solana_client
    client: Arc<RpcClient>,

    /// The RPC endpoint URL
    #[allow(dead_code)]
    endpoint: String,

    /// Global rate limiter for all requests
    global_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>>,

    /// Method-specific rate limiters
    method_limiters: Arc<
        Mutex<
            std::collections::HashMap<
                String,
                Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>>,
            >,
        >,
    >,
}

impl RateLimitedClient {
    /// Creates a new rate-limited client with the given RPC endpoint URL.
    pub fn new(url: Option<String>) -> Self {
        let endpoint = url.unwrap_or_else(|| DEFAULT_RPC_ENDPOINT.to_string());

        info!("Creating RPC client with endpoint: {}", endpoint);

        // Create global rate limiter: 100 requests per 10 seconds
        let global_quota = Quota::with_period(Duration::from_secs(10))
            .expect("Invalid rate limit period")
            .allow_burst(NonZeroU32::new(MAX_REQUESTS_PER_10S).unwrap());
        let global_limiter = Arc::new(RateLimiter::direct(global_quota));

        // Create client with reasonable timeout settings
        let client = Arc::new(RpcClient::new_with_timeout_and_commitment(
            endpoint.clone(),
            Duration::from_secs(60), // 60 second timeout (increased from 30s)
            CommitmentConfig::confirmed(),
        ));

        Self {
            client,
            endpoint,
            global_limiter,
            method_limiters: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Returns the RPC endpoint URL used by this client
    #[allow(dead_code)]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Get or create a method-specific rate limiter
    async fn get_method_limiter(
        &self,
        method: &str,
    ) -> Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>> {
        let mut limiters = self.method_limiters.lock().await;

        if let Some(limiter) = limiters.get(method) {
            limiter.clone()
        } else {
            // Create new limiter: 40 requests per 10 seconds for this specific method
            let method_quota = Quota::with_period(Duration::from_secs(10))
                .expect("Invalid rate limit period")
                .allow_burst(NonZeroU32::new(MAX_REQUESTS_PER_10S_PER_METHOD).unwrap());

            let method_limiter = Arc::new(RateLimiter::direct(method_quota));
            limiters.insert(method.to_string(), method_limiter.clone());
            method_limiter
        }
    }

    /// Execute an RPC call with rate limiting and retries
    async fn execute_with_rate_limit<F, T>(&self, method: &str, _key: &str, f: F) -> Result<T>
    where
        F: Fn() -> std::result::Result<T, solana_client::client_error::ClientError>,
    {
        let method_limiter = self.get_method_limiter(method).await;
        let mut backoff_ms = INITIAL_BACKOFF_MS;

        // Loop for retries
        loop {
            // Wait for both global and method-specific rate limiting permissions
            self.global_limiter.until_ready().await;
            method_limiter.until_ready().await;

            debug!("Executing RPC method: {}", method);
            let start = Instant::now();

            match f() {
                Ok(result) => {
                    let elapsed = start.elapsed();
                    debug!(
                        "RPC method {} completed in {:.2}ms",
                        method,
                        elapsed.as_millis()
                    );
                    return Ok(result);
                }
                Err(err) => {
                    let error_message = err.to_string().to_lowercase();

                    // Handle different error types
                    if error_message.contains("rate limit") || error_message.contains("429") {
                        warn!(
                            "Rate limit hit for method {}, backing off for {}ms",
                            method, backoff_ms
                        );
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;

                        // Exponential backoff with jitter
                        let jitter = random::<f64>() * 0.1 * (backoff_ms as f64);
                        backoff_ms = ((backoff_ms as f64 * BACKOFF_FACTOR) + jitter) as u64;
                        backoff_ms = backoff_ms.min(MAX_BACKOFF_MS);

                        // Continue retry loop
                        continue;
                    } else if error_message.contains("503") || error_message.contains("timeout") {
                        // Server error or timeout, retry with backoff
                        warn!(
                            "Server error or timeout, retrying in {}ms: {}",
                            backoff_ms, err
                        );
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;

                        // Linear backoff for these errors
                        backoff_ms = (backoff_ms + 500).min(MAX_BACKOFF_MS);
                        continue;
                    } else {
                        // Other error, don't retry
                        error!("RPC error ({}): {}", method, err);
                        return Err(anyhow::Error::new(err)
                            .context(format!("Failed to execute RPC method {}", method)));
                    }
                }
            }
        }
    }

    /// Get Solana RPC API version information
    pub async fn get_version(&self) -> Result<RpcVersionInfo> {
        self.execute_with_rate_limit("getVersion", "version", || self.client.get_version())
            .await
    }

    /// Get account information for the given Solana address
    pub async fn get_account_info(&self, address: &str) -> Result<Option<Account>> {
        let pubkey =
            Pubkey::from_str(address).context(format!("Invalid Solana address: {}", address))?;

        self.execute_with_rate_limit("getAccountInfo", address, || {
            match self.client.get_account(&pubkey) {
                Ok(account) => Ok(Some(account)),
                Err(err) => {
                    if err.to_string().contains("AccountNotFound") {
                        Ok(None)
                    } else {
                        Err(err)
                    }
                }
            }
        })
        .await
    }

    /// Get multiple accounts in a single RPC request
    pub async fn get_multiple_accounts(
        &self,
        addresses: &[String],
    ) -> Result<Vec<Option<Account>>> {
        if addresses.is_empty() {
            return Ok(Vec::new());
        }

        // Convert address strings to Pubkeys
        let mut pubkeys = Vec::with_capacity(addresses.len());
        for address in addresses {
            match Pubkey::from_str(address) {
                Ok(pubkey) => pubkeys.push(pubkey),
                Err(err) => {
                    warn!(
                        "Invalid Solana address skipped in batch: {}: {}",
                        address, err
                    );
                    // Return a None for this invalid address to maintain index alignment
                    return Err(anyhow::Error::msg(format!(
                        "Invalid Solana address in batch: {}: {}",
                        address, err
                    )));
                }
            }
        }

        // Key for rate limiting - use first address or "batch" if empty
        let key = addresses.first().map_or("batch", |s| s.as_str());

        self.execute_with_rate_limit("getMultipleAccounts", key, || {
            match self.client.get_multiple_accounts(&pubkeys) {
                Ok(accounts) => Ok(accounts),
                Err(err) => Err(err),
            }
        })
        .await
    }

    /// Get the balance of a Solana address in lamports
    #[allow(dead_code)]
    pub async fn get_balance(&self, address: &str) -> Result<u64> {
        let pubkey =
            Pubkey::from_str(address).context(format!("Invalid Solana address: {}", address))?;

        self.execute_with_rate_limit("getBalance", address, || self.client.get_balance(&pubkey))
            .await
    }

    /// Get signatures (transaction history) for an address with pagination
    pub async fn get_signatures_for_address(
        &self,
        address: &str,
        before: Option<&Signature>,
        until: Option<&Signature>,
        limit: Option<usize>,
    ) -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>> {
        let pubkey =
            Pubkey::from_str(address).context(format!("Invalid Solana address: {}", address))?;

        let limit = limit.unwrap_or(100).min(1000); // Cannot exceed 1000 per request

        self.execute_with_rate_limit("getSignaturesForAddress", address, || {
            self.client.get_signatures_for_address_with_config(
                &pubkey,
                solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config {
                    before: before.cloned(),
                    until: until.cloned(),
                    limit: Some(limit),
                    commitment: Some(CommitmentConfig::confirmed()),
                },
            )
        })
        .await
    }

    /// Get detailed transaction information for a transaction signature
    pub async fn get_transaction(
        &self,
        signature: &str,
    ) -> Result<Option<EncodedConfirmedTransactionWithStatusMeta>> {
        let sig = Signature::from_str(signature)
            .context(format!("Invalid transaction signature: {}", signature))?;

        self.execute_with_rate_limit("getTransaction", signature, || {
            match self.client.get_transaction_with_config(
                &sig,
                RpcTransactionConfig {
                    encoding: Some(UiTransactionEncoding::Json),
                    commitment: Some(CommitmentConfig::confirmed()),
                    max_supported_transaction_version: Some(0),
                },
            ) {
                Ok(tx) => Ok(Some(tx)),
                Err(err) => {
                    if err.to_string().contains("TransactionNotFound") {
                        Ok(None)
                    } else {
                        Err(err)
                    }
                }
            }
        })
        .await
    }
}
