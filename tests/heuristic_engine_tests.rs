use hackerdex::analysis::program_analyzer::ProgramAnalysis;
use hackerdex::analysis::transaction_analysis::TransactionAnalysisData;
use hackerdex::analysis::transaction_parser::ParsedTransaction;
use hackerdex::analysis::wallet_analyzer::WalletDirectAnalysisResult;
use hackerdex::db::models::AddressRecord;
use hackerdex::heuristic_engine::transaction_heuristics::{
    check_direct_illicit_interaction, check_interaction_with_risky_categories,
};
use hackerdex::heuristic_engine::wallet_heuristics::{
    check_high_frequency_volume, check_pass_through, check_structuring_patterns,
};
use hackerdex::heuristic_engine::{HeuristicFlags, WalletContext, WalletTransaction};
use solana_transaction_status::UiTransactionTokenBalance;
use sqlx::types::time::OffsetDateTime;

#[test]
fn test_heuristic_flags_default() {
    let flags = HeuristicFlags::new();

    // All boolean flags should be false
    assert!(!flags.is_high_frequency);
    assert!(!flags.is_pass_through);
    assert!(!flags.direct_illicit_interaction);
    assert!(!flags.is_new_wallet);
    assert!(!flags.rapid_dispersal_pattern);
    assert!(!flags.fund_consolidation_pattern);

    // All numeric scores should be 0.0
    assert_eq!(flags.structuring_score, 0.0);
    assert_eq!(flags.risky_category_interaction_score, 0.0);
    assert_eq!(flags.risky_funding_source_ratio, 0.0);
    assert_eq!(flags.risky_spending_destination_ratio, 0.0);

    // Custom flags should be empty
    assert!(flags.custom_flags.is_empty());

    // Overall score should be 0.0
    assert_eq!(flags.get_overall_suspicion_score(), 0.0);

    // No flags should be triggered
    assert!(flags.get_triggered_flags_description().is_empty());
}

#[test]
fn test_heuristic_flags_with_values() {
    let mut flags = HeuristicFlags::new();
    flags.is_high_frequency = true;
    flags.direct_illicit_interaction = true;
    flags.structuring_score = 0.7;
    flags.risky_category_interaction_score = 0.6;

    // Add a custom flag
    flags
        .custom_flags
        .insert("unusual_token_swaps".to_string(), 0.8);

    // Check overall score calculation (should be sum of all factors)
    // 1.0 (high freq) + 2.0 (direct illicit) + 0.7 (structuring) + 0.6 (risky category) + 0.8 (custom) = 5.1
    assert!((flags.get_overall_suspicion_score() - 5.1).abs() < 0.001); // Use approximate float comparison

    // Check triggered flags descriptions
    let triggered = flags.get_triggered_flags_description();
    // All 5 flags should be triggered (high freq, direct illicit, structuring > 0.5,
    // risky category > 0.5, and custom flag > 0.5)
    assert_eq!(triggered.len(), 5);
    assert!(triggered.contains(&"High frequency transaction patterns detected".to_string()));
    assert!(triggered.contains(&"Direct interaction with known illicit address".to_string()));
    assert!(triggered.iter().any(|s| s.contains("Possible structuring")));
    assert!(
        triggered
            .iter()
            .any(|s| s.contains("High interaction with risky categories"))
    );
    assert!(triggered.iter().any(|s| s.contains("Custom flag")));
}

#[test]
fn test_wallet_context_new_wallet() {
    // Create a wallet with creation time set to now
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut wallet = WalletContext::new("address123".to_string());
    wallet.creation_timestamp = Some(now);

    // A wallet created now should be considered new if the threshold is > 0
    assert!(wallet.is_new_wallet(7)); // Less than 7 days old

    // Create a wallet with creation time set to 30 days ago
    let thirty_days_ago = now - (30 * 24 * 60 * 60);
    wallet.creation_timestamp = Some(thirty_days_ago);

    // Should not be considered new with 7 day threshold
    assert!(!wallet.is_new_wallet(7));

    // Should be considered new with 60 day threshold
    assert!(wallet.is_new_wallet(60));
}

