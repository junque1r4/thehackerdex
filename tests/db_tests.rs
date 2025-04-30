use hackerdex::db::AddressData;
use hackerdex::db::{
    add_known_address, get_address_details, get_all_addresses_by_category, initialize_db,
    update_address_details,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::env;

async fn setup_test_db() -> anyhow::Result<PgPool> {
    // Load environment variables
    dotenv::dotenv().ok();

    // Get database URL from environment or use a default test URL
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5432/hackerdex_test".to_string()
    });

    println!("Connecting to test database: {}", database_url);

    // Create connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Clear test data for clean tests
    clean_test_data(&pool).await?;

    Ok(pool)
}

async fn clean_test_data(pool: &PgPool) -> anyhow::Result<()> {
    // Delete all test data to ensure tests start with a clean slate
    sqlx::query!("DELETE FROM address_interactions")
        .execute(pool)
        .await?;
    sqlx::query!("DELETE FROM known_addresses")
        .execute(pool)
        .await?;

    Ok(())
}

/// Helper function to create test address data
fn create_test_address(address: &str, entity: &str, category: &str, risk: &str) -> AddressData {
    AddressData {
        address: address.to_string(),
        entity_name: entity.to_string(),
        category: category.to_string(),
        risk_level: risk.to_string(),
        source_of_info: "Test Source".to_string(),
        confidence_score: 3,
        notes: Some("Test address for unit tests".to_string()),
    }
}

/// Test suite that requires a database connection
/// Use #[ignore] to prevent this from running in normal test runs
/// Run with `cargo test -- --ignored` when you want to test against a real database
#[cfg(test)]
mod database_tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_initialize_db() -> anyhow::Result<()> {
        // Test that the database can be initialized
        let pool = setup_test_db().await?;
        let result = initialize_db(&pool).await;

