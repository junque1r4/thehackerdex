//! Tests for flow pattern heuristics
//!
//! These tests verify that the rapid_dispersal and fund_consolidation heuristics
//! work correctly in different scenarios.

use hackerdex::analysis::program_analyzer::ProgramAnalysis;
use hackerdex::analysis::transaction_analysis::TransactionAnalysisData;
use hackerdex::analysis::transaction_parser::ParsedTransaction;
use hackerdex::db::models::AddressRecord;
use hackerdex::heuristic_engine::transaction_heuristics;
use hackerdex::heuristic_engine::types::{WalletContext, WalletTransaction};
use sqlx::types::time::OffsetDateTime;
use std::time::SystemTime;

// Helper function to create a transaction analysis data object
fn create_test_transaction_analysis() -> TransactionAnalysisData {
    let parsed_tx = ParsedTransaction {
        signature: "test_signature".to_string(),
        program_ids: vec!["11111111111111111111111111111111".to_string()], // System program
        involved_accounts: vec![
            "wallet1".to_string(),
            "wallet2".to_string(),
            "wallet3".to_string(),
        ],
        pre_token_balances: vec![],
        post_token_balances: vec![],
        execution_status: Some("Success".to_string()),
        fee: 5000,
    };

    // Create empty program analysis
    let program_analysis = vec![ProgramAnalysis {
        program_id: "11111111111111111111111111111111".to_string(),
        is_known: true,
        address_record: None,
    }];

    // Create empty wallet analysis
    let wallet_direct_analysis = vec![];

    TransactionAnalysisData {
        parsed_transaction: parsed_tx,
        program_analysis,
        wallet_direct_analysis,
        heuristic_flags: None,
        risk_score: None,
        risk_factors: None,
    }
}

// Helper function to create a wallet context for testing
fn create_test_wallet_context(address: &str) -> WalletContext {
    WalletContext {
        address: address.to_string(),
        creation_timestamp: Some(chrono::Utc::now().timestamp() - 86400 * 7), // 1 week old
        sol_balance: 10.0,
        known_address_record: None,
        tx_count_24h: 5,
        tx_count_7d: 20,
        volume_24h: 2.5,
        volume_7d: 15.0,
        recent_incoming_txs: Vec::new(), // Will be populated in specific tests
        recent_outgoing_txs: Vec::new(), // Will be populated in specific tests
    }
}

// Helper function to get current time as OffsetDateTime
fn now_as_offset_datetime() -> OffsetDateTime {
    let now = SystemTime::now();
    let secs = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    OffsetDateTime::from_unix_timestamp(secs).unwrap()
}

#[test]
fn test_check_rapid_dispersal_positive() {
    // Create basic test data
    let tx_analysis = create_test_transaction_analysis();
    let mut wallet_context = create_test_wallet_context("main_wallet");

    // Create a significant incoming transaction
    let incoming_tx = WalletTransaction {
        signature: "incoming_tx".to_string(),
        timestamp: chrono::Utc::now().timestamp() - 1800, // 30 minutes ago
        amount: 5.0,                                      // 5 SOL
        counterparty: "sender_wallet".to_string(),
        is_known_counterparty: false,
        counterparty_record: None,
    };

    // Create multiple outgoing transactions shortly after the incoming one
    let mut outgoing_txs = Vec::new();
    let base_timestamp = incoming_tx.timestamp + 300; // 5 minutes after receiving

    // Add 5 outgoing transactions to different recipients
    for i in 0..5 {
        outgoing_txs.push(WalletTransaction {
            signature: format!("outgoing_tx_{}", i),
            timestamp: base_timestamp + (i as i64 * 60), // Staggered by 1 minute each
            amount: 0.9, // Almost 1 SOL each, totaling 4.5 SOL (90% of incoming)
            counterparty: format!("recipient_wallet_{}", i),
            is_known_counterparty: false,
            counterparty_record: None,
        });
    }

    // Populate the wallet context
    wallet_context.recent_incoming_txs.push(incoming_tx);
    wallet_context.recent_outgoing_txs = outgoing_txs;

    // Run the heuristic
    let (is_detected, score) =
        transaction_heuristics::check_rapid_dispersal(&tx_analysis, &wallet_context);

    // Assert that the pattern is detected
    assert!(is_detected, "Rapid dispersal pattern should be detected");
    assert!(
        score > 0.5,
        "Dispersal score should be significant (>0.5), got: {}",
        score
    );
}

#[test]
fn test_check_rapid_dispersal_negative() {
    // Create basic test data
    let tx_analysis = create_test_transaction_analysis();
    let mut wallet_context = create_test_wallet_context("main_wallet");

    // Create a significant incoming transaction
    let incoming_tx = WalletTransaction {
        signature: "incoming_tx".to_string(),
        timestamp: chrono::Utc::now().timestamp() - 1800, // 30 minutes ago
        amount: 5.0,                                      // 5 SOL
        counterparty: "sender_wallet".to_string(),
        is_known_counterparty: false,
        counterparty_record: None,
    };

    // Create just one outgoing transaction - not enough for dispersal
    let outgoing_tx = WalletTransaction {
        signature: "outgoing_tx".to_string(),
        timestamp: incoming_tx.timestamp + 300, // 5 minutes after receiving
        amount: 4.5,                            // Almost all of the received amount
        counterparty: "recipient_wallet".to_string(),
        is_known_counterparty: false,
        counterparty_record: None,
    };

    // Populate the wallet context
    wallet_context.recent_incoming_txs.push(incoming_tx);
    wallet_context.recent_outgoing_txs.push(outgoing_tx);

    // Run the heuristic
    let (is_detected, score) =
        transaction_heuristics::check_rapid_dispersal(&tx_analysis, &wallet_context);

    // Assert that the pattern is not detected
    assert!(
        !is_detected,
        "Rapid dispersal pattern should not be detected with just one recipient"
    );
    assert!(
        score < 0.1,
        "Dispersal score should be very low, got: {}",
        score
    );
}

