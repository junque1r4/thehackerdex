use hackerdex::db::models::AddressRecord;
use hackerdex::heuristic_engine::bidirectional_flow_heuristics::check_bidirectional_flow;
use hackerdex::heuristic_engine::types::{WalletContext, WalletTransaction};
use sqlx::types::time::OffsetDateTime;
use std::time::{SystemTime, UNIX_EPOCH};

// Helper function to create a wallet transaction
fn create_wallet_transaction(
    signature: &str,
    counterparty: &str,
    amount: f64,
    timestamp: i64,
    category: Option<&str>,
    risk_level: Option<&str>,
) -> WalletTransaction {
    let counterparty_record = if let (Some(category), Some(risk_level)) = (category, risk_level) {
        // Create current timestamp for record
        let current_time = OffsetDateTime::from_unix_timestamp(timestamp).unwrap();

        Some(AddressRecord {
            address: counterparty.to_string(),
            category: category.to_string(),
            entity_name: format!("Test Entity for {}", counterparty),
            risk_level: risk_level.to_string(),
            source_of_info: "Test".to_string(),
            notes: None,
            confidence_score: 3,
            created_at: current_time,
            updated_at: current_time,
        })
    } else {
        None
    };

    WalletTransaction {
        signature: signature.to_string(),
        counterparty: counterparty.to_string(),
        amount,
        timestamp,
        is_known_counterparty: counterparty_record.is_some(),
        counterparty_record,
    }
}

// Helper function to get current timestamp
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[test]
fn test_bidirectional_flow_positive() {
    // Setup a wallet context with bidirectional flow
    let mut wallet_context = WalletContext::new("test_wallet".to_string());

    // Current time and earlier timestamps
    let current = now();
    let day_ago = current - 24 * 3600;
    let days_ago_3 = current - 3 * 24 * 3600;

    // Add incoming transactions from counterparty
    wallet_context
        .recent_incoming_txs
        .push(create_wallet_transaction(
            "sig1",
            "counterparty1",
            10.0,
            days_ago_3,
            Some("Untagged"),
            Some("Low"),
        ));

    wallet_context
        .recent_incoming_txs
        .push(create_wallet_transaction(
            "sig2",
            "counterparty1",
            5.0,
            day_ago,
            Some("Untagged"),
            Some("Low"),
        ));

    // Add outgoing transactions to the same counterparty
    wallet_context
        .recent_outgoing_txs
        .push(create_wallet_transaction(
            "sig3",
            "counterparty1",
            9.0,
            days_ago_3 + 3600, // 1 hour after first incoming
            Some("Untagged"),
            Some("Low"),
        ));

    wallet_context
        .recent_outgoing_txs
        .push(create_wallet_transaction(
            "sig4",
            "counterparty1",
            4.5,
            day_ago + 7200, // 2 hours after second incoming
            Some("Untagged"),
            Some("Low"),
        ));

    // Run the bidirectional flow check
    let (has_bidirectional_flow, score) = check_bidirectional_flow(&wallet_context);

    // Assert that bidirectional flow was detected
    assert!(has_bidirectional_flow, "Should detect bidirectional flow");
    assert!(score > 0.0, "Score should be greater than 0");
    println!("Bidirectional flow score: {}", score);
}

#[test]
fn test_bidirectional_flow_negative() {
    // Setup a wallet context without bidirectional flow
    let mut wallet_context = WalletContext::new("test_wallet".to_string());

    // Current time and earlier timestamps
    let current = now();
    let day_ago = current - 24 * 3600;
    let days_ago_3 = current - 3 * 24 * 3600;

    // Add incoming transactions from one counterparty
    wallet_context
        .recent_incoming_txs
        .push(create_wallet_transaction(
            "sig1",
            "counterparty1",
            10.0,
            days_ago_3,
            Some("Untagged"),
            Some("Low"),
        ));

    wallet_context
        .recent_incoming_txs
        .push(create_wallet_transaction(
            "sig2",
            "counterparty2",
            5.0,
            day_ago,
            Some("Untagged"),
            Some("Low"),
        ));

    // Add outgoing transactions to different counterparties
    wallet_context
        .recent_outgoing_txs
        .push(create_wallet_transaction(
            "sig3",
            "counterparty3",
            9.0,
            days_ago_3 + 3600,
            Some("Untagged"),
            Some("Low"),
        ));

    wallet_context
        .recent_outgoing_txs
        .push(create_wallet_transaction(
            "sig4",
            "counterparty4",
            4.5,
            day_ago + 7200,
            Some("Untagged"),
            Some("Low"),
        ));

    // Run the bidirectional flow check
    let (has_bidirectional_flow, score) = check_bidirectional_flow(&wallet_context);

    // Assert that bidirectional flow was not detected
    assert!(
        !has_bidirectional_flow,
        "Should not detect bidirectional flow"
    );
    assert_eq!(score, 0.0, "Score should be 0");
}

#[test]
fn test_bidirectional_flow_with_safe_trading_category() {
    // Setup a wallet context with bidirectional flow but to a DEX (should be excluded)
    let mut wallet_context = WalletContext::new("test_wallet".to_string());

    // Current time and earlier timestamps
    let current = now();
    let day_ago = current - 24 * 3600;
    let days_ago_3 = current - 3 * 24 * 3600;

    // Add incoming transactions from DEX
    wallet_context
        .recent_incoming_txs
        .push(create_wallet_transaction(
            "sig1",
            "dex_address",
            10.0,
            days_ago_3,
            Some("DEX"), // This is a safe trading category
            Some("Low"),
        ));

    wallet_context
        .recent_incoming_txs
        .push(create_wallet_transaction(
            "sig2",
            "dex_address",
            5.0,
            day_ago,
            Some("DEX"),
            Some("Low"),
        ));

    // Add outgoing transactions to the same DEX
    wallet_context
        .recent_outgoing_txs
        .push(create_wallet_transaction(
            "sig3",
            "dex_address",
            9.0,
            days_ago_3 + 3600,
            Some("DEX"),
            Some("Low"),
        ));

    wallet_context
        .recent_outgoing_txs
        .push(create_wallet_transaction(
            "sig4",
            "dex_address",
            4.5,
            day_ago + 7200,
            Some("DEX"),
            Some("Low"),
        ));

    // Run the bidirectional flow check
    let (has_bidirectional_flow, score) = check_bidirectional_flow(&wallet_context);

    // Assert that bidirectional flow was NOT detected (because it's a DEX)
    assert!(
        !has_bidirectional_flow,
        "Should not detect bidirectional flow with DEX"
    );
    assert_eq!(score, 0.0, "Score should be 0 for DEX interactions");
}
