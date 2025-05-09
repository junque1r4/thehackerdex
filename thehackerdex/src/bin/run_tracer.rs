use clap::{Arg, Command};
use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;

use thehackerdex::db::repository::Repository;
use thehackerdex::discovery::tracer::{FundTracer, FundTracerConfig};
use thehackerdex::error::{HackerdexError, HackerdexResult};
use thehackerdex::rpc::RateLimitedClient;

#[tokio::main]
async fn main() -> HackerdexResult<()> {
    dotenv().ok();

    // Parse command line arguments using clap
    let matches = Command::new("HackerDex Fund Tracer")
        .about("Trace cryptocurrency funds flowing between addresses")
        .arg(Arg::new("mode")
            .value_parser(["forward-track", "backward-track"])
            .required(true)
            .index(1)
            .help("Tracing mode: forward-track (from source to destination) or backward-track (from destination to source)"))
        .arg(Arg::new("source")
            .long("source")
            .value_name("ADDRESS")
            .help("Specific address to trace from (overrides automatic high-risk address selection)"))
        .arg(Arg::new("hops")
            .long("hops")
            .value_name("COUNT")
            .help("Maximum number of hops to trace (default: 2)"))
        .get_matches();

    // Get the tracing mode
    let mode = matches.get_one::<String>("mode").unwrap(); // Safe unwrap due to required arg

    // Get optional parameters
    let specified_address = matches.get_one::<String>("source").map(|s| s.as_str());
    let max_hops = matches
        .get_one::<String>("hops")
        .and_then(|h| h.parse::<i32>().ok())
        .unwrap_or(2);

    println!(
        "Starting Fund Tracer - {} Mode",
        if mode == "forward-track" {
            "Forward Tracking"
        } else {
            "Backward Tracking"
        }
    );

    // Use default configuration values
    let config = FundTracerConfig {
        high_risk_categories: vec!["Known Hacker".to_string(), "Sanctioned Entity".to_string()],
        high_risk_levels: vec!["High".to_string(), "Critical".to_string()],
        max_hop_count: 3,
        max_batch_size: 10,
    };

    // Setup database connection
    let database_url =
        env::var("DATABASE_URL").expect("DATABASE_URL must be set in environment or .env file");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to create database connection pool");

    // Create repository
    let repo = Repository::new(pool);

    // Setup RPC client
    let rpc_url =
        env::var("SOLANA_RPC_URL").expect("SOLANA_RPC_URL must be set in environment or .env file");
    let rpc = RateLimitedClient::new(Some(rpc_url));

    // Create fund tracer
    let tracer = FundTracer::new(repo, rpc, config);

    // Get high risk addresses for display
    println!("🔍 Identifying high-risk addresses based on criteria...");
    match tracer.get_high_risk_addresses().await {
        Ok(addresses) => {
            println!("Found {} high-risk addresses", addresses.len());

            // Display first few addresses
            let display_count = std::cmp::min(5, addresses.len());
            for (i, address) in addresses.iter().take(display_count).enumerate() {
                println!(
                    "  {}. {} ({}): {} risk - {}",
                    i + 1,
                    address.address,
                    address.entity_name,
                    address.risk_level,
                    address.category
                );
            }

            // Determine which address to trace
            let trace_address = match specified_address {
                Some(addr) => {
                    // Use the specified address from command line
                    addr.to_string()
                }
                None => {
                    if addresses.is_empty() {
                        return Err(HackerdexError::NotFound(
                            "No high-risk addresses found".to_string(),
                        ));
                    }
                    // Use the first high-risk address if none is specified
                    addresses[0].address.clone()
                }
            };

            // Perform the tracing operation based on the selected mode
            match mode.as_str() {
                "forward-track" => {
                    println!("\n🔍 Running forward trace from address: {}", trace_address);
                    match tracer
                        .trace_funds_forward(&trace_address, Some(max_hops))
                        .await
                    {
                        Ok(links) => {
                            println!("Discovered {} links from {}", links.len(), trace_address);
                            for link in links {
                                println!(
                                    "  Hop {}: {} -> {}",
                                    link.hop_count, link.source_address, link.target_address
                                );
                            }
                        }
                        Err(e) => println!("Error tracing funds: {}", e),
                    }
                }
                "backward-track" => {
                    println!("\n🔍 Running backward trace to address: {}", trace_address);
                    match tracer
                        .trace_funds_backward(&trace_address, Some(max_hops))
                        .await
                    {
                        Ok(links) => {
                            println!("Discovered {} links to {}", links.len(), trace_address);
                            for link in links {
                                println!(
                                    "  Hop {}: {} -> {}",
                                    link.hop_count, link.source_address, link.target_address
                                );
                            }
                        }
                        Err(e) => println!("Error tracing funds: {}", e),
                    }
                }
                _ => unreachable!(), // Clap ensures we only get valid modes
            }
        }
        Err(e) => println!("Error getting high-risk addresses: {}", e),
    }

    Ok(())
}
