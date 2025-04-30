use hackerdex::db::AddressData;
use hackerdex::error::HackerdexError;
use std::collections::HashMap;
use time::OffsetDateTime as TimeOffsetDateTime;

// Simple in-memory mock database for testing
struct MockDb {
    addresses: HashMap<String, MockAddressRecord>,
}

// Add Debug derive to fix compiler errors
#[derive(Debug, Clone)]
#[allow(dead_code)] // Allow unused fields for testing purposes
struct MockAddressRecord {
    address: String,
    entity_name: String,
    category: String,
    risk_level: String,
    source_of_info: String,
    confidence_score: i32,
    notes: Option<String>,
    created_at: TimeOffsetDateTime,
    updated_at: TimeOffsetDateTime,
}

impl From<&AddressData> for MockAddressRecord {
    fn from(data: &AddressData) -> Self {
        let now = TimeOffsetDateTime::now_utc();
        Self {
            address: data.address.clone(),
            entity_name: data.entity_name.clone(),
            category: data.category.clone(),
            risk_level: data.risk_level.clone(),
            source_of_info: data.source_of_info.clone(),
            confidence_score: data.confidence_score,
            notes: data.notes.clone(),
            created_at: now,
            updated_at: now,
        }
    }
}

impl MockDb {
    fn new() -> Self {
        Self {
            addresses: HashMap::new(),
        }
    }

    fn add_known_address(&mut self, address_data: &AddressData) -> Result<(), HackerdexError> {
        // Check if already exists
        if self.addresses.contains_key(&address_data.address) {
            return Err(HackerdexError::DatabaseError(
                "Address already exists".to_string(),
            ));
        }

        // Validate confidence score
        if address_data.confidence_score < 1 || address_data.confidence_score > 5 {
            return Err(HackerdexError::DatabaseError(
                "Invalid confidence score (must be between 1 and 5)".to_string(),
            ));
        }

        // Create record
        let record = MockAddressRecord::from(address_data);
        self.addresses.insert(address_data.address.clone(), record);

        Ok(())
    }

    fn get_address_details(&self, address: &str) -> Result<MockAddressRecord, HackerdexError> {
        match self.addresses.get(address) {
            Some(record) => Ok(record.clone()),
            None => Err(HackerdexError::NotFound(format!(
                "Address {} not found",
                address
            ))),
        }
    }

    fn update_address_details(&mut self, address_data: &AddressData) -> Result<(), HackerdexError> {
        // Check if exists
        if !self.addresses.contains_key(&address_data.address) {
            return Err(HackerdexError::NotFound(format!(
                "Address {} not found",
                address_data.address
            )));
        }

        // Validate confidence score
        if address_data.confidence_score < 1 || address_data.confidence_score > 5 {
            return Err(HackerdexError::DatabaseError(
                "Invalid confidence score (must be between 1 and 5)".to_string(),
            ));
        }

        // Get existing record
        let mut record = self.addresses.get(&address_data.address).unwrap().clone();

        // Update fields
        record.entity_name = address_data.entity_name.clone();
        record.category = address_data.category.clone();
        record.risk_level = address_data.risk_level.clone();
        record.source_of_info = address_data.source_of_info.clone();
        record.confidence_score = address_data.confidence_score;
        record.notes = address_data.notes.clone();
        record.updated_at = TimeOffsetDateTime::now_utc();

        // Store updated record
        self.addresses.insert(address_data.address.clone(), record);

        Ok(())
    }

