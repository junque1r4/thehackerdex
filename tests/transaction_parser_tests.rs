use hackerdex::analysis::transaction_parser::{get_program_address_by_name, validate_address};

#[test]
fn test_validate_address() {
    // Valid address
    let valid_address = "11111111111111111111111111111111";
    assert!(validate_address(valid_address).is_ok());

    // Invalid address
    let invalid_address = "not_a_valid_address";
    assert!(validate_address(invalid_address).is_err());
}

#[test]
fn test_get_program_address_by_name() {
    // Core Solana programs
    assert_eq!(
        get_program_address_by_name("System Program"),
        Some("11111111111111111111111111111111".to_string())
    );

    // SPL programs
    assert_eq!(
        get_program_address_by_name("SPL Token"),
        Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string())
    );

    // New programs we added
    assert_eq!(
        get_program_address_by_name("Jupiter Aggregator"),
        Some("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4".to_string())
    );

    // Unknown program
    assert_eq!(get_program_address_by_name("Unknown Program"), None);
}
