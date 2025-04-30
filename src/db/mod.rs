pub mod models;
pub mod repository;

pub use models::AddressData;
pub use models::AddressRecord;
pub use models::SerializableAddressRecord;
pub use repository::{
    Repository, add_known_address, get_address_details, get_all_addresses_by_category,
    initialize_db, update_address_details,
};

#[cfg(test)]
mod tests {
    #[allow(unused)]
    use super::*;
    use sqlx::PgPool;
    use std::env;

    #[allow(unused)]
    async fn setup_test_db() -> anyhow::Result<PgPool> {
        dotenv::dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

        println!("Connecting to test database: {}", database_url);

        let pool = PgPool::connect(&database_url).await?;

        // Clear test data if needed
        // sqlx::query!("DELETE FROM known_addresses").execute(&pool).await?;

        Ok(pool)
    }

    // These tests are commented out since they require a running database
    // Uncomment them when you want to run actual database tests
    /*
    #[tokio::test]
    async fn test_db_operations() -> anyhow::Result<()> {
        let pool = setup_test_db().await?;
        initialize_db(&pool).await?;

        // Test adding an address
        let address_data = AddressData {
            address: "SoLWormhoLe1111111111111111111111111111111".to_string(),
            entity_name: "Wormhole".to_string(),
            category: "Bridge Contract".to_string(),
            risk_level: "Low".to_string(),
            source_of_info: "Official Docs".to_string(),
            confidence_score: 5,
            notes: Some("Official Wormhole bridge contract".to_string()),
        };

        add_known_address(&pool, &address_data).await?;

        // Test retrieving address details
        let details = get_address_details(&pool, &address_data.address).await?;
        assert_eq!(details.entity_name, "Wormhole");
        assert_eq!(details.category, "Bridge Contract");

        // Test updating address details
        let updates = AddressData {
            address: address_data.address.clone(),
            entity_name: "Wormhole Bridge".to_string(),
            category: "Bridge Contract".to_string(),
            risk_level: "Low".to_string(),
            source_of_info: "Official Docs".to_string(),
            confidence_score: 5,
            notes: Some("Updated notes".to_string()),
        };

        update_address_details(&pool, &updates).await?;

        // Test retrieving by category
        let addresses = get_all_addresses_by_category(&pool, "Bridge Contract").await?;
        assert_eq!(addresses.len(), 1);
        assert_eq!(addresses[0].entity_name, "Wormhole Bridge");

        Ok(())
    }
    */
}