    fn get_all_addresses_by_category(&self, category: &str) -> Vec<MockAddressRecord> {
        self.addresses
            .values()
            .filter(|record| record.category == category)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create test address data
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

    #[test]
    fn test_add_and_get_known_address() {
        let mut db = MockDb::new();

        // Test data
        let test_address = create_test_address(
            "AddTestAddress111111111111111111111111111111",
            "Test Entity",
            "Test Category",
            "Low",
        );

        // Add address
        let add_result = db.add_known_address(&test_address);
        assert!(
            add_result.is_ok(),
            "Failed to add address: {:?}",
            add_result
        );

        // Get address
        let get_result = db.get_address_details(&test_address.address);
        assert!(
            get_result.is_ok(),
            "Failed to get address: {:?}",
            get_result
        );

        let record = get_result.unwrap();
        assert_eq!(record.address, test_address.address);
        assert_eq!(record.entity_name, "Test Entity");
        assert_eq!(record.category, "Test Category");
        assert_eq!(record.risk_level, "Low");
        assert_eq!(record.confidence_score, 3);
    }

    #[test]
    fn test_update_address_details() {
        let mut db = MockDb::new();

        // Initial address
        let initial_address = create_test_address(
            "UpdateTestAddr11111111111111111111111111111",
            "Original Entity",
            "Original Category",
            "Low",
        );

        // Add initial address
        db.add_known_address(&initial_address).unwrap();

        // Updated address
        let updated_address = AddressData {
            address: initial_address.address.clone(),
            entity_name: "Updated Entity".to_string(),
            category: "Updated Category".to_string(),
            risk_level: "High".to_string(),
            source_of_info: "Updated Source".to_string(),
            confidence_score: 5,
            notes: Some("Updated notes for testing".to_string()),
        };

        // Update address
        let update_result = db.update_address_details(&updated_address);
        assert!(
            update_result.is_ok(),
            "Failed to update address: {:?}",
            update_result
        );

        // Get updated address
        let get_result = db.get_address_details(&initial_address.address);
        assert!(
            get_result.is_ok(),
            "Failed to get updated address: {:?}",
            get_result
        );

        let record = get_result.unwrap();
        assert_eq!(record.entity_name, "Updated Entity");
        assert_eq!(record.category, "Updated Category");
        assert_eq!(record.risk_level, "High");
        assert_eq!(record.source_of_info, "Updated Source");
        assert_eq!(record.confidence_score, 5);
        assert_eq!(record.notes, Some("Updated notes for testing".to_string()));
    }

    #[test]
    fn test_get_all_addresses_by_category() {
        let mut db = MockDb::new();

        // Add addresses in different categories
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

        // Add addresses
        for addr in &addresses {
            db.add_known_address(addr).unwrap();
        }

        // Get by category
        let bridge_addresses = db.get_all_addresses_by_category("Test Bridge");
        let dex_addresses = db.get_all_addresses_by_category("Test DEX");

        // Verify counts
        assert_eq!(
            bridge_addresses.len(),
            2,
            "Should find 2 Test Bridge addresses"
        );
        assert_eq!(dex_addresses.len(), 1, "Should find 1 Test DEX address");

        // Verify addresses in Bridge category
        let bridge_addresses: Vec<String> = bridge_addresses
            .iter()
            .map(|record| record.address.clone())
            .collect();

        assert!(bridge_addresses.contains(&addresses[0].address));
        assert!(bridge_addresses.contains(&addresses[1].address));
        assert!(!bridge_addresses.contains(&addresses[2].address));
    }

    #[test]
    fn test_address_not_found() {
        let mut db = MockDb::new();

        // Try to get non-existent address
        let get_result = db.get_address_details("NonExistentAddress111111111111111111111");
        assert!(
            get_result.is_err(),
            "Expected error for non-existent address"
        );

        match get_result {
            Err(HackerdexError::NotFound(_)) => {} // Expected error
            Err(e) => panic!("Unexpected error type: {}", e),
            Ok(_) => panic!("Expected error but got success"),
        }

        // Try to update non-existent address
        let non_existent = create_test_address(
            "NonExistentAddress111111111111111111111",
            "Invalid Entity",
            "Invalid",
            "Low",
        );

        let update_result = db.update_address_details(&non_existent);
        assert!(
            update_result.is_err(),
            "Expected error for updating non-existent address"
        );

        match update_result {
            Err(HackerdexError::NotFound(_)) => {} // Expected error
            Err(e) => panic!("Unexpected error type: {}", e),
            Ok(_) => panic!("Expected error but got success"),
        }
    }

    #[test]
    fn test_duplicate_address() {
        let mut db = MockDb::new();

        // Add first address
        let test_address = create_test_address(
            "DuplicateTest111111111111111111111111111111",
            "First Entity",
            "Test Category",
            "Low",
        );

        db.add_known_address(&test_address).unwrap();

        // Try to add duplicate
        let duplicate = create_test_address(
            "DuplicateTest111111111111111111111111111111",
            "Second Entity",
            "Another Category",
            "High",
        );

        let result = db.add_known_address(&duplicate);
        assert!(result.is_err(), "Expected error for duplicate address");

        match result {
            Err(HackerdexError::DatabaseError(_)) => {} // Expected error
            Err(e) => panic!("Unexpected error type: {}", e),
            Ok(_) => panic!("Expected error but got success"),
        }
    }

    #[test]
    fn test_invalid_confidence_score() {
        let mut db = MockDb::new();

        // Test with too low confidence score
        let invalid_low = AddressData {
            address: "InvalidScore1111111111111111111111111111".to_string(),
            entity_name: "Invalid Score Entity".to_string(),
            category: "Test Category".to_string(),
            risk_level: "Low".to_string(),
            source_of_info: "Test".to_string(),
            confidence_score: 0, // Too low (valid range is 1-5)
            notes: None,
        };

        let result_low = db.add_known_address(&invalid_low);
        assert!(
            result_low.is_err(),
            "Expected error for invalid low confidence score"
        );

        // Test with too high confidence score
        let invalid_high = AddressData {
            address: "InvalidScore2222222222222222222222222222".to_string(),
            entity_name: "Invalid Score Entity".to_string(),
            category: "Test Category".to_string(),
            risk_level: "Low".to_string(),
            source_of_info: "Test".to_string(),
            confidence_score: 6, // Too high (valid range is 1-5)
            notes: None,
        };

        let result_high = db.add_known_address(&invalid_high);
        assert!(
            result_high.is_err(),
            "Expected error for invalid high confidence score"
        );
    }
}