#[test]
fn test_wallet_context_high_frequency() {
    let mut wallet = WalletContext::new("address456".to_string());

    // Initially should not have high frequency
    assert!(!wallet.has_high_frequency_pattern(50));

    // Update to have high transaction count
    wallet.tx_count_24h = 75;

    // Should now have high frequency if threshold is 50
    assert!(wallet.has_high_frequency_pattern(50));

    // Should not have high frequency if threshold is 100
    assert!(!wallet.has_high_frequency_pattern(100));
}

#[test]
fn test_check_high_frequency_volume() {
    // Create a wallet with normal transaction activity
    let mut wallet = WalletContext::new("normal_wallet".to_string());
    wallet.tx_count_24h = 10;
    wallet.volume_24h = 1000.0;

    // Should not detect high frequency
    let (is_high_freq, score) = check_high_frequency_volume(&wallet);
    assert!(!is_high_freq);
    assert_eq!(score, 0.0);

    // Create a wallet with high transaction count but normal volume
    let mut high_count_wallet = WalletContext::new("high_count_wallet".to_string());
    high_count_wallet.tx_count_24h = 60; // Above the 50 threshold
    high_count_wallet.volume_24h = 5000.0;

    // Should detect high frequency
    let (is_high_freq, score) = check_high_frequency_volume(&high_count_wallet);
    assert!(is_high_freq);
    assert!(score > 0.0);

    // Create a wallet with normal count but high volume
    let mut high_volume_wallet = WalletContext::new("high_volume_wallet".to_string());
    high_volume_wallet.tx_count_24h = 30;
    high_volume_wallet.volume_24h = 150000.0; // Above the 100,000 threshold

    // Should detect high frequency
    let (is_high_freq, score) = check_high_frequency_volume(&high_volume_wallet);
    assert!(is_high_freq);
    assert!(score > 0.0);

    // Test transaction bursts
    let mut burst_wallet = WalletContext::new("burst_wallet".to_string());

    // Add transactions with timestamps clustered within the same hour
    let base_timestamp = 1000000000;

    // Add 15 transactions within the same hour (burst)
    for i in 0..15 {
        burst_wallet.recent_incoming_txs.push(WalletTransaction {
            signature: format!("sig{}", i),
            timestamp: base_timestamp + i * 60, // One minute apart
            amount: 10.0,
            counterparty: "counterparty".to_string(),
            is_known_counterparty: false,
            counterparty_record: None,
        });
    }

    // Should detect high frequency due to burst
    let (is_high_freq, score) = check_high_frequency_volume(&burst_wallet);
    assert!(is_high_freq);
    assert!(score > 0.0);
}

#[test]
fn test_check_structuring_patterns() {
    // Create a wallet with no structuring patterns
    let normal_wallet = WalletContext::new("normal_wallet".to_string());

    // Not enough transactions, should return 0.0
    assert_eq!(check_structuring_patterns(&normal_wallet), 0.0);

    // Create a wallet with structuring patterns
    let mut structuring_wallet = WalletContext::new("structuring_wallet".to_string());

    // Add multiple small transactions of similar amounts within a day
    let base_timestamp = 1000000000;

    // Add 6 outgoing transactions with similar small amounts
    for i in 0..6 {
        structuring_wallet
            .recent_outgoing_txs
            .push(WalletTransaction {
                signature: format!("sig{}", i),
                timestamp: base_timestamp + i * 3600, // One hour apart
                amount: 9.0 + (i as f64 * 0.1),       // Around 9 SOL
                counterparty: "same_destination".to_string(),
                is_known_counterparty: false,
                counterparty_record: None,
            });
    }

    // Should detect structuring
    let score = check_structuring_patterns(&structuring_wallet);
    assert!(score > 0.0, "Structuring score should be positive");

    // Add some random transactions that don't fit the pattern
    let mut mixed_wallet = WalletContext::new("mixed_wallet".to_string());

    // Add only 3 similar transactions (below the MIN_TRANSACTIONS_FOR_STRUCTURING threshold)
    // This guarantees a lower score since some time windows won't have enough transactions
    for i in 0..3 {
        mixed_wallet.recent_outgoing_txs.push(WalletTransaction {
            signature: format!("sig{}", i),
            timestamp: base_timestamp + i * 7200, // Two hours apart
            amount: 9.5,
            counterparty: "destination1".to_string(),
            is_known_counterparty: false,
            counterparty_record: None,
        });
    }

    // Add some dissimilar transactions in another time window
    for i in 0..7 {
        mixed_wallet.recent_outgoing_txs.push(WalletTransaction {
            signature: format!("sigB{}", i),
            timestamp: base_timestamp + 100000 + i * 3600,
            amount: 50.0 + (i as f64 * 10.0), // Much larger amounts with significant variance
            counterparty: format!("destination{}", i + 2),
            is_known_counterparty: false,
            counterparty_record: None,
        });
    }

    // Should detect some structuring but score should be lower
    let mixed_score = check_structuring_patterns(&mixed_wallet);
    assert!(
        mixed_score > 0.0,
        "Mixed wallet should still have non-zero structuring score"
    );
    assert!(
        mixed_score < score,
        "Mixed wallet should have lower structuring score than pure structuring wallet"
    );
}

