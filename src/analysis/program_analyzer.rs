use crate::analysis::transaction_parser::ParsedTransaction;
use crate::db::{models::AddressRecord, repository};
use crate::error::HackerdexResult;
use sqlx::PgPool;
use tracing::{debug, info};

#[cfg(test)]
mod tests {
    use crate::analysis::transaction_parser::ParsedTransaction;
    use crate::db::models::AddressData;
    use crate::db::repository;
    use crate::error::HackerdexResult;
    use solana_transaction_status::UiTransactionTokenBalance;
    use sqlx::PgPool;
    use sqlx::postgres::PgPoolOptions;
    use std::env;

    // Helper function to create a test database connection
    async fn setup_test_db() -> HackerdexResult<Option<PgPool>> {
        // If SKIP_DB_TESTS environment variable is set, skip these tests
        if env::var("SKIP_DB_TESTS").is_ok() {
            return Ok(None);
        }

        // Get connection string from environment or use default test DB
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@localhost:5432/hackerdex_test".to_string()
        });

        // Connect to the database
        match PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
        {
            Ok(pool) => {
                // Clear the test database to ensure clean state
                match sqlx::query!("DELETE FROM known_addresses")
                    .execute(&pool)
                    .await
                {
                    Ok(_) => Ok(Some(pool)),
                    Err(e) => {
                        println!("Skipping DB tests - failed to clear test database: {}", e);
                        Ok(None)
                    }
                }
            }
            Err(e) => {
                println!("Skipping DB tests - failed to connect to database: {}", e);
                Ok(None)
            }
        }
    }

    // Helper function to populate test data
    async fn populate_test_data(pool: &PgPool) -> HackerdexResult<()> {
        // Add some known program addresses for testing
        let test_addresses = vec![
            AddressData {
                address: "11111111111111111111111111111111".to_string(),
                entity_name: "System Program".to_string(),
                category: "Core Program".to_string(),
                risk_level: "Low".to_string(),
                source_of_info: "Official Docs".to_string(),
                confidence_score: 5,
                notes: Some("Solana System Program".to_string()),
            },
            AddressData {
                address: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
                entity_name: "SPL Token".to_string(),
                category: "Token Program".to_string(),
                risk_level: "Low".to_string(),
                source_of_info: "Official Docs".to_string(),
                confidence_score: 5,
                notes: Some("Solana Program Library Token Program".to_string()),
            },
            AddressData {
                address: "MaliciousPr0gramxxxxxxxxxxxxxxxxxxxxx111111".to_string(),
                entity_name: "Known Hacker Program".to_string(),
                category: "Malicious Contract".to_string(),
                risk_level: "High".to_string(),
                source_of_info: "OSINT Report-2023-01-01".to_string(),
                confidence_score: 4,
                notes: Some("Associated with multiple fund thefts".to_string()),
            },
        ];

        // Add each test address to the database
        for address in test_addresses {
            repository::add_known_address(pool, &address).await?;
        }

        Ok(())
    }

    // Test case for cross-referencing program IDs
    #[tokio::test]
    async fn test_cross_reference_program_ids() -> HackerdexResult<()> {
        // Skip if SKIP_DB_TESTS environment variable is set
        if env::var("SKIP_DB_TESTS").is_ok() {
            return Ok(());
        }

        let pool_result = setup_test_db().await?;

        // Skip test if we couldn't connect to the database
        let pool = match pool_result {
            Some(pool) => pool,
            None => {
                println!(
                    "Skipping test_cross_reference_program_ids due to database connection issues"
                );
                return Ok(());
            }
        };

        populate_test_data(&pool).await?;

        // Test program IDs: two known, one unknown
        let program_ids = vec![
            "11111111111111111111111111111111".to_string(),
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
            "UnknownProgramxxxxxxxxxxxxxxxxxxxxxxxxx11111".to_string(),
        ];

        // Perform cross-reference
        let results = super::cross_reference_program_ids(&pool, &program_ids).await?;

        // Check that we got back the expected number of results
        assert_eq!(results.len(), 3);

        // Check first result (System Program)
        assert!(results[0].is_known);
        assert_eq!(results[0].program_id, "11111111111111111111111111111111");
        assert!(results[0].address_record.is_some());
        if let Some(record) = &results[0].address_record {
            assert_eq!(record.entity_name, "System Program");
            assert_eq!(record.category, "Core Program");
            assert_eq!(record.risk_level, "Low");
        }

        // Check second result (SPL Token)
        assert!(results[1].is_known);
        assert_eq!(
            results[1].program_id,
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        );
        assert!(results[1].address_record.is_some());
        if let Some(record) = &results[1].address_record {
            assert_eq!(record.entity_name, "SPL Token");
            assert_eq!(record.category, "Token Program");
            assert_eq!(record.risk_level, "Low");
        }

        // Check third result (Unknown Program)
        assert!(!results[2].is_known);
        assert_eq!(
            results[2].program_id,
            "UnknownProgramxxxxxxxxxxxxxxxxxxxxxxxxx11111"
        );
        assert!(results[2].address_record.is_none());

        Ok(())
    }

    // Test case for creating a parsed transaction and analyzing it
    #[tokio::test]
    async fn test_analyze_transaction_with_known_programs() -> HackerdexResult<()> {
        // Skip if SKIP_DB_TESTS environment variable is set
        if env::var("SKIP_DB_TESTS").is_ok() {
            return Ok(());
        }

        let pool_result = setup_test_db().await?;

        // Skip test if we couldn't connect to the database
        let pool = match pool_result {
            Some(pool) => pool,
            None => {
                println!(
                    "Skipping test_analyze_transaction_with_known_programs due to database connection issues"
                );
                return Ok(());
            }
        };

        populate_test_data(&pool).await?;

        // Create a sample parsed transaction with known and unknown programs
        let parsed_tx = ParsedTransaction {
            signature: "test_signature".to_string(),
            program_ids: vec![
                "11111111111111111111111111111111".to_string(),
                "MaliciousPr0gramxxxxxxxxxxxxxxxxxxxxx111111".to_string(),
                "UnknownProgramxxxxxxxxxxxxxxxxxxxxxxxxx11111".to_string(),
            ],
            involved_accounts: vec![
                "SomeUserWallet1111111111111111111111111111111".to_string(),
                "AnotherWallet22222222222222222222222222222222".to_string(),
            ],
            pre_token_balances: Vec::<UiTransactionTokenBalance>::new(),
            post_token_balances: Vec::<UiTransactionTokenBalance>::new(),
            execution_status: Some("confirmed".to_string()),
            fee: 5000,
        };

        // Analyze the transaction
        let results = super::analyze_transaction_programs(&pool, &parsed_tx).await?;

        // Check that we got back the expected number of results
        assert_eq!(results.len(), 3);

        // Check for System Program
        let system_result = results
            .iter()
            .find(|r| r.program_id == "11111111111111111111111111111111");
        assert!(system_result.is_some());
        let system_result = system_result.unwrap();
        assert!(system_result.is_known);
        assert!(system_result.address_record.is_some());

        // Check for Malicious Program
        let malicious_result = results
            .iter()
            .find(|r| r.program_id == "MaliciousPr0gramxxxxxxxxxxxxxxxxxxxxx111111");
        assert!(malicious_result.is_some());
        let malicious_result = malicious_result.unwrap();
        assert!(malicious_result.is_known);
        assert!(malicious_result.address_record.is_some());
        let record = malicious_result.address_record.as_ref().unwrap();
        assert_eq!(record.entity_name, "Known Hacker Program");
        assert_eq!(record.risk_level, "High");

        // Check for Unknown Program
        let unknown_result = results
            .iter()
            .find(|r| r.program_id == "UnknownProgramxxxxxxxxxxxxxxxxxxxxxxxxx11111");
        assert!(unknown_result.is_some());
        let unknown_result = unknown_result.unwrap();
        assert!(!unknown_result.is_known);
        assert!(unknown_result.address_record.is_none());

        // Test categorize_transaction
        let summary = super::categorize_transaction(&pool, &parsed_tx).await?;

        // Check that the summary contains the high risk warning
        assert!(summary.contains("⚠️ HIGH RISK TRANSACTION ⚠️"));
        assert!(summary.contains("Known Hacker Program"));
        assert!(summary.contains("Malicious Contract"));

        Ok(())
    }
}

