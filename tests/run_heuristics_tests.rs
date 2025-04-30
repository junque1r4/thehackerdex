use hackerdex::analysis::program_analyzer::ProgramAnalysis;
use hackerdex::analysis::transaction_analysis::TransactionAnalysisData;
use hackerdex::analysis::transaction_parser::ParsedTransaction;
use hackerdex::analysis::wallet_analyzer::WalletDirectAnalysisResult;
use hackerdex::db::models::AddressRecord;
use hackerdex::heuristic_engine::{WalletContext, WalletTransaction, run_all_heuristics};
use solana_transaction_status::UiTransactionTokenBalance;
use sqlx::types::time::OffsetDateTime;
use std::time::{SystemTime, UNIX_EPOCH};

// Helper function to create transaction analysis data with specific properties
fn create_transaction_analysis(
    signature: &str,
    involved_addresses: Vec<&str>,
    programs: Vec<&str>,
    include_risky_wallet: bool,
    include_risky_program: bool,
) -> TransactionAnalysisData {
    // Create the ParsedTransaction
    let parsed_transaction = ParsedTransaction {
        signature: signature.to_string(),
        program_ids: programs.iter().map(|&p| p.to_string()).collect(),
        involved_accounts: involved_addresses.iter().map(|&a| a.to_string()).collect(),
        pre_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        post_token_balances: Vec::<UiTransactionTokenBalance>::new(),
        execution_status: Some("Success".to_string()),
        fee: 5000,
    };

    let mut analysis_data = TransactionAnalysisData::new(parsed_transaction);

    // Add wallet analysis
    for address in &involved_addresses {
        let risk_level = if *address == "risky_wallet" && include_risky_wallet {
            "High".to_string()
        } else {
            "Low".to_string()
        };

        let category = if *address == "risky_wallet" && include_risky_wallet {
            "HighRiskService".to_string()
        } else {
            "Exchange".to_string()
        };

        // Create a current timestamp for the record
        let now = OffsetDateTime::now_utc();

        analysis_data
            .wallet_direct_analysis
            .push(WalletDirectAnalysisResult {
                wallet_address: address.to_string(),
                address_record: Some(AddressRecord {
                    address: address.to_string(),
                    entity_name: format!("Entity {}", address),
                    category,
                    risk_level,
                    source_of_info: "Test Data".to_string(),
                    confidence_score: 5,
                    notes: None,
                    created_at: now,
                    updated_at: now,
                }),
                is_known: true,
            });
    }

    // Add program analysis
    for program in &programs {
        let risk_level = if *program == "risky_program" && include_risky_program {
            "Critical".to_string()
        } else {
            "Low".to_string()
        };

        let category = if *program == "risky_program" && include_risky_program {
            "MaliciousContract".to_string()
        } else {
            "DeFiProtocol".to_string()
        };

        // Create a current timestamp for the record
        let now = OffsetDateTime::now_utc();

        analysis_data.program_analysis.push(ProgramAnalysis {
            program_id: program.to_string(),
            address_record: Some(AddressRecord {
                address: program.to_string(),
                entity_name: format!("Program {}", program),
                category,
                risk_level,
                source_of_info: "Test Data".to_string(),
                confidence_score: 5,
                notes: None,
                created_at: now,
                updated_at: now,
            }),
            is_known: true,
        });
    }

    analysis_data
}