#[test]
fn test_check_pass_through() {
    // Create a wallet with no pass-through behavior
    let normal_wallet = WalletContext::new("normal_wallet".to_string());

    // Not enough transactions, should return (false, 0.0)
    let (is_pass_through, score) = check_pass_through(&normal_wallet);
    assert!(!is_pass_through);
    assert_eq!(score, 0.0);

    // Create a wallet with clear pass-through behavior
    let mut pass_through_wallet = WalletContext::new("pass_through_wallet".to_string());

    // Base timestamp
    let base_timestamp = 1000000000;

    // Add incoming transactions
    pass_through_wallet
        .recent_incoming_txs
        .push(WalletTransaction {
            signature: "in1".to_string(),
            timestamp: base_timestamp,
            amount: 50.0,
            counterparty: "sender1".to_string(),
            is_known_counterparty: false,
            counterparty_record: None,
        });

    pass_through_wallet
        .recent_incoming_txs
        .push(WalletTransaction {
            signature: "in2".to_string(),
            timestamp: base_timestamp + 10000, // Later transaction
            amount: 100.0,
            counterparty: "sender2".to_string(),
            is_known_counterparty: false,
            counterparty_record: None,
        });

    // Add outgoing transactions very soon after incoming
    pass_through_wallet
        .recent_outgoing_txs
        .push(WalletTransaction {
            signature: "out1".to_string(),
            timestamp: base_timestamp + 120, // 2 minutes later
            amount: 49.0,                    // Slightly less (accounting for fees)
            counterparty: "recipient1".to_string(),
            is_known_counterparty: false,
            counterparty_record: None,
        });

    pass_through_wallet
        .recent_outgoing_txs
        .push(WalletTransaction {
            signature: "out2".to_string(),
            timestamp: base_timestamp + 10180, // 3 minutes after second incoming
            amount: 98.0,                      // Slightly less (accounting for fees)
            counterparty: "recipient2".to_string(),
            is_known_counterparty: false,
            counterparty_record: None,
        });

    // Should detect pass-through behavior
    let (is_pass_through, score) = check_pass_through(&pass_through_wallet);
    assert!(is_pass_through);
    assert!(score > 0.5, "Pass-through score should be significant");

    // Create a wallet with delayed transfers (not pass-through)
    let mut delayed_wallet = WalletContext::new("delayed_wallet".to_string());

    // Add incoming transaction
    delayed_wallet.recent_incoming_txs.push(WalletTransaction {
        signature: "in1".to_string(),
        timestamp: base_timestamp,
        amount: 50.0,
        counterparty: "sender1".to_string(),
        is_known_counterparty: false,
        counterparty_record: None,
    });

    // Add outgoing transaction much later
    delayed_wallet.recent_outgoing_txs.push(WalletTransaction {
        signature: "out1".to_string(),
        timestamp: base_timestamp + 12000, // 3+ hours later
        amount: 48.0,
        counterparty: "recipient1".to_string(),
        is_known_counterparty: false,
        counterparty_record: None,
    });

    // Should not detect pass-through behavior due to delay
    let (is_pass_through, score) = check_pass_through(&delayed_wallet);
    assert!(!is_pass_through);
    assert_eq!(score, 0.0);
}

