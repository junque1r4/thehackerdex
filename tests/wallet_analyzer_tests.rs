use hackerdex::analysis::transaction_parser::ParsedTransaction;
use hackerdex::analysis::wallet_analyzer::{
    analyze_transaction_wallets_direct, categorize_transaction_wallets,
    cross_reference_wallet_addresses,
};
use hackerdex::db::models::AddressData;
use hackerdex::db::repository;
use hackerdex::error::HackerdexResult;
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
    // Add some known wallet addresses for testing
    let test_addresses = vec![
        AddressData {
            address: "SuspiciousWallet111111111111111111111111111".to_string(),
            entity_name: "Known Hacker Wallet".to_string(),
            category: "Malicious Actor".to_string(),
            risk_level: "High".to_string(),
            source_of_info: "OSINT Report-2023-02-15".to_string(),
            confidence_score: 4,
            notes: Some("Associated with multiple fund thefts".to_string()),
        },
        AddressData {
            address: "ExchangeHotWallet22222222222222222222222222".to_string(),
            entity_name: "Major Exchange Hot Wallet".to_string(),
            category: "Exchange".to_string(),
            risk_level: "Low".to_string(),
            source_of_info: "Official Docs".to_string(),
            confidence_score: 5,
            notes: Some("Primary hot wallet for major exchange".to_string()),
        },
        AddressData {
            address: "BridgeContract333333333333333333333333333333".to_string(),
            entity_name: "Cross-Chain Bridge".to_string(),
            category: "Bridge Contract".to_string(),
            risk_level: "Medium".to_string(),
            source_of_info: "Official Documentation".to_string(),
            confidence_score: 5,
            notes: Some("Cross-chain bridge with previous minor vulnerabilities".to_string()),
        },
    ];

    // Add each test address to the database
    for address in test_addresses {
        repository::add_known_address(pool, &address).await?;
    }

    Ok(())
}

// Test case for cross-referencing wallet addresses
#[tokio::test]
async fn test_cross_reference_wallet_addresses() -> HackerdexResult<()> {
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
                "Skipping test_cross_reference_wallet_addresses due to database connection issues"
            );
            return Ok(());
        }
    };

    populate_test_data(&pool).await?;

    // Test wallet addresses: two known, one unknown
    let wallet_addresses = vec![
        "SuspiciousWallet111111111111111111111111111".to_string(),
        "ExchangeHotWallet22222222222222222222222222".to_string(),
        "UnknownWallet44444444444444444444444444444444".to_string(),
    ];

    // Perform cross-reference
    let results = cross_reference_wallet_addresses(&pool, &wallet_addresses).await?;

    // Check that we got back the expected number of results
    assert_eq!(results.len(), 3);

    // Check first result (Suspicious Wallet)
    assert!(results[0].is_known);
    assert_eq!(
        results[0].wallet_address,
        "SuspiciousWallet111111111111111111111111111"
    );
    assert!(results[0].address_record.is_some());
    if let Some(record) = &results[0].address_record {
        assert_eq!(record.entity_name, "Known Hacker Wallet");
        assert_eq!(record.category, "Malicious Actor");
        assert_eq!(record.risk_level, "High");
    }

    // Check second result (Exchange Hot Wallet)
    assert!(results[1].is_known);
    assert_eq!(
        results[1].wallet_address,
        "ExchangeHotWallet22222222222222222222222222"
    );
    assert!(results[1].address_record.is_some());
    if let Some(record) = &results[1].address_record {
        assert_eq!(record.entity_name, "Major Exchange Hot Wallet");
        assert_eq!(record.category, "Exchange");
        assert_eq!(record.risk_level, "Low");
    }

    // Check third result (Unknown Wallet)
    assert!(!results[2].is_known);
    assert_eq!(
        results[2].wallet_address,
        "UnknownWallet44444444444444444444444444444444"
    );
    assert!(results[2].address_record.is_none());

    Ok(())
}

// Test case for creating a parsed transaction and analyzing its wallets
#[tokio::test]
async fn test_analyze_transaction_with_known_wallets() -> HackerdexResult<()> {
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
                "Skipping test_analyze_transaction_with_known_wallets due to database connection issues"
            );
            return Ok(());
        }
    };

    populate_test_data(&pool).await?;

    // Create a sample parsed transaction with known and unknown wallets
    let parsed_tx = ParsedTransaction {
        signature: "test_signature".to_string(),
        program_ids: vec![
            "11111111111111111111111111111111".to_string(), // System Program
        ],
        involved_accounts: vec![
            "SuspiciousWallet111111111111111111111111111".to_string(),
            "ExchangeHotWallet22222222222222222222222222".to_string(),
            "UnknownWallet44444444444444444444444444444444".to_string(),
        ],
        pre_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        post_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        execution_status: Some("confirmed".to_string()),
        fee: 5000,
    };

    // Analyze the transaction
    let results = analyze_transaction_wallets_direct(&pool, &parsed_tx).await?;

    // Check that we got back the expected number of results
    assert_eq!(results.len(), 3);

    // Check for Suspicious Wallet
    let suspicious_result = results
        .iter()
        .find(|r| r.wallet_address == "SuspiciousWallet111111111111111111111111111");
    assert!(suspicious_result.is_some());
    let suspicious_result = suspicious_result.unwrap();
    assert!(suspicious_result.is_known);
    assert!(suspicious_result.address_record.is_some());
    let record = suspicious_result.address_record.as_ref().unwrap();
    assert_eq!(record.entity_name, "Known Hacker Wallet");
    assert_eq!(record.risk_level, "High");

    // Check for Exchange Hot Wallet
    let exchange_result = results
        .iter()
        .find(|r| r.wallet_address == "ExchangeHotWallet22222222222222222222222222");
    assert!(exchange_result.is_some());
    let exchange_result = exchange_result.unwrap();
    assert!(exchange_result.is_known);
    assert!(exchange_result.address_record.is_some());

    // Check for Unknown Wallet
    let unknown_result = results
        .iter()
        .find(|r| r.wallet_address == "UnknownWallet44444444444444444444444444444444");
    assert!(unknown_result.is_some());
    let unknown_result = unknown_result.unwrap();
    assert!(!unknown_result.is_known);
    assert!(unknown_result.address_record.is_none());

    // Test categorize_transaction_wallets
    let summary = categorize_transaction_wallets(&pool, &parsed_tx).await?;

    // Check that the summary contains the high risk warning
    assert!(summary.contains("⚠️ HIGH RISK WALLETS DETECTED ⚠️"));
    assert!(summary.contains("Known Hacker Wallet"));
    assert!(summary.contains("Malicious Actor"));

    Ok(())
}
