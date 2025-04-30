use hackerdex::{
    HackerdexError,
    config::Config,
    db::repository::get_address_details,
    osint::{MaliciousWalletReport, add_wallet_to_database, lookup_wallet_address},
};
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process;

const RESPONSE_DIR: &str = "api_responses";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get wallet address from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: {} <solana_wallet_address> [--query] [--save]",
            args[0]
        );
        eprintln!("  --query: Query the ChainAbuse API (uses 1 of 10 monthly API calls)");
        eprintln!("  --save:  Save the wallet to the database if found");
        process::exit(1);
    }

    let wallet_address = &args[1];
    let query_api = args.iter().any(|arg| arg == "--query");
    let save_to_db = args.iter().any(|arg| arg == "--save");

    // Load configuration
    let config = Config::from_env();

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

    println!("Looking up wallet address: {}", wallet_address);

    // Try the database first
    let db_result = get_address_details(&pool, wallet_address).await;
    match db_result {
        Ok(record) => {
            println!("═══════════════════════════════════════════════════");
            println!("WALLET FOUND IN DATABASE");
            println!("═══════════════════════════════════════════════════");
            println!("Address: {}", record.address);
            println!("Entity Name: {}", record.entity_name);
            println!("Category: {}", record.category);
            println!("Risk Level: {}", record.risk_level);
            println!("Source: {}", record.source_of_info);
            println!("Confidence Score: {}", record.confidence_score);
            if let Some(notes) = record.notes {
                println!("Notes: {}", notes);
            }
            println!("Added: {}", record.created_at);
            println!("Last Updated: {}", record.updated_at);
        }
        Err(_) => {
            println!("Wallet not found in local database.");

            // If not found and --query is specified, check ChainAbuse API
            if query_api {
                check_chainabuse_api(&config, &pool, wallet_address, save_to_db).await?;
            } else {
                println!(
                    "\nTip: Use --query to check ChainAbuse API (will use 1 of 10 monthly API calls)"
                );
            }
        }
    }

    Ok(())
}

async fn check_chainabuse_api(
    config: &Config,
    pool: &PgPool,
    wallet_address: &str,
    save_to_db: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Print warning about API usage
    println!("\nWarning: This will use one of your limited (10/month) ChainAbuse API calls!");
    println!("Press Ctrl+C within 5 seconds to abort...");

    // 5 second delay to allow cancellation
    for i in (1..=5).rev() {
        print!("\rContinuing in {} seconds...", i);
        std::io::stdout().flush().unwrap();
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
    println!("\n");

    // Ensure response directory exists
    let _ = fs::create_dir_all(RESPONSE_DIR);

    // Lookup the wallet address
    match lookup_wallet_address(config, pool, wallet_address).await {
        Ok(Some(report)) => {
            print_report(&report);

            // Save to database if requested
            if save_to_db {
                println!("\nSaving wallet to database...");
                match add_wallet_to_database(pool, &report).await {
                    Ok(_) => {
                        if report.in_database {
                            println!("Wallet already exists in database.");
                        } else {
                            println!("Successfully added wallet to database.");
                        }
                    }
                    Err(e) => {
                        eprintln!("Error saving wallet to database: {}", e);
                        process::exit(1);
                    }
                }
            }
        }
        Ok(None) => {
            println!("No malicious activity reports found for this address on ChainAbuse.");
        }
        Err(HackerdexError::ConfigError(msg)) => {
            eprintln!("Configuration error: {}", msg);
            eprintln!("Make sure CHAINABUSE_API is set in your .env file.");
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Error looking up wallet address on ChainAbuse: {}", e);

            // Notify user about saved response
            if Path::new(&format!(
                "{}/chainabuse_error_{}.json",
                RESPONSE_DIR, wallet_address
            ))
            .exists()
            {
                println!(
                    "\nAPI response was saved to {}/chainabuse_error_{}.json",
                    RESPONSE_DIR, wallet_address
                );
                println!("This allows analysis of the error without using another API call.");
            }

            process::exit(1);
        }
    }

    Ok(())
}

fn print_report(report: &MaliciousWalletReport) {
    println!("═══════════════════════════════════════════════════");
    println!("🚨 MALICIOUS ACTIVITY DETECTED 🚨");
    println!("═══════════════════════════════════════════════════");
    println!("Address: {}", report.address);
    println!("Categories: {}", report.categories.join(", "));
    println!("Risk Level: {}", report.risk_level);
    println!("Reports: {}", report.report_count);
    println!(
        "Already in Database: {}",
        if report.in_database { "Yes" } else { "No" }
    );

    println!("\nDETAILED REPORTS:");
    println!("───────────────────────────────────────────────────");

    for (i, detail) in report.details.iter().enumerate() {
        println!("Report #{}", i + 1);
        println!("ID: {}", detail.id);
        println!("Created: {}", detail.created_at);
        println!("Category: {}", detail.category);
        println!("Trusted Source: {}", detail.trusted);
        if let Some(desc) = &detail.description {
            println!("Description: {}", desc);
        }
        println!();
    }
}