// Helper function to create a wallet context for testing
fn create_wallet_context(
    address: &str,
    creation_timestamp: Option<i64>,
    tx_count_24h: u32,
    is_pass_through: bool,
    is_high_risk: bool,
) -> WalletContext {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Create a current timestamp for the record
    let db_now = OffsetDateTime::now_utc();

    let mut wallet_ctx = WalletContext {
        address: address.to_string(),
        creation_timestamp,
        sol_balance: 10.0,
        known_address_record: if is_high_risk {
            Some(AddressRecord {
                address: address.to_string(),
                entity_name: "High Risk Entity".to_string(),
                category: "HighRiskService".to_string(),
                risk_level: "High".to_string(),
                source_of_info: "Test".to_string(),
                confidence_score: 4,
                notes: None,
                created_at: db_now,
                updated_at: db_now,
            })
        } else {
            None
        },
        tx_count_24h,
        tx_count_7d: tx_count_24h + 20,
        volume_24h: 100.0,
        volume_7d: 500.0,
        recent_incoming_txs: Vec::new(),
        recent_outgoing_txs: Vec::new(),
    };

    // If we want to simulate a pass-through wallet
    if is_pass_through {
        // Add recent incoming transactions (received funds)
        wallet_ctx.recent_incoming_txs = vec![
            WalletTransaction {
                signature: "sig_in_1".to_string(),
                timestamp: now - 600, // 10 minutes ago
                amount: 5.0,
                counterparty: "sender1".to_string(),
                is_known_counterparty: false,
                counterparty_record: None,
            },
            WalletTransaction {
                signature: "sig_in_2".to_string(),
                timestamp: now - 300, // 5 minutes ago
                amount: 3.0,
                counterparty: "sender2".to_string(),
                is_known_counterparty: false,
                counterparty_record: None,
            },
        ];

        // Add recent outgoing transactions (sent funds quickly after receiving)
        wallet_ctx.recent_outgoing_txs = vec![
            WalletTransaction {
                signature: "sig_out_1".to_string(),
                timestamp: now - 590, // 9 minutes and 50 seconds ago (10 seconds after receiving)
                amount: 4.9,          // Almost all of the received amount
                counterparty: "receiver1".to_string(),
                is_known_counterparty: false,
                counterparty_record: None,
            },
            WalletTransaction {
                signature: "sig_out_2".to_string(),
                timestamp: now - 290, // 4 minutes and 50 seconds ago (10 seconds after receiving)
                amount: 2.9,          // Almost all of the received amount
                counterparty: "receiver2".to_string(),
                is_known_counterparty: false,
                counterparty_record: None,
            },
        ];
    }

    wallet_ctx
}

// Direct unit test for run_all_heuristics function
#[test]
fn test_run_all_heuristics() {
    // Create a wallet context with specific characteristics
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let wallet_context = create_wallet_context(
        "test_wallet",
        Some(now - 86400), // 1 day old (new wallet)
        50,                // High transaction count
        true,              // Is pass-through
        false,             // Not high risk
    );

    // Create transaction analysis with a high-risk wallet
    let analysis_data = create_transaction_analysis(
        "test_run_all_heuristics",
        vec!["test_wallet", "risky_wallet"],
        vec!["program789"],
        true,  // Include risky wallet
        false, // No risky program
    );

    // Run all heuristics
    let flags = run_all_heuristics(&analysis_data, &wallet_context);

    // Verify individual heuristic flags are set as expected
    assert!(flags.is_new_wallet, "New wallet flag should be true");
    assert!(
        flags.is_high_frequency,
        "High frequency flag should be true"
    );
    assert!(flags.is_pass_through, "Pass-through flag should be true");
    assert!(
        flags.direct_illicit_interaction,
        "Direct illicit interaction flag should be true"
    );

    // Check that the overall suspicion score reflects multiple risk factors
    assert!(
        flags.get_overall_suspicion_score() > 3.0,
        "Overall suspicion score should be high due to multiple risk factors"
    );

    // Verify the triggered flags description includes all the expected flags
    let descriptions = flags.get_triggered_flags_description();
    assert!(
        descriptions
            .iter()
            .any(|d| d.contains("Newly created wallet"))
    );
    assert!(descriptions.iter().any(|d| d.contains("High frequency")));
    assert!(
        descriptions
            .iter()
            .any(|d| d.contains("Pass-through wallet"))
    );
    assert!(
        descriptions
            .iter()
            .any(|d| d.contains("Direct interaction with known illicit address"))
    );
}