#[test]
fn test_check_direct_illicit_interaction() {
    // Setup
    let now = OffsetDateTime::now_utc();

    // Create a transaction with normal programs/wallets
    let mut normal_tx = TransactionAnalysisData::new(ParsedTransaction {
        signature: "normal_tx".to_string(),
        program_ids: vec!["program1".to_string(), "program2".to_string()],
        involved_accounts: vec!["address1".to_string(), "address2".to_string()],
        pre_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        post_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        execution_status: Some("confirmed".to_string()),
        fee: 5000,
    });

    // Add program results with low risk
    normal_tx.program_analysis.push(ProgramAnalysis {
        program_id: "program1".to_string(),
        is_known: true,
        address_record: Some(AddressRecord {
            address: "program1".to_string(),
            entity_name: "Safe DEX".to_string(),
            category: "DEX Router".to_string(),
            risk_level: "Low".to_string(),
            source_of_info: "Official Docs".to_string(),
            confidence_score: 5,
            notes: None,
            created_at: now,
            updated_at: now,
        }),
    });

    // Add wallet results with low risk
    normal_tx
        .wallet_direct_analysis
        .push(WalletDirectAnalysisResult {
            wallet_address: "address1".to_string(),
            is_known: true,
            address_record: Some(AddressRecord {
                address: "address1".to_string(),
                entity_name: "Known Exchange".to_string(),
                category: "Exchange".to_string(),
                risk_level: "Low".to_string(),
                source_of_info: "Official Docs".to_string(),
                confidence_score: 5,
                notes: None,
                created_at: now,
                updated_at: now,
            }),
        });

    // Should not detect illicit interaction
    assert!(!check_direct_illicit_interaction(&normal_tx));

    // Create a transaction with high risk program
    let mut high_risk_program_tx = TransactionAnalysisData::new(ParsedTransaction {
        signature: "high_risk_program_tx".to_string(),
        program_ids: vec!["program1".to_string(), "high_risk_program".to_string()],
        involved_accounts: vec!["address1".to_string(), "address2".to_string()],
        pre_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        post_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        execution_status: Some("confirmed".to_string()),
        fee: 5000,
    });

    // Add program results with high risk
    high_risk_program_tx.program_analysis.push(ProgramAnalysis {
        program_id: "high_risk_program".to_string(),
        is_known: true,
        address_record: Some(AddressRecord {
            address: "high_risk_program".to_string(),
            entity_name: "Suspicious Mixer".to_string(),
            category: "Privacy Service".to_string(),
            risk_level: "High".to_string(),
            source_of_info: "OSINT Report".to_string(),
            confidence_score: 4,
            notes: None,
            created_at: now,
            updated_at: now,
        }),
    });

    // Should detect illicit interaction
    assert!(check_direct_illicit_interaction(&high_risk_program_tx));

    // Create a transaction with high risk wallet
    let mut high_risk_wallet_tx = TransactionAnalysisData::new(ParsedTransaction {
        signature: "high_risk_wallet_tx".to_string(),
        program_ids: vec!["program1".to_string(), "program2".to_string()],
        involved_accounts: vec!["address1".to_string(), "high_risk_wallet".to_string()],
        pre_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        post_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        execution_status: Some("confirmed".to_string()),
        fee: 5000,
    });

    // Add wallet results with critical risk
    high_risk_wallet_tx
        .wallet_direct_analysis
        .push(WalletDirectAnalysisResult {
            wallet_address: "high_risk_wallet".to_string(),
            is_known: true,
            address_record: Some(AddressRecord {
                address: "high_risk_wallet".to_string(),
                entity_name: "Known Hacker X Wallet 1".to_string(),
                category: "Known Hacker".to_string(),
                risk_level: "Critical".to_string(),
                source_of_info: "OSINT Report".to_string(),
                confidence_score: 5,
                notes: None,
                created_at: now,
                updated_at: now,
            }),
        });

    // Should detect illicit interaction
    assert!(check_direct_illicit_interaction(&high_risk_wallet_tx));
}

