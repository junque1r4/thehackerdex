use hackerdex::{
    db::models::AddressRecord,
    heuristic_engine::{
        WalletContext, WalletTransaction,
        wallet_heuristics::{
            analyze_funding_sources, analyze_spending_destinations, check_wallet_age_and_activity,
        },
    },
};
use sqlx::types::time::OffsetDateTime;

/// Creates a test wallet context with configurable parameters
fn create_test_wallet_context(
    is_new: bool,
    high_volume: bool,
    high_tx_count: bool,
    incoming_txs: Vec<WalletTransaction>,
    outgoing_txs: Vec<WalletTransaction>,
) -> WalletContext {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Creation time is 60 days ago if not new, 15 days ago if new
    let creation_timestamp = if is_new {
        now - (15 * 24 * 60 * 60)
    } else {
        now - (60 * 24 * 60 * 60)
    };

    let tx_count_24h = if high_tx_count { 30 } else { 5 };
    let tx_count_7d = if high_tx_count { 80 } else { 15 };

    let volume_24h = if high_volume { 20000.0 } else { 500.0 };
    let volume_7d = if high_volume { 50000.0 } else { 1500.0 };

    WalletContext {
        address: "TestWalletAddr123".to_string(),
        creation_timestamp: Some(creation_timestamp),
        sol_balance: 1000.0,
        known_address_record: None,
        tx_count_24h,
        tx_count_7d,
        volume_24h,
        volume_7d,
        recent_incoming_txs: incoming_txs,
        recent_outgoing_txs: outgoing_txs,
    }
}

/// Creates a test transaction for use in wallet context
fn create_test_transaction(
    amount: f64,
    timestamp: i64,
    counterparty: &str,
    is_known: bool,
    risk_level: Option<&str>,
    category: Option<&str>,
) -> WalletTransaction {
    let counterparty_record = if is_known {
        let risk = risk_level.unwrap_or("Low");
        let cat = category.unwrap_or("Exchange");
        let now = OffsetDateTime::now_utc();

        Some(AddressRecord {
            address: counterparty.to_string(),
            entity_name: format!("Entity_{}", counterparty),
            category: cat.to_string(),
            risk_level: risk.to_string(),
            source_of_info: "Test".to_string(),
            confidence_score: 5,
            notes: Some("Test record".to_string()),
            created_at: now,
            updated_at: now,
        })
    } else {
        None
    };

    WalletTransaction {
        signature: format!("sig_{}", counterparty),
        timestamp,
        amount,
        counterparty: counterparty.to_string(),
        is_known_counterparty: is_known,
        counterparty_record,
    }
}

#[test]
fn test_check_wallet_age_and_activity() {
    // Case 1: New wallet with high activity
    let wallet_context = create_test_wallet_context(true, true, true, vec![], vec![]);
    let (is_suspicious, score) = check_wallet_age_and_activity(&wallet_context);

    assert!(
        is_suspicious,
        "New wallet with high activity should be flagged as suspicious"
    );
    assert!(
        score > 0.5,
        "New wallet with high activity should have a high suspicion score"
    );

    // Case 2: New wallet with low activity
    let wallet_context = create_test_wallet_context(true, false, false, vec![], vec![]);
    let (is_suspicious, score) = check_wallet_age_and_activity(&wallet_context);

    assert!(
        !is_suspicious,
        "New wallet with low activity should not be flagged as suspicious"
    );
    assert!(
        score <= 0.3,
        "New wallet with low activity should have a low suspicion score"
    );

    // Case 3: Old wallet
    let wallet_context = create_test_wallet_context(false, true, true, vec![], vec![]);
    let (is_suspicious, score) = check_wallet_age_and_activity(&wallet_context);

    assert!(
        !is_suspicious,
        "Old wallet should not be flagged as suspicious regardless of activity"
    );
    assert_eq!(
        score, 0.0,
        "Old wallet should have zero suspicion score for age check"
    );
}

