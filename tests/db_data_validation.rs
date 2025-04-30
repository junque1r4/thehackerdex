use hackerdex::db::AddressData;

/// This module contains tests related to valid Solana address formats
/// and other data validation logic for database operations
#[cfg(test)]
mod data_validation_tests {
    use super::*;

    /// Tests for validating Solana address format
    #[test]
    fn test_address_format_validation() {
        // Regular expected length (44 characters) base-58 Solana address
        let valid_address = "EATYKPBH2wUsSx3HbqVgiJWVVCkr6ZNJ5g6AVZWY7JHS";
        assert!(is_valid_solana_address(valid_address));

        // Too short
        let too_short = "Short1111111";
        assert!(!is_valid_solana_address(too_short));

        // Too long
        let too_long = "TooLongAddress1111111111111111111111111111111111111";
        assert!(!is_valid_solana_address(too_long));

        // Invalid characters (contains 'I', 'O', 'l', '0')
        let invalid_chars = "InvalidO00lI11111111111111111111111111111";
        assert!(!is_valid_solana_address(invalid_chars));
    }

    /// Tests for validating risk level values
    #[test]
    fn test_risk_level_validation() {
        // Valid risk levels
        assert!(is_valid_risk_level("Low"));
        assert!(is_valid_risk_level("Medium"));
        assert!(is_valid_risk_level("High"));
        assert!(is_valid_risk_level("Critical"));

        // Invalid risk levels
        assert!(!is_valid_risk_level("Unknown"));
        assert!(!is_valid_risk_level("Extreme"));
        assert!(!is_valid_risk_level(""));
    }

    /// Tests for validating complete AddressData objects
    #[test]
    fn test_address_data_validation() {
        // Valid address data
        let valid_data = AddressData {
            address: "EATYKPBH2wUsSx3HbqVgiJWVVCkr6ZNJ5g6AVZWY7JHS".to_string(),
            entity_name: "Valid Entity".to_string(),
            category: "Test Category".to_string(),
            risk_level: "Low".to_string(),
            source_of_info: "Test Source".to_string(),
            confidence_score: 3,
            notes: Some("Valid test notes".to_string()),
        };

        assert!(validate_address_data(&valid_data).is_ok());

        // Invalid address data - empty entity name
        let invalid_entity = AddressData {
            address: "ValidAddress111111111111111111111111111111".to_string(),
            entity_name: "".to_string(), // Empty entity name
            category: "Test Category".to_string(),
            risk_level: "Low".to_string(),
            source_of_info: "Test Source".to_string(),
            confidence_score: 3,
            notes: Some("Valid test notes".to_string()),
        };

        assert!(validate_address_data(&invalid_entity).is_err());

        // Invalid address data - invalid risk level
        let invalid_risk = AddressData {
            address: "ValidAddress111111111111111111111111111111".to_string(),
            entity_name: "Valid Entity".to_string(),
            category: "Test Category".to_string(),
            risk_level: "Invalid".to_string(), // Invalid risk level
            source_of_info: "Test Source".to_string(),
            confidence_score: 3,
            notes: Some("Valid test notes".to_string()),
        };

        assert!(validate_address_data(&invalid_risk).is_err());

        // Invalid address data - invalid confidence score
        let invalid_score = AddressData {
            address: "ValidAddress111111111111111111111111111111".to_string(),
            entity_name: "Valid Entity".to_string(),
            category: "Test Category".to_string(),
            risk_level: "Low".to_string(),
            source_of_info: "Test Source".to_string(),
            confidence_score: 6, // Invalid score (valid is 1-5)
            notes: Some("Valid test notes".to_string()),
        };

        assert!(validate_address_data(&invalid_score).is_err());
    }

    /// Helper function to validate Solana address format
    /// This is simplified and could be expanded with more rigorous checks
    fn is_valid_solana_address(address: &str) -> bool {
        // Basic check: just verify length and that it contains only base58 characters
        if address.len() != 44 {
            return false;
        }

        // Base58 doesn't use: I, O, l, 0
        let disallowed_chars = ['I', 'O', 'l', '0'];
        for c in address.chars() {
            if disallowed_chars.contains(&c) {
                return false;
            }
        }

        true
    }

    /// Helper function to validate risk level
    fn is_valid_risk_level(risk_level: &str) -> bool {
        match risk_level {
            "Low" | "Medium" | "High" | "Critical" => true,
            _ => false,
        }
    }

    /// Helper function to validate the entire AddressData object
    fn validate_address_data(data: &AddressData) -> Result<(), String> {
        // Validate address format
        if !is_valid_solana_address(&data.address) {
            return Err(format!("Invalid Solana address format: {}", data.address));
        }

        // Validate entity name is not empty
        if data.entity_name.is_empty() {
            return Err("Entity name cannot be empty".to_string());
        }

        // Validate category is not empty
        if data.category.is_empty() {
            return Err("Category cannot be empty".to_string());
        }

        // Validate risk level
        if !is_valid_risk_level(&data.risk_level) {
            return Err(format!("Invalid risk level: {}", data.risk_level));
        }

        // Validate source of info is not empty
        if data.source_of_info.is_empty() {
            return Err("Source of info cannot be empty".to_string());
        }

        // Validate confidence score
        if data.confidence_score < 1 || data.confidence_score > 5 {
            return Err(format!(
                "Invalid confidence score: {} (must be between 1-5)",
                data.confidence_score
            ));
        }

        Ok(())
    }
}