/// Results of cross-referencing a program ID against the known address database
#[derive(Debug, Clone)]
pub struct ProgramAnalysis {
    /// The program ID that was analyzed
    pub program_id: String,
    /// Whether the program was found in the known address database
    pub is_known: bool,
    /// The full address record if found, or None if not found
    pub address_record: Option<AddressRecord>,
}

/// Cross-references a list of program IDs against the known address database
///
/// # Arguments
///
/// * `pool` - PostgreSQL connection pool
/// * `program_ids` - List of program IDs to cross-reference
///
/// # Returns
///
/// A `HackerdexResult` containing a vector of `ProgramAnalysis` or an error
pub async fn cross_reference_program_ids(
    pool: &PgPool,
    program_ids: &[String],
) -> HackerdexResult<Vec<ProgramAnalysis>> {
    let mut results = Vec::with_capacity(program_ids.len());

    // Process each program ID
    for program_id in program_ids {
        debug!("Cross-referencing program ID: {}", program_id);

        // Query the database for this program ID
        let address_record = repository::get_address_details(pool, program_id).await;

        let is_known = address_record.is_ok();
        let record = address_record.ok();

        // Store the results
        results.push(ProgramAnalysis {
            program_id: program_id.clone(),
            is_known,
            address_record: record,
        });
    }

    Ok(results)
}