#[test]
fn test_check_interaction_with_risky_categories() {
    // Setup
    let now = OffsetDateTime::now_utc();

    // Create a transaction with normal programs/wallets
    let mut normal_tx = TransactionAnalysisData::new(ParsedTransaction {
        signature: "normal_tx".to_string(),
        program_ids: vec!["program1".to_string(), "program2".to_string()],
        involved_accounts: vec!["address1".to_string(), "address2".to_string()],
        pre_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        post_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        execution_status: Some("confirmed".to_string()),
        fee: 5000,
    });

    // Add program results with low risk category
    normal_tx.program_analysis.push(ProgramAnalysis {
        program_id: "program1".to_string(),
        is_known: true,
        address_record: Some(AddressRecord {
            address: "program1".to_string(),
            entity_name: "Safe DEX".to_string(),
            category: "DEX Router".to_string(),
            risk_level: "Low".to_string(),
            source_of_info: "Official Docs".to_string(),
            confidence_score: 5,
            notes: None,
            created_at: now,
            updated_at: now,
        }),
    });

    // Should return 0.0 score (no risky categories)
    assert_eq!(check_interaction_with_risky_categories(&normal_tx), 0.0);

    // Create a transaction with high risk category program
    let mut high_risk_category_tx = TransactionAnalysisData::new(ParsedTransaction {
        signature: "high_risk_category_tx".to_string(),
        program_ids: vec!["program1".to_string(), "risky_program".to_string()],
        involved_accounts: vec!["address1".to_string(), "address2".to_string()],
        pre_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        post_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        execution_status: Some("confirmed".to_string()),
        fee: 5000,
    });

    // Add program with high risk category
    high_risk_category_tx
        .program_analysis
        .push(ProgramAnalysis {
            program_id: "risky_program".to_string(),
            is_known: true,
            address_record: Some(AddressRecord {
                address: "risky_program".to_string(),
                entity_name: "Suspicious Service".to_string(),
                category: "Privacy Service".to_string(), // High risk category
                risk_level: "Medium".to_string(),
                source_of_info: "OSINT Report".to_string(),
                confidence_score: 4,
                notes: None,
                created_at: now,
                updated_at: now,
            }),
        });

    // Should return a significant score (> 0.4)
    let high_risk_score = check_interaction_with_risky_categories(&high_risk_category_tx);
    assert!(
        high_risk_score > 0.4,
        "High risk category score: {}",
        high_risk_score
    );

    // Create a transaction with multiple risky categories
    let mut multiple_risks_tx = TransactionAnalysisData::new(ParsedTransaction {
        signature: "multiple_risks_tx".to_string(),
        program_ids: vec!["program1".to_string(), "risky_program".to_string()],
        involved_accounts: vec!["address1".to_string(), "risky_address".to_string()],
        pre_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        post_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        execution_status: Some("confirmed".to_string()),
        fee: 5000,
    });

    // Add program with high risk category
    multiple_risks_tx.program_analysis.push(ProgramAnalysis {
        program_id: "risky_program".to_string(),
        is_known: true,
        address_record: Some(AddressRecord {
            address: "risky_program".to_string(),
            entity_name: "Suspicious Service".to_string(),
            category: "Privacy Service".to_string(), // High risk category
            risk_level: "Medium".to_string(),
            source_of_info: "OSINT Report".to_string(),
            confidence_score: 4,
            notes: None,
            created_at: now,
            updated_at: now,
        }),
    });

    // Add wallet with medium risk category
    multiple_risks_tx
        .wallet_direct_analysis
        .push(WalletDirectAnalysisResult {
            wallet_address: "risky_address".to_string(),
            is_known: true,
            address_record: Some(AddressRecord {
                address: "risky_address".to_string(),
                entity_name: "Gambling Service".to_string(),
                category: "Gambling DApp".to_string(), // Medium risk category
                risk_level: "Medium".to_string(),
                source_of_info: "OSINT Report".to_string(),
                confidence_score: 3,
                notes: None,
                created_at: now,
                updated_at: now,
            }),
        });

    // Should return a high score (> 0.6)
    let multiple_risks_score = check_interaction_with_risky_categories(&multiple_risks_tx);
    assert!(
        multiple_risks_score > 0.6,
        "Multiple risks score: {}",
        multiple_risks_score
    );
    // Score should be capped at 1.0
    assert!(
        multiple_risks_score <= 1.0,
        "Score exceeds 1.0: {}",
        multiple_risks_score
    );
}