#[test]
fn test_check_fund_consolidation_positive() {
    // Create basic test data
    let tx_analysis = create_test_transaction_analysis();
    let mut wallet_context = create_test_wallet_context("collector_wallet");

    // Create multiple incoming transactions from risky sources within 24 hours
    let mut incoming_txs = Vec::new();
    let base_timestamp = chrono::Utc::now().timestamp() - 12 * 3600; // 12 hours ago
    let now = now_as_offset_datetime();

    // Add 4 incoming transactions from suspicious sources
    for i in 0..4 {
        // Create a risky address record
        let record = AddressRecord {
            address: format!("suspicious_sender_{}", i),
            entity_name: format!("Suspicious Entity {}", i),
            category: "Suspicious Exchange".to_string(),
            risk_level: "High".to_string(),
            source_of_info: "Test".to_string(),
            confidence_score: 4,
            notes: None,
            created_at: now,
            updated_at: now,
        };

        incoming_txs.push(WalletTransaction {
            signature: format!("incoming_tx_{}", i),
            timestamp: base_timestamp + (i as i64 * 3600), // Staggered by 1 hour each
            amount: 1.0,                                   // 1 SOL each
            counterparty: format!("suspicious_sender_{}", i),
            is_known_counterparty: true,
            counterparty_record: Some(record),
        });
    }

    // Populate the wallet context
    wallet_context.recent_incoming_txs = incoming_txs;

    // Run the heuristic
    let (is_detected, score) =
        transaction_heuristics::check_fund_consolidation(&tx_analysis, &wallet_context);

    // Assert that the pattern is detected
    assert!(is_detected, "Fund consolidation pattern should be detected");
    assert!(
        score > 0.5,
        "Consolidation score should be significant (>0.5), got: {}",
        score
    );
}

#[test]
fn test_check_fund_consolidation_negative() {
    // Create basic test data
    let tx_analysis = create_test_transaction_analysis();
    let mut wallet_context = create_test_wallet_context("normal_wallet");

    // Create multiple incoming transactions but not from risky sources
    let mut incoming_txs = Vec::new();
    let base_timestamp = chrono::Utc::now().timestamp() - 12 * 3600; // 12 hours ago

    // Add 4 incoming transactions from normal sources
    for i in 0..4 {
        incoming_txs.push(WalletTransaction {
            signature: format!("incoming_tx_{}", i),
            timestamp: base_timestamp + (i as i64 * 3600), // Staggered by 1 hour each
            amount: 1.0,                                   // 1 SOL each
            counterparty: format!("normal_sender_{}", i),
            is_known_counterparty: false,
            counterparty_record: None, // No risk record
        });
    }

    // Populate the wallet context
    wallet_context.recent_incoming_txs = incoming_txs;

    // Run the heuristic
    let (is_detected, score) =
        transaction_heuristics::check_fund_consolidation(&tx_analysis, &wallet_context);

    // Assert that the pattern is not detected
    assert!(
        !is_detected,
        "Fund consolidation pattern should not be detected with normal sources"
    );
    assert!(
        score < 0.1,
        "Consolidation score should be very low, got: {}",
        score
    );
}

#[test]
fn test_heuristic_integration() {
    // This test verifies that the heuristics are properly integrated into the run_all_heuristics function
    use hackerdex::heuristic_engine;

    // Create a transaction analysis with normal transaction data
    let tx_analysis = create_test_transaction_analysis();

    // Create a wallet context with suspicious flow patterns
    let mut wallet_context = create_test_wallet_context("suspicious_wallet");

    // Create a significant incoming transaction
    let incoming_tx = WalletTransaction {
        signature: "incoming_tx".to_string(),
        timestamp: chrono::Utc::now().timestamp() - 1800, // 30 minutes ago
        amount: 5.0,                                      // 5 SOL
        counterparty: "sender_wallet".to_string(),
        is_known_counterparty: false,
        counterparty_record: None,
    };

    // Create multiple outgoing transactions shortly after the incoming one
    let mut outgoing_txs = Vec::new();
    let base_timestamp = incoming_tx.timestamp + 300; // 5 minutes after receiving

    // Add 5 outgoing transactions to different recipients
    for i in 0..5 {
        outgoing_txs.push(WalletTransaction {
            signature: format!("outgoing_tx_{}", i),
            timestamp: base_timestamp + (i as i64 * 60), // Staggered by 1 minute each
            amount: 0.9, // Almost 1 SOL each, totaling 4.5 SOL (90% of incoming)
            counterparty: format!("recipient_wallet_{}", i),
            is_known_counterparty: false,
            counterparty_record: None,
        });
    }

    // Populate the wallet context
    wallet_context.recent_incoming_txs.push(incoming_tx);
    wallet_context.recent_outgoing_txs = outgoing_txs;

    // Run the heuristics
    let heuristic_flags = heuristic_engine::run_all_heuristics(&tx_analysis, &wallet_context);

    // Check that the rapid dispersal pattern was detected
    assert!(
        heuristic_flags.rapid_dispersal_pattern,
        "Rapid dispersal pattern should be detected in integration test"
    );

    // Check that the score was added to custom flags
    assert!(
        heuristic_flags
            .custom_flags
            .contains_key("rapid_dispersal_score"),
        "Custom flag 'rapid_dispersal_score' should be present"
    );

    // Check that the overall suspicion score was increased
    assert!(
        heuristic_flags.get_overall_suspicion_score() > 1.0,
        "Overall suspicion score should be increased due to flow pattern"
    );
}