        assert!(
            result.is_ok(),
            "Database initialization failed: {:?}",
            result
        );

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_add_and_get_known_address() -> anyhow::Result<()> {
        let pool = setup_test_db().await?;

        // Test data
        let test_address = create_test_address(
            "AddTestAddress111111111111111111111111111111",
            "Test Entity",
            "Test Category",
            "Low",
        );

        // Test add_known_address
        add_known_address(&pool, &test_address).await?;

        // Test get_address_details
        let fetched = get_address_details(&pool, &test_address.address).await?;

        // Verify the data was correctly stored and retrieved
        assert_eq!(fetched.address, test_address.address);
        assert_eq!(fetched.entity_name, "Test Entity");
        assert_eq!(fetched.category, "Test Category");
        assert_eq!(fetched.risk_level, "Low");
        assert_eq!(fetched.source_of_info, "Test Source");
        assert_eq!(fetched.confidence_score, 3);
        assert_eq!(
            fetched.notes,
            Some("Test address for unit tests".to_string())
        );

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_update_address_details() -> anyhow::Result<()> {
        let pool = setup_test_db().await?;

        // Create initial address data
        let initial_address = create_test_address(
            "UpdateTestAddr11111111111111111111111111111",
            "Original Entity",
            "Original Category",
            "Low",
        );

        // Add the initial address
        add_known_address(&pool, &initial_address).await?;

        // Create update data
        let updated_address = AddressData {
            address: initial_address.address.clone(),
            entity_name: "Updated Entity".to_string(),
            category: "Updated Category".to_string(),
            risk_level: "High".to_string(),
            source_of_info: "Updated Source".to_string(),
            confidence_score: 5,
            notes: Some("Updated notes for testing".to_string()),
        };

        // Test update_address_details
        update_address_details(&pool, &updated_address).await?;

        // Fetch and verify the updated data
        let fetched = get_address_details(&pool, &initial_address.address).await?;

        assert_eq!(fetched.address, initial_address.address);
        assert_eq!(fetched.entity_name, "Updated Entity");
        assert_eq!(fetched.category, "Updated Category");
        assert_eq!(fetched.risk_level, "High");
        assert_eq!(fetched.source_of_info, "Updated Source");
        assert_eq!(fetched.confidence_score, 5);
        assert_eq!(fetched.notes, Some("Updated notes for testing".to_string()));

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_all_addresses_by_category() -> anyhow::Result<()> {
        let pool = setup_test_db().await?;

        // Create multiple test addresses in the same category
        let addresses = [
            create_test_address(
                "CategoryTest111111111111111111111111111111",
                "Entity 1",
                "Test Bridge",
                "Medium",
            ),
            create_test_address(
                "CategoryTest222222222222222222222222222222",
                "Entity 2",
                "Test Bridge",
                "Low",
            ),
            create_test_address(
                "CategoryTest333333333333333333333333333333",
                "Entity 3",
                "Test DEX",
                "Low",
            ),
        ];

        // Add all test addresses to the database
        for address in &addresses {
            add_known_address(&pool, address).await?;
        }

        // Test getting addresses by category
        let bridge_addresses = get_all_addresses_by_category(&pool, "Test Bridge").await?;
        let dex_addresses = get_all_addresses_by_category(&pool, "Test DEX").await?;

        // Verify the correct counts
        assert_eq!(
            bridge_addresses.len(),
            2,
            "Should find 2 Test Bridge addresses"
        );
        assert_eq!(dex_addresses.len(), 1, "Should find 1 Test DEX address");

        // Verify the addresses in the Bridge category
        let bridge_addr_set: std::collections::HashSet<String> = bridge_addresses
            .iter()
            .map(|record| record.address.clone())
            .collect();

        assert!(bridge_addr_set.contains(&addresses[0].address));
        assert!(bridge_addr_set.contains(&addresses[1].address));
        assert!(!bridge_addr_set.contains(&addresses[2].address));

        // Verify the addresses in the DEX category
        let dex_addr_set: std::collections::HashSet<String> = dex_addresses
            .iter()
            .map(|record| record.address.clone())
            .collect();

        assert!(dex_addr_set.contains(&addresses[2].address));
        assert!(!dex_addr_set.contains(&addresses[0].address));
        assert!(!dex_addr_set.contains(&addresses[1].address));

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_invalid_data() -> anyhow::Result<()> {
        let pool = setup_test_db().await?;

        // Test with invalid confidence score (too high)
        let invalid_score = AddressData {
            address: "InvalidScore1111111111111111111111111111".to_string(),
            entity_name: "Invalid Score Entity".to_string(),
            category: "Test Category".to_string(),
            risk_level: "Low".to_string(),
            source_of_info: "Test".to_string(),
            confidence_score: 10, // Outside valid range (1-5)
            notes: None,
        };

        // This should fail due to the CHECK constraint in the database
        let result = add_known_address(&pool, &invalid_score).await;
        assert!(
            result.is_err(),
            "Expected error for invalid confidence score"
        );
        assert!(
            result.unwrap_err().to_string().contains("check"),
            "Error should be related to constraint violation"
        );

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_duplicate_address() -> anyhow::Result<()> {
        let pool = setup_test_db().await?;

        // Create test address
        let test_address = create_test_address(
            "DuplicateTest111111111111111111111111111111",
            "First Entity",
            "Test Category",
            "Low",
        );

        // Add it once
        add_known_address(&pool, &test_address).await?;

        // Create second address with same address but different entity
        let duplicate = AddressData {
            address: test_address.address.clone(),
            entity_name: "Second Entity".to_string(),
            category: "Another Category".to_string(),
            risk_level: "High".to_string(),
            source_of_info: "Another Source".to_string(),
            confidence_score: 4,
            notes: None,
        };

        // Attempt to add the duplicate - should fail due to primary key constraint
        let result = add_known_address(&pool, &duplicate).await;

        assert!(result.is_err(), "Expected error for duplicate address");
        assert!(
            result.unwrap_err().to_string().contains("duplicate key"),
            "Error should be related to duplicate key violation"
        );

        Ok(())
    }
}

/// Mock tests that don't require a database connection
/// Note: These tests are implemented separately in db_mock_tests.rs
#[cfg(test)]
mod mock_database_tests {}

// Integration tests for the database repository module
#[cfg(test)]
mod integration_tests {
    use super::*;

    // Test the full workflow from adding to querying by category
    #[tokio::test]
    #[ignore]
    async fn test_full_address_workflow() -> anyhow::Result<()> {
        let pool = setup_test_db().await?;

        // 1. Add a few addresses in different categories
        let addresses = [
            // Bridge contracts (low risk)
            create_test_address(
                "WorkflowBridge111111111111111111111111111",
                "Test Wormhole",
                "Bridge Contract",
                "Low",
            ),
            create_test_address(
                "WorkflowBridge222222222222222222222222222",
                "Test Portal",
                "Bridge Contract",
                "Low",
            ),
            // DEX addresses (low risk)
            create_test_address(
                "WorkflowDEX11111111111111111111111111111",
                "Test Jupiter",
                "DEX Router",
                "Low",
            ),
            // Known hackers (high risk)
            create_test_address(
                "WorkflowHacker111111111111111111111111111",
                "Test Hacker A",
                "Known Hacker",
                "Critical",
            ),
            create_test_address(
                "WorkflowHacker222222222222222222222222222",
                "Test Hacker B",
                "Known Hacker",
                "Critical",
            ),
        ];

        // Add all addresses
        for address in &addresses {
            add_known_address(&pool, address).await?;
        }

        // 2. Update one address
        let updated_address = AddressData {
            address: addresses[0].address.clone(),
            entity_name: "Updated Wormhole".to_string(),
            category: "Bridge Contract".to_string(),
            risk_level: "Medium".to_string(), // Changed from Low to Medium
            source_of_info: "Updated Source".to_string(),
            confidence_score: 4, // Changed from 3 to 4
            notes: Some("Updated bridge with potential vulnerabilities".to_string()),
        };

        update_address_details(&pool, &updated_address).await?;

        // 3. Get addresses by category
        let bridge_addresses = get_all_addresses_by_category(&pool, "Bridge Contract").await?;
        let hacker_addresses = get_all_addresses_by_category(&pool, "Known Hacker").await?;

        // Verify counts
        assert_eq!(bridge_addresses.len(), 2);
        assert_eq!(hacker_addresses.len(), 2);

        // 4. Check that the update was applied
        let updated = get_address_details(&pool, &addresses[0].address).await?;
        assert_eq!(updated.entity_name, "Updated Wormhole");
        assert_eq!(updated.risk_level, "Medium");
        assert_eq!(updated.confidence_score, 4);

        // 5. Verify no results for non-existent category
        let non_existent = get_all_addresses_by_category(&pool, "Non Existent Category").await?;
        assert_eq!(non_existent.len(), 0);

        Ok(())
    }
}