#[test]
fn test_analyze_funding_sources() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Case 1: Wallet with risky funding sources
    let incoming_txs = vec![
        // Critical risk source (100% weight)
        create_test_transaction(
            1000.0,
            now - 3600,
            "Source1",
            true,
            Some("Critical"),
            Some("Known Hacker"),
        ),
        // High risk source (100% weight)
        create_test_transaction(
            1000.0,
            now - 7200,
            "Source2",
            true,
            Some("High"),
            Some("Mixer Feeder Address"),
        ),
        // Medium risk source (50% weight)
        create_test_transaction(
            1000.0,
            now - 10800,
            "Source3",
            true,
            Some("Medium"),
            Some("High-Risk Gambling DApp"),
        ),
        // Low risk source (0% weight)
        create_test_transaction(
            1000.0,
            now - 14400,
            "Source4",
            true,
            Some("Low"),
            Some("Exchange"),
        ),
    ];

    let wallet_context = create_test_wallet_context(false, false, false, incoming_txs, vec![]);
    let risk_ratio = analyze_funding_sources(&wallet_context);

    // Expected: (1000 + 1000 + 500) / 4000 = 0.625
    assert!(
        risk_ratio > 0.6 && risk_ratio < 0.65,
        "Risk ratio for mixed funding sources should be around 0.625 but got {}",
        risk_ratio
    );

    // Case 2: Wallet with all low-risk funding
    let incoming_txs = vec![
        create_test_transaction(
            1000.0,
            now - 3600,
            "GoodSource1",
            true,
            Some("Low"),
            Some("DEX"),
        ),
        create_test_transaction(
            1000.0,
            now - 7200,
            "GoodSource2",
            true,
            Some("Low"),
            Some("Bridge"),
        ),
    ];

    let wallet_context = create_test_wallet_context(false, false, false, incoming_txs, vec![]);
    let risk_ratio = analyze_funding_sources(&wallet_context);

    assert_eq!(
        risk_ratio, 0.0,
        "Risk ratio for all low-risk sources should be 0.0"
    );

    // Case 3: Empty transactions
    let wallet_context = create_test_wallet_context(false, false, false, vec![], vec![]);
    let risk_ratio = analyze_funding_sources(&wallet_context);

    assert_eq!(
        risk_ratio, 0.0,
        "Risk ratio for no transactions should be 0.0"
    );
}

#[test]
fn test_analyze_spending_destinations() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Case 1: Wallet with risky spending destinations
    let outgoing_txs = vec![
        // Critical risk destination (100% weight)
        create_test_transaction(
            1000.0,
            now - 3600,
            "Dest1",
            true,
            Some("Critical"),
            Some("Mixer"),
        ),
        // High risk destination (100% weight)
        create_test_transaction(
            1000.0,
            now - 7200,
            "Dest2",
            true,
            Some("High"),
            Some("Sanctioned Entity"),
        ),
        // Medium risk destination (50% weight)
        create_test_transaction(
            1000.0,
            now - 10800,
            "Dest3",
            true,
            Some("Medium"),
            Some("Privacy Service"),
        ),
        // Low risk destination (0% weight)
        create_test_transaction(1000.0, now - 14400, "Dest4", true, Some("Low"), Some("DEX")),
    ];

    let wallet_context = create_test_wallet_context(false, false, false, vec![], outgoing_txs);
    let risk_ratio = analyze_spending_destinations(&wallet_context);

    // Expected: (1000 + 1000 + 500) / 4000 = 0.625
    assert!(
        risk_ratio > 0.6 && risk_ratio < 0.65,
        "Risk ratio for mixed destinations should be around 0.625 but got {}",
        risk_ratio
    );

    // Case 2: Wallet with all low-risk destinations
    let outgoing_txs = vec![
        create_test_transaction(
            1000.0,
            now - 3600,
            "SafeDest1",
            true,
            Some("Low"),
            Some("Exchange"),
        ),
        create_test_transaction(
            1000.0,
            now - 7200,
            "SafeDest2",
            true,
            Some("Low"),
            Some("DEX"),
        ),
    ];

    let wallet_context = create_test_wallet_context(false, false, false, vec![], outgoing_txs);
    let risk_ratio = analyze_spending_destinations(&wallet_context);

    assert_eq!(
        risk_ratio, 0.0,
        "Risk ratio for all low-risk destinations should be 0.0"
    );

    // Case 3: Wallet with fund dispersal pattern
    let mut dispersal_txs = Vec::new();
    for i in 0..10 {
        let is_risky = i < 3; // 3 out of 10 destinations are risky
        let risk_level = if is_risky { Some("High") } else { Some("Low") };
        let category = if is_risky {
            Some("Privacy Service")
        } else {
            Some("Exchange")
        };

        dispersal_txs.push(create_test_transaction(
            100.0,
            now - (i * 3600) as i64,
            &format!("MultiDest{}", i),
            true,
            risk_level,
            category,
        ));
    }

    let wallet_context = create_test_wallet_context(false, false, false, vec![], dispersal_txs);
    let risk_ratio = analyze_spending_destinations(&wallet_context);

    // Base ratio: 300 / 1000 = 0.3, plus some extra for dispersal pattern
    assert!(
        risk_ratio > 0.3,
        "Risk ratio for dispersal pattern should be higher than base ratio"
    );

    // Case 4: Empty transactions
    let wallet_context = create_test_wallet_context(false, false, false, vec![], vec![]);
    let risk_ratio = analyze_spending_destinations(&wallet_context);

    assert_eq!(
        risk_ratio, 0.0,
        "Risk ratio for no transactions should be 0.0"
    );
}
