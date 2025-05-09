use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};

/// Usage: cargo run --bin convert_csv -- <input_csv_path> <output_csv_path>
/// This script is being used just to convert the public Meteora csv from the Kelsir Ventures rug pullers.
/// Source: https://github.com/MeteoraAg/ops?tab=readme-ov-file
/// Input format: address,reason
/// Output format: address,entity_name,category,risk_level,source_of_info,confidence_score,notes
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Usage: cargo run --bin convert_csv -- <input_csv_path> <output_csv_path>");
        return Ok(());
    }

    let input_path = &args[1];
    let output_path = &args[2];

    let input_file = File::open(input_path)?;
    let reader = BufReader::new(input_file);
    let mut output_file = File::create(output_path)?;

    println!("Converting {} to {}...", input_path, output_path);

    // Add header if needed
    // writeln!(output_file, "address,entity_name,category,risk_level,source_of_info,confidence_score,notes")?;

    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 2 {
            println!("Skipping invalid line: {}", line);
            continue;
        }

        let address = parts[0].trim();
        let reason = parts[1..].join(",").trim().to_string(); // bug: Handle cases where the reason itself might contain commas

        let sanitized_reason = reason.replace(',', ";");

        let formatted_line = format!(
            "{},Malicious_Wallet_{},Known Hacker,High,Twitter OSINT,4,{}",
            address,
            index + 1, // Identity id
            sanitized_reason
        );

        writeln!(output_file, "{}", formatted_line)?;
    }

    println!("Conversion completed successfully.");
    Ok(())
}
