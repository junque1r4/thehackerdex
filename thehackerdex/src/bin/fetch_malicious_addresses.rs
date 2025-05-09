use thehackerdex::{HackerdexError, config::Config, osint::fetch_malicious_solana_addresses};
use sqlx::postgres::PgPoolOptions;
use std::process;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = Config::from_env();

    // Print warning about API usage
    println!("Warning: This tool will use one of your limited (10/month) ChainAbuse API calls!");
    println!("Press Ctrl+C within 5 seconds to abort...");

    // 5 second delay to allow cancellation
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Connect to the database
    let pool = match PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("Error connecting to database: {}", e);
            process::exit(1);
        }
    };

    println!("Fetching malicious Solana addresses from ChainAbuse...");

    // Fetch and store malicious addresses
    match fetch_malicious_solana_addresses(&config, &pool).await {
        Ok(count) => {
            println!(
                "Successfully added {} new malicious addresses to the database.",
                count
            );
            println!(
                "Addresses are marked with category 'Known Hacker', 'Money Laundering', or similar"
            );
            println!("and will be used to detect potential exfiltration routes in the analysis.");
        }
        Err(HackerdexError::ConfigError(msg)) => {
            eprintln!("Configuration error: {}", msg);
            eprintln!("Make sure CHAINABUSE_API is set in your .env file.");
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Error fetching malicious addresses: {}", e);
            process::exit(1);
        }
    }

    Ok(())
}