/// Analyzes program IDs from a parsed transaction
///
/// # Arguments
///
/// * `pool` - PostgreSQL connection pool
/// * `transaction` - The parsed transaction to analyze
///
/// # Returns
///
/// A `HackerdexResult` containing a vector of `ProgramAnalysis` or an error
pub async fn analyze_transaction_programs(
    pool: &PgPool,
    transaction: &ParsedTransaction,
) -> HackerdexResult<Vec<ProgramAnalysis>> {
    info!(
        "Analyzing programs for transaction: {}",
        transaction.signature
    );
    cross_reference_program_ids(pool, &transaction.program_ids).await
}

/// Analyzes and categorizes a transaction based on its program interactions
///
/// # Arguments
///
/// * `pool` - PostgreSQL connection pool
/// * `transaction` - The parsed transaction to analyze
///
/// # Returns
///
/// A `HackerdexResult` containing a summary of findings as a string
pub async fn categorize_transaction(
    pool: &PgPool,
    transaction: &ParsedTransaction,
) -> HackerdexResult<String> {
    let program_analyses = analyze_transaction_programs(pool, transaction).await?;

    let mut summary = String::new();
    let mut has_high_risk = false;
    let mut risk_details = Vec::new();
    let mut categories = Vec::new();

    for analysis in &program_analyses {
        if let Some(record) = &analysis.address_record {
            categories.push(record.category.clone());

            // Check for high risk programs
            if record.risk_level == "High" || record.risk_level == "Critical" {
                has_high_risk = true;
                risk_details.push(format!(
                    "Program {} ({}) has risk level {} (confidence: {}/5)",
                    record.entity_name, record.address, record.risk_level, record.confidence_score
                ));
            }

            summary.push_str(&format!(
                "Program {} is known as {} ({}), risk level: {}\n",
                analysis.program_id, record.entity_name, record.category, record.risk_level
            ));
        } else {
            summary.push_str(&format!(
                "Program {} is unknown in the database\n",
                analysis.program_id
            ));
        }
    }

    // Add summary header based on findings
    let mut header = String::new();
    if has_high_risk {
        header.push_str("⚠️ HIGH RISK TRANSACTION ⚠️\n");
        for detail in risk_details {
            header.push_str(&format!("- {}\n", detail));
        }
        header.push('\n');
    }

    if !categories.is_empty() {
        header.push_str("Transaction categories: ");
        header.push_str(&categories.join(", "));
        header.push_str("\n\n");
    }

    // Combine header and detailed summary
    Ok(format!("{}{}", header, summary))
}
