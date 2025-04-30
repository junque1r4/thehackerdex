use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::db::models::{AddressRecord, TraceLink};
use crate::db::repository::Repository;
use crate::error::{HackerdexError, HackerdexResult};
use crate::rpc::RateLimitedClient;

/// Maximum number of addresses to process in a batch
const MAX_BATCH_SIZE: usize = 100;
/// Maximum hop count for tracing operations
const MAX_HOP_COUNT: i32 = 3;

/// Configuration for the fund tracer
#[derive(Debug, Clone)]
pub struct FundTracerConfig {
    /// Categories of addresses to consider high-risk
    pub high_risk_categories: Vec<String>,
    /// Risk levels to consider high-risk
    pub high_risk_levels: Vec<String>,
    /// Maximum number of hops to trace
    pub max_hop_count: i32,
    /// Maximum batch size for processing addresses
    pub max_batch_size: usize,
}

impl Default for FundTracerConfig {
    fn default() -> Self {
        Self {
            high_risk_categories: vec!["Known Hacker".to_string(), "Sanctioned Entity".to_string()],
            high_risk_levels: vec!["High".to_string(), "Critical".to_string()],
            max_hop_count: MAX_HOP_COUNT,
            max_batch_size: MAX_BATCH_SIZE,
        }
    }
}

/// A service for tracing funds between addresses
pub struct FundTracer {
    /// Repository for database access
    repo: Repository,
    /// RPC client for blockchain interaction
    rpc: RateLimitedClient,
    /// Configuration for the tracer
    config: FundTracerConfig,
    /// Set of processed addresses to avoid duplicates
    processed_addresses: Arc<RwLock<HashSet<String>>>,
}

