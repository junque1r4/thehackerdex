use hackerdex::analysis::program_analyzer;
use hackerdex::analysis::transaction_parser::{ParsedTransaction, parse_transaction};
use hackerdex::config; // Import the config module itself
use hackerdex::db; // Import the db module itself
use hackerdex::demo; // Import the demo module itself
use hackerdex::rpc::client::RateLimitedClient;

use anyhow::Result;
use solana_transaction_status::UiTransactionTokenBalance;
use sqlx::PgPool;
use std::env;
use tracing::{Level, error, info, warn};
use tracing_subscriber::FmtSubscriber;

/// Demonstrates the program analyzer functionality with a sample transaction
async fn run_program_analyzer_demo(pool: &PgPool) -> Result<()> {
    info!("=== Program Analyzer Demo ===");

    // Create a sample transaction or fetch a real one
    info!("Creating sample transaction for analysis...");

    // Option 1: Create a mock transaction with known program IDs
    let mock_transaction = create_mock_transaction();

    info!("Analyzing mock transaction with program IDs:");
    for program_id in &mock_transaction.program_ids {
        info!(" - {}", program_id);
    }

    // Cross-reference the program IDs against the database
    let analysis_results =
        match program_analyzer::analyze_transaction_programs(pool, &mock_transaction).await {
            Ok(results) => {
                info!("Successfully analyzed transaction programs");
                results
            }
            Err(e) => {
                error!("Failed to analyze transaction: {}", e);
                return Err(anyhow::anyhow!("Program analysis failed: {}", e));
            }
        };

    // Print the analysis results
    info!("Analysis results:");
    for result in &analysis_results {
        if result.is_known {
            if let Some(record) = &result.address_record {
                info!(" - Program: {} ({})", result.program_id, record.entity_name);
                info!("   Category: {}", record.category);
                info!("   Risk Level: {}", record.risk_level);
                info!("   Confidence: {}/5", record.confidence_score);
                if let Some(notes) = &record.notes {
                    info!("   Notes: {}", notes);
                }
            }
        } else {
            warn!(" - Unknown program: {}", result.program_id);
        }
    }

    // Get categorization summary
    match program_analyzer::categorize_transaction(pool, &mock_transaction).await {
        Ok(summary) => {
            info!("Transaction summary:\n{}", summary);
        }
        Err(e) => {
            error!("Failed to categorize transaction: {}", e);
        }
    }

    // Option 2: If RPC is available, fetch a real transaction
    if let Ok(rpc_endpoint) = env::var("SOLANA_RPC_URL") {
        info!("Attempting to fetch a real transaction for analysis...");
        match fetch_and_analyze_real_transaction(pool, &rpc_endpoint).await {
            Ok(_) => info!("Real transaction analysis completed"),
            Err(e) => warn!("Could not analyze real transaction: {}", e),
        }
    } else {
        info!("Skipping real transaction analysis (SOLANA_RPC_URL not set)");
    }

    info!("Program analyzer demo completed");
    Ok(())
}

/// Creates a mock transaction for analysis
fn create_mock_transaction() -> ParsedTransaction {
    ParsedTransaction {
        signature: "mock_signature_for_demo".to_string(),
        program_ids: vec![
            // System Program (should be recognized)
            "11111111111111111111111111111111".to_string(),
            // SPL Token (should be recognized)
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
            // Jupiter Aggregator v6 (should be recognized if in DB)
            "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4".to_string(),
            // MarginFi (M2) Bank (should be recognized if in DB)
            "mrgnFMdcVuF8M5SzvmE33dgJ65p8uJxfNsjhf6nyFpk".to_string(),
            // Raydium AMM (should be recognized if in DB)
            "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(),
            // Made-up program (should be unknown)
            "UnknownProgram111111111111111111111111111111".to_string(),
        ],
        involved_accounts: vec![
            "DemoWallet111111111111111111111111111111111".to_string(),
            "AnotherWallet22222222222222222222222222222222".to_string(),
        ],
        pre_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        post_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        execution_status: Some("confirmed".to_string()),
        fee: 5000,
    }
}

/// Fetches a real transaction from Solana and analyzes it
async fn fetch_and_analyze_real_transaction(pool: &PgPool, rpc_endpoint: &str) -> Result<()> {
    // Use a known transaction signature or fetch a recent one
    // This is just an example signature - it might not exist on the network you're using
    let signature =
        "4Mev6SqRFupoFN1uA6YNaDdMHJ3F1RzKm8CeTmkSZsbDGjR34SRmkYqBSjfUCuz8nrpZFRJjPzcRJMozPkRDv1zB";

    info!("Fetching transaction with signature: {}", signature);

    // Create an RPC client
    let rpc_client = RateLimitedClient::new(Some(rpc_endpoint.to_string()));

    // Get the transaction
    let transaction = match rpc_client.get_transaction(signature).await {
        Ok(tx) => {
            info!("Successfully fetched transaction");
            match tx {
                Some(tx) => tx,
                None => {
                    warn!("Transaction not found");
                    return Err(anyhow::anyhow!("Transaction not found"));
                }
            }
        }
        Err(e) => {
            warn!("Failed to fetch transaction: {}", e);
            return Err(anyhow::anyhow!("Failed to fetch transaction: {}", e));
        }
    };

    // Parse the transaction
    let parsed_tx = match parse_transaction(&transaction, signature) {
        Ok(tx) => {
            info!(
                "Successfully parsed transaction with {} program IDs",
                tx.program_ids.len()
            );
            info!("Program IDs in transaction:");
            for program_id in &tx.program_ids {
                info!(" - {}", program_id);
            }
            tx
        }
        Err(e) => {
            warn!("Failed to parse transaction: {}", e);
            return Err(anyhow::anyhow!("Failed to parse transaction: {}", e));
        }
    };

    // Analyze the transaction
    info!("Analyzing real transaction programs...");
    match program_analyzer::categorize_transaction(pool, &parsed_tx).await {
        Ok(summary) => {
            info!("Real transaction analysis summary:\n{}", summary);
        }
        Err(e) => {
            warn!("Failed to analyze real transaction: {}", e);
            return Err(anyhow::anyhow!("Failed to analyze transaction: {}", e));
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    info!("Starting HackerDex Solana Address Rank System");

    // Load configuration
    let config = config::Config::from_env();
    info!("Loaded configuration: {:?}", config);

    // Initialize database connection
    info!("Initializing database connection...");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url).await?;
    db::initialize_db(&pool).await?;

    // Run migrations
    info!("Running database migrations...");
    match sqlx::migrate!("./migrations").run(&pool).await {
        Ok(_) => info!("Database migrations completed successfully"),
        Err(e) => {
            error!("Migration error: {}", e);
            return Err(e.into());
        }
    }
    info!("Database initialized successfully");

    // Run RPC demo to verify functionality
    info!("Running RPC demo...");
    match demo::run_rpc_demo().await {
        Ok(_) => info!("RPC demo completed successfully"),
        Err(e) => warn!("RPC demo had some issues: {}", e),
    }

    // Run program analyzer demo
    info!("Running program analyzer demo...");
    match run_program_analyzer_demo(&pool).await {
        Ok(_) => info!("Program analyzer demo completed successfully"),
        Err(e) => warn!("Program analyzer demo had some issues: {}", e),
    }

    info!("HackerDex execution completed");
    Ok(())
}
