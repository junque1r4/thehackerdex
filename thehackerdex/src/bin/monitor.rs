use thehackerdex::{
    config::Config,
    db::{self, repository::Repository},
    rpc::{TransactionFetcher, client::RateLimitedClient},
};

use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{env, sync::Arc, time::Duration};
use tokio::{signal, time};
use tracing::{Level, error, info};
use tracing_subscriber::FmtSubscriber;

/// The main function for the transaction monitor binary
///
/// This is a standalone binary for monitoring transactions on Solana
/// and detecting potentially risky wallets based on configured criteria.
/// The monitor runs continuously until manually stopped and can automatically
/// add high-risk wallets to the database.
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging with tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    info!("Starting HackerDex Transaction Monitor");

    // Load configuration
    let config = Config::from_env();

    // Try to load monitoring configuration, use default if not found
    let monitoring_config = match config.load_monitoring_config() {
        Ok(Some(cfg)) => {
            info!("Loaded monitoring configuration successfully");
            cfg
        }
        Ok(None) => {
            info!("No monitoring configuration found, using defaults");
            thehackerdex::config::monitoring::MonitoringConfig::default()
        }
        Err(e) => {
            error!("Failed to load monitoring configuration: {}", e);
            thehackerdex::config::monitoring::MonitoringConfig::default()
        }
    };

    // Log monitoring targets
    info!(
        "Monitoring {} program IDs and {} watch addresses",
        monitoring_config.target_programs.len(),
        monitoring_config.watch_addresses.len()
    );

    // Initialize database connection
    info!("Initializing database connection...");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    db::initialize_db(&pool).await?;
    info!("Database initialized successfully");

    // Initialize repository for database operations
    let _repository = Repository::new(pool.clone());

    // Initialize RPC client
    let rpc_endpoint = env::var("SOLANA_RPC_URL").expect("SOLANA_RPC_URL must be set");
    let rpc_client = RateLimitedClient::new(Some(rpc_endpoint.clone()));
    info!("RPC client initialized with endpoint: {}", rpc_endpoint);

    // Initialize transaction fetcher with config
    let transaction_fetcher = TransactionFetcher::new(monitoring_config, rpc_client);

    // Set up graceful shutdown handling with Arc<AtomicBool>
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("Shutdown signal received. Gracefully shutting down...");
                r.store(false, Ordering::SeqCst);
            }
            Err(err) => {
                error!("Error setting up Ctrl+C handler: {}", err);
            }
        }
    });

    info!("Transaction monitor is now running. Press Ctrl+C to stop.");

    // Start transaction fetcher
    let fetcher = Arc::new(transaction_fetcher);
    let fetcher_clone = fetcher.clone();

    let fetcher_task = tokio::spawn(async move {
        if let Err(e) = fetcher_clone.start().await {
            error!("Transaction fetcher failed: {}", e);
        }
    });

    // Wait for shutdown signal
    while running.load(Ordering::SeqCst) {
        // Wait a bit before checking the shutdown flag again
        time::sleep(Duration::from_secs(1)).await;
    }

    // Stop the transaction fetcher when shutting down
    info!("Stopping transaction fetcher...");
    fetcher.stop().await;

    // Wait for fetcher task to complete
    if let Err(e) = tokio::time::timeout(Duration::from_secs(10), fetcher_task).await {
        error!(
            "Transaction fetcher did not stop cleanly within timeout: {}",
            e
        );
    }

    info!("Shutting down Transaction Monitor...");

    // Perform any cleanup operations needed
    // (none required at the moment, but space for future additions)

    info!("Transaction Monitor shutdown complete");
    Ok(())
}
