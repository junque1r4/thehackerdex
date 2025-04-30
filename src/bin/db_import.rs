use hackerdex::db::{AddressData, add_known_address, initialize_db};
use sqlx::PgPool;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufRead, BufReader};

/// Command-line utility for importing known addresses into the database
/// Usage: cargo run --bin db_import -- <csv_file_path>
///
/// CSV format: address,entity_name,category,risk_level,source_of_info,confidence_score,notes
/// Example:
/// ```
/// SoLWormhoLe1111111111111111111111111111111,Wormhole,Bridge Contract,Low,Official Docs,5,Official Wormhole bridge contract
/// ```
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: cargo run --bin db_import -- <csv_file_path>");
        return Ok(());
    }

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");
    println!("Connecting to database: {}", database_url);

    let pool = PgPool::connect(&database_url).await?;

    initialize_db(&pool).await?;

    let file_path = &args[1];
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    println!("Importing addresses from {}", file_path);
    let mut count = 0;

    for line in reader.lines() {
        let line = line?;
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 6 {
            println!("Skipping invalid line: {}", line);
            continue;
        }

        let address_data = AddressData {
            address: parts[0].trim().to_string(),
            entity_name: parts[1].trim().to_string(),
            category: parts[2].trim().to_string(),
            risk_level: parts[3].trim().to_string(),
            source_of_info: parts[4].trim().to_string(),
            confidence_score: parts[5].trim().parse::<i32>().unwrap_or(3),
            notes: if parts.len() > 6 {
                Some(parts[6].trim().to_string())
            } else {
                None
            },
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

async fn read_from_stdin(pool: &PgPool) -> Result<(), Box<dyn Error>> {
    println!(
        "Enter addresses in CSV format (address,entity_name,category,risk_level,source,confidence_score,notes)"
    );
    println!("Type 'exit' to quit");

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    while let Some(line) = lines.next() {
        let line = line?;
        if line.trim() == "exit" {
            break;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 6 {
            println!(
                "Invalid format. Expected: address,entity_name,category,risk_level,source,confidence_score,notes"
            );
            continue;
        }

        let address_data = AddressData {
            address: parts[0].trim().to_string(),
            entity_name: parts[1].trim().to_string(),
            category: parts[2].trim().to_string(),
            risk_level: parts[3].trim().to_string(),
            source_of_info: parts[4].trim().to_string(),
            confidence_score: parts[5].trim().parse::<i32>().unwrap_or(3),
            notes: if parts.len() > 6 {
                Some(parts[6].trim().to_string())
            } else {
                None
            },
        };

        match add_known_address(&pool, &address_data).await {
            Ok(_) => {
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

    Ok(())
}
