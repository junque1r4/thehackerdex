use thehackerdex::analysis::transaction_analysis::perform_comprehensive_analysis;
use thehackerdex::analysis::transaction_parser::ParsedTransaction;
use thehackerdex::db::repository::initialize_db;
use thehackerdex::error::HackerdexResult;
use solana_client::rpc_client::RpcClient;
use solana_transaction_status::UiTransactionTokenBalance;
use sqlx::postgres::PgPoolOptions;
use tracing::{Level, info};

#[tokio::main]
async fn main() -> HackerdexResult<()> {
    // Setup tracing with minimal debug output to see the risk scores
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting transaction analysis test");

    // Connect to the database
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/hackerdex".to_string());

    info!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Initialize the database if needed
    initialize_db(&pool).await?;

    // Add some test addresses to the database
    info!("Adding test data to the database...");

    // Add a malicious program for testing
    sqlx::query!(
        r#"
        INSERT INTO known_addresses (address, entity_name, category, risk_level, source_of_info, confidence_score, notes)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (address) DO UPDATE SET
            entity_name = EXCLUDED.entity_name,
            category = EXCLUDED.category,
            risk_level = EXCLUDED.risk_level,
            source_of_info = EXCLUDED.source_of_info,
            confidence_score = EXCLUDED.confidence_score,
            notes = EXCLUDED.notes
        "#,
        "M4lik1ousXXXnnnnnnnnnnnnnnnnnnnnnnnnnnn1111",
        "Malicious Program",
        "Scam Contract",
        "High",
        "Test Data",
        5,
        Some("High-risk malicious program for testing")
    )
    .execute(&pool)
    .await?;

    // Add a suspicious wallet for testing
    sqlx::query!(
        r#"
        INSERT INTO known_addresses (address, entity_name, category, risk_level, source_of_info, confidence_score, notes)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (address) DO UPDATE SET
            entity_name = EXCLUDED.entity_name,
            category = EXCLUDED.category,
            risk_level = EXCLUDED.risk_level,
            source_of_info = EXCLUDED.source_of_info,
            confidence_score = EXCLUDED.confidence_score,
            notes = EXCLUDED.notes
        "#,
        "SuspWalletXXXXXXXXXXXXXXXXXXXXXXXXXXXXX1111",
        "Suspicious Wallet",
        "Suspicious Entity",
        "Medium",
        "Test Data",
        4,
        Some("Suspicious wallet for testing")
    )
    .execute(&pool)
    .await?;

    // Create a Solana RPC client (using default endpoint)
    let rpc_client = RpcClient::new("https://api.mainnet-beta.solana.com");

    // Create a sample transaction for testing
    // You can modify these values to test different scenarios
    let parsed_tx = ParsedTransaction {
        signature: "test_signature_for_comprehensive_analysis".to_string(),
        program_ids: vec![
            "11111111111111111111111111111111".to_string(), // System Program
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(), // SPL Token
            "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".to_string(), // Associated Token Account
            "M4lik1ousXXXnnnnnnnnnnnnnnnnnnnnnnnnnnn1111".to_string(), // Made up high-risk program
        ],
        involved_accounts: vec![
            "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(), // Random wallet
            "G5E4KhCRcnJWziqEbJ7vnmr2YscwW1hTsx41SHUjJ8YN".to_string(), // Another random wallet
            "SuspWalletXXXXXXXXXXXXXXXXXXXXXXXXXXXXX1111".to_string(),  // Made up suspicious wallet
        ],
        pre_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        post_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        execution_status: Some("confirmed".to_string()),
        fee: 5000,
    };

    // Perform comprehensive analysis
    info!("Running comprehensive analysis...");
    let analysis_result = perform_comprehensive_analysis(&pool, &parsed_tx, &rpc_client).await?;

    // Print the detailed analysis results
    println!("\n=== TRANSACTION ANALYSIS RESULTS ===\n");
    println!("{}", analysis_result);

    // Print additional details about heuristics and risk scoring
    if let Some(heuristic_flags) = &analysis_result.heuristic_flags {
        println!("\n=== DETAILED HEURISTIC FLAGS ===\n");
        println!(
            "Overall suspicion score: {:.2}",
            heuristic_flags.get_overall_suspicion_score()
        );
        println!("Is high frequency: {}", heuristic_flags.is_high_frequency);
        println!(
            "Direct illicit interaction: {}",
            heuristic_flags.direct_illicit_interaction
        );
        println!("Is new wallet: {}", heuristic_flags.is_new_wallet);
        println!("Is pass-through: {}", heuristic_flags.is_pass_through);
        println!(
            "Structuring score: {:.2}",
            heuristic_flags.structuring_score
        );
        println!(
            "Risky funding ratio: {:.2}",
            heuristic_flags.risky_funding_source_ratio
        );
        println!(
            "Risky spending ratio: {:.2}",
            heuristic_flags.risky_spending_destination_ratio
        );
    }

    if let Some(risk_score) = &analysis_result.risk_score {
        println!("\n=== RISK SCORING DETAILS ===\n");
        println!("Numerical score: {:.2}", risk_score.numerical_score);
        println!("Risk category: {}", risk_score.category);
        println!("Risk factors:");
        for factor in &risk_score.risk_factors {
            println!("  - {}", factor);
        }
    }

    info!("Analysis test completed successfully");
    Ok(())
}