impl FundTracer {
    /// Create a new FundTracer instance
    pub fn new(repo: Repository, rpc: RateLimitedClient, config: FundTracerConfig) -> Self {
        Self {
            repo,
            rpc,
            config,
            processed_addresses: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Initialize a fund tracer with default configuration
    pub fn with_defaults(repo: Repository, rpc: RateLimitedClient) -> Self {
        Self::new(repo, rpc, FundTracerConfig::default())
    }

    /// Get high-risk addresses based on configured criteria
    pub async fn get_high_risk_addresses(&self) -> HackerdexResult<Vec<AddressRecord>> {
        self.repo
            .get_addresses_by_criteria(
                &self.config.high_risk_categories,
                &self.config.high_risk_levels,
            )
            .await
    }

    /// Trace funds going out from a given address (forward tracing)
    /// Returns a list of trace links discovered
    pub async fn trace_funds_forward(
        &self,
        source_address: &str,
        max_hops: Option<i32>,
    ) -> HackerdexResult<Vec<TraceLink>> {
        let max_hops = max_hops.unwrap_or(self.config.max_hop_count);
        if max_hops <= 0 {
            return Err(HackerdexError::ConfigError(
                "max_hops must be greater than 0".to_string(),
            ));
        }

        // Get outgoing transactions from the address via RPC
        println!("Fetching outgoing transfers for: {}", source_address);
        let target_addresses = match self.get_outgoing_transfers(source_address).await {
            Ok(addresses) => {
                println!(
                    "Tracer: Found {} outgoing transfers: {:?}",
                    addresses.len(),
                    addresses
                );
                addresses
            }
            Err(e) => {
                eprintln!(
                    "Error fetching outgoing transfers for {}: {}",
                    source_address, e
                );
                return Ok(Vec::new());
            }
        };

        println!(
            "After RPC call, target_addresses.len() = {}",
            target_addresses.len()
        );

        // For demo purposes, we'll create mock trace links rather than relying on database
        let mut discovered_links = Vec::new();

        // Current time for timestamps
        let current_time = sqlx::types::time::OffsetDateTime::now_utc();

        // Create mock trace links for each target address
        for (i, target_address) in target_addresses.iter().enumerate() {
            let link = TraceLink {
                id: i as i32 + 1,
                source_address: source_address.to_string(),
                target_address: target_address.clone(),
                relationship_type: "forward_trace_hop".to_string(),
                trace_initiator: source_address.to_string(),
                hop_count: 1,
                discovery_timestamp: current_time,
                transaction_signature: Some(format!("mock_sig_{}", i)),
            };

            discovered_links.push(link);
        }

        Ok(discovered_links)
    }

    /// Trace funds coming in to a given address (backward tracing)
    /// Returns a list of trace links discovered
    pub async fn trace_funds_backward(
        &self,
        target_address: &str,
        max_hops: Option<i32>,
    ) -> HackerdexResult<Vec<TraceLink>> {
        let max_hops = max_hops.unwrap_or(self.config.max_hop_count);
        if max_hops <= 0 {
            return Err(HackerdexError::ConfigError(
                "max_hops must be greater than 0".to_string(),
            ));
        }

        // Get incoming transactions to the address via RPC
        let source_addresses = match self.get_incoming_transfers(target_address).await {
            Ok(addresses) => addresses,
            Err(e) => {
                eprintln!(
                    "Error fetching incoming transfers for {}: {}",
                    target_address, e
                );
                return Ok(Vec::new());
            }
        };

        // For demo purposes, we'll create mock trace links rather than relying on database
        let mut discovered_links = Vec::new();

        // Current time for timestamps
        let current_time = sqlx::types::time::OffsetDateTime::now_utc();

        // Create mock trace links for each source address
        for (i, source_address) in source_addresses.iter().enumerate() {
            let link = TraceLink {
                id: i as i32 + 1,
                source_address: source_address.clone(),
                target_address: target_address.to_string(),
                relationship_type: "backward_trace_hop".to_string(),
                trace_initiator: target_address.to_string(),
                hop_count: 1,
                discovery_timestamp: current_time,
                transaction_signature: Some(format!("mock_sig_b_{}", i)),
            };

            discovered_links.push(link);
        }

        Ok(discovered_links)
    }

    /// Find addresses that have received funds from high-risk sources
    pub async fn find_high_risk_fund_recipients(
        &self,
        max_hops: Option<i32>,
    ) -> HackerdexResult<Vec<TraceLink>> {
        // Get high-risk addresses based on our criteria
        let high_risk_addresses = self.get_high_risk_addresses().await?;
        let mut all_discovered_links = Vec::new();

        // Process each high-risk address in batches
        for chunk in high_risk_addresses.chunks(self.config.max_batch_size) {
            let mut tasks = Vec::new();

            // Create a task for each address in this batch
            for address in chunk {
                let address_clone = address.address.clone();
                let self_clone = self.clone();
                let max_hops_clone = max_hops;

                // Spawn a task to trace funds forward from this address
                let task = tokio::spawn(async move {
                    self_clone
                        .trace_funds_forward(&address_clone, max_hops_clone)
                        .await
                });

                tasks.push(task);
            }

            // Wait for all tasks to complete and collect results
            for task in tasks {
                match task.await {
                    Ok(result) => match result {
                        Ok(links) => all_discovered_links.extend(links),
                        Err(e) => eprintln!("Error in fund tracing: {}", e),
                    },
                    Err(e) => eprintln!("Task join error: {}", e),
                }
            }
        }

        Ok(all_discovered_links)
    }

    /// Find addresses that have sent funds to your monitored destinations
    pub async fn find_suspicious_fund_sources(
        &self,
        monitored_addresses: &[String],
        max_hops: Option<i32>,
    ) -> HackerdexResult<Vec<TraceLink>> {
        let mut all_discovered_links = Vec::new();

        // Process each monitored address in batches
        for chunk in monitored_addresses.chunks(self.config.max_batch_size) {
            let mut tasks = Vec::new();

            // Create a task for each address in this batch
            for address in chunk {
                let address_clone = address.clone();
                let self_clone = self.clone();
                let max_hops_clone = max_hops;

                // Spawn a task to trace funds backward from this address
                let task = tokio::spawn(async move {
                    self_clone
                        .trace_funds_backward(&address_clone, max_hops_clone)
                        .await
                });

                tasks.push(task);
            }

            // Wait for all tasks to complete and collect results
            for task in tasks {
                match task.await {
                    Ok(result) => match result {
                        Ok(links) => all_discovered_links.extend(links),
                        Err(e) => eprintln!("Error in fund tracing: {}", e),
                    },
                    Err(e) => eprintln!("Task join error: {}", e),
                }
            }
        }

        Ok(all_discovered_links)
    }

    /// Helper method: Get outgoing transfers from an address
    async fn get_outgoing_transfers(&self, address: &str) -> HackerdexResult<Vec<String>> {
        // Use our RPC client to fetch outgoing transfers
        match self.rpc.get_outgoing_transfers(address).await {
            Ok(recipients) => {
                // Filter out any recipients that aren't in our database
                let mut valid_recipients = Vec::new();

                for recipient in recipients {
                    // Check if this address is in our database before including it
                    if let Ok(_) = self.repo.get_address_details(&recipient).await {
                        valid_recipients.push(recipient);
                    }
                }

                Ok(valid_recipients)
            }
            Err(e) => Err(e),
        }
    }

    /// Helper method: Get incoming transfers to an address
    async fn get_incoming_transfers(&self, address: &str) -> HackerdexResult<Vec<String>> {
        // Use our RPC client to fetch incoming transfers
        match self.rpc.get_incoming_transfers(address).await {
            Ok(senders) => {
                // Filter out any senders that aren't in our database
                let mut valid_senders = Vec::new();

                for sender in senders {
                    // Check if this address is in our database before including it
                    if let Ok(_) = self.repo.get_address_details(&sender).await {
                        valid_senders.push(sender);
                    }
                }

                Ok(valid_senders)
            }
            Err(e) => Err(e),
        }
    }
}

impl Clone for FundTracer {
    fn clone(&self) -> Self {
        Self {
            repo: Repository::new(self.repo.pool.clone()),
            rpc: self.rpc.clone(),
            config: self.config.clone(),
            processed_addresses: self.processed_addresses.clone(),
        }
    }
}
