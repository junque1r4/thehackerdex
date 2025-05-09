use chrono::Utc;
use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

use thehackerdex::db::models::TraceLink;
use thehackerdex::db::repository::Repository;
use thehackerdex::discovery::tracer::{FundTracer, FundTracerConfig};
use thehackerdex::rpc::RateLimitedClient;

/// Simple scheduled fund trace job
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    // Use hardcoded values instead of CLI args
    let output_dir = PathBuf::from("./reports");
    let max_hops = 2;
    let batch_size = 5;
    let high_risk_categories = vec!["Known Hacker".to_string(), "Sanctioned Entity".to_string()];
    let high_risk_levels = vec!["High".to_string(), "Critical".to_string()];

    // Ensure output directory exists
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir)?;
    }

    println!("Starting scheduled fund trace job at {}", Utc::now());
    println!("Tracing {} hops from high-risk addresses", max_hops);

    // Create tracer config
    let config = FundTracerConfig {
        high_risk_categories,
        high_risk_levels,
        max_hop_count: max_hops,
        max_batch_size: batch_size,
    };

    // Setup database connection
    let database_url =
        env::var("DATABASE_URL").expect("DATABASE_URL must be set in environment or .env file");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to create database connection pool");

    let repo = Repository::new(pool);

    // Setup RPC client
    let rpc_url =
        env::var("SOLANA_RPC_URL").expect("SOLANA_RPC_URL must be set in environment or .env file");
    let rpc = RateLimitedClient::new(Some(rpc_url));

    // Create fund tracer
    let tracer = FundTracer::new(repo, rpc, config);

    // Generate timestamp for report files
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");

    // Run forward tracking from high-risk sources
    println!("Running forward tracking from high-risk sources...");
    let forward_links = tracer
        .find_high_risk_fund_recipients(Some(max_hops))
        .await?;

    // Write forward tracking report
    let forward_report_path = output_dir.join(format!("high_risk_forward_trace_{}.csv", timestamp));
    write_trace_report(&forward_links, &forward_report_path)?;
    println!(
        "Forward tracking report written to {:?}",
        forward_report_path
    );

    // In a full implementation, we would:
    // 1. Find new addresses discovered during tracing
    // 2. Report them for review
    // 3. Optionally add them to the database with default risk assessments
    println!("Remember to review any new addresses discovered during tracing.");
    println!("You can implement automatic reporting and assessment as needed.");

    println!("Fund tracing job completed at {}", Utc::now());
    Ok(())
}

/// Write trace links to a CSV report
fn write_trace_report(links: &[TraceLink], path: &PathBuf) -> io::Result<()> {
    let mut file = File::create(path)?;

    // Write header
    writeln!(
        file,
        "id,trace_initiator,source_address,target_address,relationship_type,hop_count,discovery_timestamp,transaction_signature"
    )?;

    // Write data rows
    for link in links {
        writeln!(
            file,
            "{},{},{},{},{},{},{},{}",
            link.id,
            link.trace_initiator,
            link.source_address,
            link.target_address,
            link.relationship_type,
            link.hop_count,
            link.discovery_timestamp,
            link.transaction_signature.as_deref().unwrap_or("")
        )?;
    }

    Ok(())
}

// Note: Full implementation would include:
// - Function to write addresses to a review file
// - Function to extract new addresses from trace links
// These were removed for simplicity in this implementation
