use thehackerdex::db::{AddressData, add_known_address, initialize_db};
use sqlx::PgPool;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};

/// Script to import hacked wallets from a CSV file with address,reason format
/// Usage: cargo run --bin import_hacked_wallets -- <input_csv_path>
///
/// Input format: address,reason
/// Example:
/// ```
/// DefcyKc4yAjRsCLZjdxWuSUzVohXtLna9g22y3pBCm2z, credit to @dethective on Twitter
/// ```
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: cargo run --bin import_hacked_wallets -- <input_csv_path>");
        return Ok(());
    }

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");
    println!("Connecting to database: {}", database_url);

    let pool = PgPool::connect(&database_url).await?;

    initialize_db(&pool).await?;

    let file_path = &args[1];
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    println!("Importing hacked wallets from {}", file_path);
    let mut count = 0;

    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        // Split the address,reason format
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 1 {
            println!("Skipping invalid line: {}", line);
            continue;
        }

        let address = parts[0].trim();
        let reason = if parts.len() > 1 {
            parts[1..].join(",").trim().to_string()
        } else {
            "No reason provided".to_string()
        };

        let entity_name = format!("Malicious_Wallet_{}", index + 1);

        let address_data = AddressData {
            address: address.to_string(),
            entity_name,
            category: "Known Hacker".to_string(),
            risk_level: "High".to_string(),
            source_of_info: "Twitter OSINT".to_string(),
            confidence_score: 4,
            notes: Some(reason),
        };

        match add_known_address(&pool, &address_data).await {
            Ok(_) => {
                count += 1;
                println!(
                    "Added: {} ({})",
                    address_data.entity_name, address_data.address
                );
            }
            Err(e) => {
                println!("Error adding {}: {}", address_data.address, e);
            }
        }
    }

    println!("Import completed. Added {} addresses.", count);
    Ok(())
}
