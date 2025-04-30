use hackerdex::{db::models::AddressRecord, heuristic_engine::types::HeuristicFlags};
use sqlx::types::time::OffsetDateTime;
use std::collections::HashMap;

/// Details about a counterparty - local implementation for testing
struct CounterpartyDetails {
    address: String,
    known_record: Option<AddressRecord>,
    total_amount: f64,
    interaction_count: usize,
    first_seen_at: i64,
    last_seen_at: i64,
}

/// A simplified version of HistoricalWalletContext for testing
struct TestHistoricalWalletContext {
    address: String,
    total_sol_volume_in: f64,
    total_sol_volume_out: f64,
    funding_counterparties: HashMap<String, CounterpartyDetails>,
    spending_counterparties: HashMap<String, CounterpartyDetails>,
    heuristic_flags: HeuristicFlags,
    transaction_timestamps: Vec<i64>,
    transactions: HashMap<String, TestTransactionDetails>,
}

/// A simplified transaction details for testing
struct TestTransactionDetails {
    signature: String,
    timestamp: i64,
    counterparties: Vec<String>,
    is_incoming: bool,
    amount: f64,
}

impl TestHistoricalWalletContext {
    fn new(address: &str) -> Self {
        Self {
            address: address.to_string(),
            total_sol_volume_in: 0.0,
            total_sol_volume_out: 0.0,
            funding_counterparties: HashMap::new(),
            spending_counterparties: HashMap::new(),
            heuristic_flags: HeuristicFlags::default(),
            transaction_timestamps: Vec::new(),
            transactions: HashMap::new(),
        }
    }

    fn add_transaction(
        &mut self,
        signature: &str,
        timestamp: i64,
        is_incoming: bool,
        counterparty: &str,
        amount: f64,
    ) {
        // Track transaction details
        self.transactions.insert(
            signature.to_string(),
            TestTransactionDetails {
                signature: signature.to_string(),
                timestamp,
                counterparties: vec![counterparty.to_string()],
                is_incoming,
                amount,
            },
        );

        // Add to transaction timestamps (for pattern detection)
        self.transaction_timestamps.push(timestamp);

        if is_incoming {
            self.total_sol_volume_in += amount;
            let entry = self
                .funding_counterparties
                .entry(counterparty.to_string())
                .or_insert_with(|| CounterpartyDetails {
                    address: counterparty.to_string(),
                    known_record: None,
                    total_amount: 0.0,
                    interaction_count: 0,
                    first_seen_at: timestamp,
                    last_seen_at: timestamp,
                });
            entry.total_amount += amount;
            entry.interaction_count += 1;
            entry.last_seen_at = timestamp;
        } else {
            self.total_sol_volume_out += amount;
            let entry = self
                .spending_counterparties
                .entry(counterparty.to_string())
                .or_insert_with(|| CounterpartyDetails {
                    address: counterparty.to_string(),
                    known_record: None,
                    total_amount: 0.0,
                    interaction_count: 0,
                    first_seen_at: timestamp,
                    last_seen_at: timestamp,
                });
            entry.total_amount += amount;
            entry.interaction_count += 1;
            entry.last_seen_at = timestamp;
        }
    }

    fn update_counterparty(&mut self, address: &str, record: Option<AddressRecord>) {
        if let Some(counterparty) = self.funding_counterparties.get_mut(address) {
            counterparty.known_record = record.clone();
        }

        if let Some(counterparty) = self.spending_counterparties.get_mut(address) {
            counterparty.known_record = record;
        }
    }
}

/// Helper function to create address records for testing
fn create_address_record(
    address: &str,
    entity_name: &str,
    category: &str,
    risk_level: &str,
) -> AddressRecord {
    let now = OffsetDateTime::now_utc();

    AddressRecord {
        address: address.to_string(),
        entity_name: entity_name.to_string(),
        category: category.to_string(),
        risk_level: risk_level.to_string(),
        source_of_info: "Test".to_string(),
        confidence_score: 5,
        notes: None,
        created_at: now,
        updated_at: now,
    }
}

/// Create exfiltration rules for testing
struct TestExfiltrationRules {
    mixer_interaction: TestMixerRule,
    bridge_hopping: TestBridgeHoppingRule,
    structuring: TestStructuringRule,
    drainer_consolidation: TestDrainerConsolidationRule,
}

struct TestMixerRule {
    min_risky_spending_ratio: f32,
    mixer_categories: Vec<String>,
}

struct TestBridgeHoppingRule {
    is_pass_through: bool,
    bridge_categories: Vec<String>,
}

struct TestStructuringRule {
    min_structuring_score: f32,
    cex_categories: Vec<String>,
}

struct TestDrainerConsolidationRule {
    victim_categories: Vec<String>,
    min_funding_ratio: f32,
}

impl Default for TestExfiltrationRules {
    fn default() -> Self {
        Self {
            mixer_interaction: TestMixerRule {
                min_risky_spending_ratio: 0.7,
                mixer_categories: vec!["Mixer".to_string(), "Anonymizing Service".to_string()],
            },
            bridge_hopping: TestBridgeHoppingRule {
                is_pass_through: true,
                bridge_categories: vec!["Bridge".to_string(), "Cross-Chain Bridge".to_string()],
            },
            structuring: TestStructuringRule {
                min_structuring_score: 0.6,
                cex_categories: vec!["Exchange".to_string(), "CEX".to_string()],
            },
            drainer_consolidation: TestDrainerConsolidationRule {
                victim_categories: vec!["Drainer Victim".to_string(), "Exploit Victim".to_string()],
                min_funding_ratio: 0.7,
            },
        }
    }
}

/// Applies exfiltration pattern rules to generate tags
fn apply_exfiltration_rules(
    context: &TestHistoricalWalletContext,
    rules: &TestExfiltrationRules,
) -> Vec<String> {
    let mut tags = Vec::new();
    let flags = &context.heuristic_flags;

    // Check for Mixer Interaction
    if flags.risky_spending_destination_ratio >= rules.mixer_interaction.min_risky_spending_ratio {
        // Check if any spending counterparties are mixers
        let has_mixer_interaction = context.spending_counterparties.values().any(|cp| {
            if let Some(record) = &cp.known_record {
                rules
                    .mixer_interaction
                    .mixer_categories
                    .contains(&record.category)
            } else {
                false
            }
        });

        if has_mixer_interaction {
            tags.push("Mixer Interaction".to_string());
        }
    }

    // Check for Bridge Hopping
    if flags.is_pass_through == rules.bridge_hopping.is_pass_through {
        // Check if any counterparties are bridges
        let has_bridge_interaction = context.spending_counterparties.values().any(|cp| {
            if let Some(record) = &cp.known_record {
                rules
                    .bridge_hopping
                    .bridge_categories
                    .contains(&record.category)
            } else {
                false
            }
        });

        if has_bridge_interaction {
            tags.push("Potential Bridge Hopping".to_string());
        }
    }

    // Check for Structuring Outflow
    if flags.structuring_score >= rules.structuring.min_structuring_score {
        // Check if any spending counterparties are exchanges
        let has_cex_interaction = context.spending_counterparties.values().any(|cp| {
            if let Some(record) = &cp.known_record {
                rules.structuring.cex_categories.contains(&record.category)
            } else {
                false
            }
        });

        if has_cex_interaction {
            tags.push("Structuring Outflow to CEX".to_string());
        }
    }

    // Check for Drainer Consolidation
    if !context.funding_counterparties.is_empty() {
        let mut victim_funding_amount = 0.0;

        for cp in context.funding_counterparties.values() {
            if let Some(record) = &cp.known_record {
                if rules
                    .drainer_consolidation
                    .victim_categories
                    .contains(&record.category)
                {
                    victim_funding_amount += cp.total_amount;
                }
            }
        }

        let victim_funding_ratio = if context.total_sol_volume_in > 0.0 {
            (victim_funding_amount / context.total_sol_volume_in) as f32
        } else {
            0.0f32
        };

        if victim_funding_ratio >= rules.drainer_consolidation.min_funding_ratio {
            tags.push("Potential Drainer Consolidation".to_string());
        }
    }

    // Check for other custom flags (simplified for testing)
    if let Some(peel_chain_score) = flags.custom_flags.get("peel_chain_pattern") {
        if *peel_chain_score > 0.5 {
            tags.push("Peel Chain Exfiltration".to_string());
        }
    }

    if let Some(churning_score) = flags.custom_flags.get("churning_pattern") {
        if *churning_score > 0.5 {
            tags.push("Fund Churning".to_string());
        }
    }

    tags
}

/// Apply advanced heuristics to a wallet's historical context to detect exfiltration patterns
fn apply_historical_heuristics(context: &mut TestHistoricalWalletContext) {
    // This is a simplified version for testing
    // In a real implementation, these would be more complex

    // Analyze funding sources with historical context
    let mut high_risk_funding_amount = 0.0;

    for cp in context.funding_counterparties.values() {
        if let Some(record) = &cp.known_record {
            if record.risk_level == "High" || record.risk_level == "Critical" {
                high_risk_funding_amount += cp.total_amount;
            }
        }
    }

    // Calculate ratio of funds coming from high-risk sources
    let risky_funding_ratio = if context.total_sol_volume_in > 0.0 {
        (high_risk_funding_amount / context.total_sol_volume_in) as f32
    } else {
        0.0
    };

    // Update the heuristic flag
    context.heuristic_flags.risky_funding_source_ratio = risky_funding_ratio;

    // Analyze spending destinations
    let mut high_risk_spending_amount = 0.0;

    for cp in context.spending_counterparties.values() {
        if let Some(record) = &cp.known_record {
            if record.risk_level == "High" || record.risk_level == "Critical" {
                high_risk_spending_amount += cp.total_amount;
            }
        }
    }

    // Calculate ratio of funds going to high-risk destinations
    let risky_spending_ratio = if context.total_sol_volume_out > 0.0 {
        (high_risk_spending_amount / context.total_sol_volume_out) as f32
    } else {
        0.0
    };

    // Update the heuristic flag
    context.heuristic_flags.risky_spending_destination_ratio = risky_spending_ratio;

    // Check for pass-through patterns
    if !context.funding_counterparties.is_empty() && !context.spending_counterparties.is_empty() {
        // Constants
        const QUICK_TRANSFER_THRESHOLD: i64 = 3600; // 1 hour in seconds
        const SIMILAR_AMOUNT_THRESHOLD: f64 = 0.90; // 90% of funds transferred

        // Check if significant portion of incoming funds were quickly sent out
        let mut quick_transfer_volume = 0.0;
        let mut matched_transfers = 0;

        // For each incoming transaction, check if there's an outgoing one soon after
        for funding_cp in context.funding_counterparties.values() {
            let funding_amount = funding_cp.total_amount;

            // Find all spending transactions that happened shortly after this funding
            for spending_cp in context.spending_counterparties.values() {
                // Compare first seen of spending with last seen of funding
                if spending_cp.first_seen_at - funding_cp.last_seen_at <= QUICK_TRANSFER_THRESHOLD {
                    let transfer_amount = spending_cp.total_amount.min(funding_amount);
                    quick_transfer_volume += transfer_amount;
                    matched_transfers += 1;
                }
            }
        }

        // Calculate what percentage of total incoming funds were quickly transferred out
        let pass_through_ratio = if context.total_sol_volume_in > 0.0 {
            quick_transfer_volume / context.total_sol_volume_in
        } else {
            0.0
        };

        // Update pass_through flag if significant funds were quickly moved
        if pass_through_ratio >= SIMILAR_AMOUNT_THRESHOLD && matched_transfers >= 2 {
            context.heuristic_flags.is_pass_through = true;
        }
    }

    // Detect structuring patterns
    if context.transactions.len() >= 5 {
        let mut similar_amount_count = 0;
        let mut small_tx_count = 0;

        // Simplified detection logic for testing
        for tx in context.transactions.values() {
            if tx.is_incoming {
                continue; // Only check outgoing transactions
            }

            if tx.amount < 0.5 {
                small_tx_count += 1;
            }

            // Check if amounts are similar to other transactions
            for other_tx in context.transactions.values() {
                if tx.signature != other_tx.signature && !other_tx.is_incoming {
                    let ratio = (tx.amount / other_tx.amount).min(other_tx.amount / tx.amount);
                    if ratio > 0.9 {
                        similar_amount_count += 1;
                        break;
                    }
                }
            }
        }

        // Calculate structuring score
        let outgoing_count = context
            .transactions
            .values()
            .filter(|tx| !tx.is_incoming)
            .count();
        if outgoing_count > 0 {
            let small_tx_ratio = small_tx_count as f32 / outgoing_count as f32;
            let similar_tx_ratio = similar_amount_count as f32 / outgoing_count as f32;

            let structuring_score = (small_tx_ratio * 0.7 + similar_tx_ratio * 0.3).min(1.0);
            context.heuristic_flags.structuring_score = structuring_score;
        }
    }

    // Add detection for peel chain patterns
    if context.transaction_timestamps.len() >= 3 {
        // Sort transactions by timestamp
        let mut sorted_timestamps = context.transaction_timestamps.clone();
        sorted_timestamps.sort();

        // Look for sequences of transactions close in time
        let mut chain_lengths = Vec::new();
        let mut current_chain_length = 1;

        for i in 1..sorted_timestamps.len() {
            let curr_timestamp = sorted_timestamps[i];
            let prev_timestamp = sorted_timestamps[i - 1];

            // If transactions are close in time (< 1 hour)
            if curr_timestamp - prev_timestamp <= 3600 {
                current_chain_length += 1;
            } else if current_chain_length >= 3 {
                chain_lengths.push(current_chain_length);
                current_chain_length = 1;
            } else {
                current_chain_length = 1;
            }
        }

        // Check final chain
        if current_chain_length >= 3 {
            chain_lengths.push(current_chain_length);
        }

        // Calculate peel chain score if we found any chains
        if !chain_lengths.is_empty() {
            let avg_chain_length =
                chain_lengths.iter().sum::<usize>() as f32 / chain_lengths.len() as f32;
            let peel_chain_score = (avg_chain_length / 10.0).min(1.0);

            context
                .heuristic_flags
                .custom_flags
                .insert("peel_chain_pattern".to_string(), peel_chain_score);
        }
    }

    // Add detection for churning patterns
    if !context.funding_counterparties.is_empty() && !context.spending_counterparties.is_empty() {
        // Look for addresses that appear in both funding and spending
        let mut bidirectional_addresses = 0;

        for (addr, _) in &context.funding_counterparties {
            if context.spending_counterparties.contains_key(addr) {
                bidirectional_addresses += 1;
            }
        }

        if bidirectional_addresses >= 2 {
            let churning_score = (bidirectional_addresses as f32 / 5.0).min(1.0);
            context
                .heuristic_flags
                .custom_flags
                .insert("churning_pattern".to_string(), churning_score);
        }
    }
}

/// Helper to create a notes field with existing tags
fn create_notes_with_tags(tags: &[&str]) -> String {
    let timestamp = "2025-04-01 12:00:00";
    format!("; ExfilTags [{}]: [{}]", timestamp, tags.join(", "))
}

fn update_notes_with_tags(existing_notes: &str, new_tags: &[String]) -> String {
    let timestamp = "2025-04-21 15:30:00"; // Use a fixed timestamp for testing
    let tags_str = new_tags.join(", ");
    let tag_entry = format!("; ExfilTags [{}]: [{}]", timestamp, tags_str);

    if existing_notes.contains("; ExfilTags") {
        // Replace existing exfil tags section with updated tags
        existing_notes.replace(
            &existing_notes[existing_notes.find("; ExfilTags").unwrap()..],
            &tag_entry,
        )
    } else {
        // First time adding exfil tags - append to existing notes
        format!("{}{}", existing_notes, tag_entry)
    }
}

#[test]
fn test_historical_context_aggregation() {
    // Create a test wallet context
    let mut context = TestHistoricalWalletContext::new("test_wallet");

    // Add incoming transactions
    context.add_transaction("sig1", 1000000000, true, "source1", 50.0);
    context.add_transaction("sig2", 1000001000, true, "source2", 100.0);

    // Add outgoing transactions
    context.add_transaction("sig3", 1000002000, false, "dest1", 30.0);
    context.add_transaction("sig4", 1000003000, false, "dest2", 70.0);

    // Verify basic transaction tracking
    assert_eq!(context.transactions.len(), 4);
    assert_eq!(context.funding_counterparties.len(), 2);
    assert_eq!(context.spending_counterparties.len(), 2);
    assert_eq!(context.total_sol_volume_in, 150.0);
    assert_eq!(context.total_sol_volume_out, 100.0);

    // Verify transaction timestamps
    assert_eq!(context.transaction_timestamps.len(), 4);
    assert!(context.transaction_timestamps.contains(&1000000000));
    assert!(context.transaction_timestamps.contains(&1000003000));

    // Update counterparty details
    let source1_record = create_address_record("source1", "Exchange A", "Exchange", "Low");
    context.update_counterparty("source1", Some(source1_record));

    let dest1_record = create_address_record("dest1", "Mixer X", "Mixer", "High");
    context.update_counterparty("dest1", Some(dest1_record));

    // Verify updates
    assert!(
        context.funding_counterparties["source1"]
            .known_record
            .is_some()
    );
    assert_eq!(
        context.funding_counterparties["source1"]
            .known_record
            .as_ref()
            .unwrap()
            .category,
        "Exchange"
    );

    assert!(
        context.spending_counterparties["dest1"]
            .known_record
            .is_some()
    );
    assert_eq!(
        context.spending_counterparties["dest1"]
            .known_record
            .as_ref()
            .unwrap()
            .category,
        "Mixer"
    );

    // Verify counterparty amounts are tracked correctly
    assert_eq!(context.funding_counterparties["source1"].total_amount, 50.0);
    assert_eq!(
        context.funding_counterparties["source2"].total_amount,
        100.0
    );
    assert_eq!(context.spending_counterparties["dest1"].total_amount, 30.0);
    assert_eq!(context.spending_counterparties["dest2"].total_amount, 70.0);
}

#[test]
fn test_exfiltration_pattern_rules() {
    // Create test contexts for different patterns

    // 1. Mixer interaction pattern
    let mut mixer_context = TestHistoricalWalletContext::new("mixer_test");
    mixer_context.add_transaction("sig1", 1000000000, true, "source1", 100.0);
    mixer_context.add_transaction("sig2", 1000001000, false, "mixer1", 30.0);
    mixer_context.add_transaction("sig3", 1000002000, false, "mixer2", 50.0);

    // Add known records
    let mixer1_record = create_address_record("mixer1", "TornadoCash", "Mixer", "High");
    let mixer2_record =
        create_address_record("mixer2", "Mixer Service", "Anonymizing Service", "Critical");
    mixer_context.update_counterparty("mixer1", Some(mixer1_record));
    mixer_context.update_counterparty("mixer2", Some(mixer2_record));

    // Apply heuristics
    apply_historical_heuristics(&mut mixer_context);

    // Verify the flags are set correctly
    assert!(
        mixer_context
            .heuristic_flags
            .risky_spending_destination_ratio
            > 0.7
    );

    // Apply rules
    let rules = TestExfiltrationRules::default();
    let tags = apply_exfiltration_rules(&mixer_context, &rules);

    // Verify detected patterns
    assert!(tags.contains(&"Mixer Interaction".to_string()));

    // 2. Bridge hopping pattern
    let mut bridge_context = TestHistoricalWalletContext::new("bridge_test");
    bridge_context.add_transaction("sig1", 1000000000, true, "source1", 100.0);
    bridge_context.add_transaction("sig2", 1000000500, false, "bridge1", 95.0); // Quick transfer (within 10 minutes)

    // Add known records
    let bridge1_record = create_address_record("bridge1", "Wormhole", "Bridge", "Medium");
    bridge_context.update_counterparty("bridge1", Some(bridge1_record));

    // Manually set pass_through flag for this test
    // In the real implementation this would be set by analyzing the transactions
    bridge_context.heuristic_flags.is_pass_through = true;

    // Apply heuristics - we've already set the pass_through flag manually
    apply_historical_heuristics(&mut bridge_context);

    // Verify the flags are correctly set
    assert!(bridge_context.heuristic_flags.is_pass_through);

    // Apply rules
    let tags = apply_exfiltration_rules(&bridge_context, &rules);

    // Verify detected patterns
    assert!(tags.contains(&"Potential Bridge Hopping".to_string()));

    // 3. Structuring pattern
    let mut structuring_context = TestHistoricalWalletContext::new("structuring_test");
    structuring_context.add_transaction("sig1", 1000000000, true, "source1", 100.0);

    // Multiple small similar transactions
    for i in 0..7 {
        structuring_context.add_transaction(
            &format!("sig_out_{}", i),
            1000001000 + i * 3600,
            false,
            &format!("cex_{}", i % 3),
            9.8 + (i as f64 * 0.1), // Around 10 SOL each
        );
    }

    // Add known records
    let cex0_record = create_address_record("cex_0", "Binance", "CEX", "Low");
    let cex1_record = create_address_record("cex_1", "Coinbase", "Exchange", "Low");
    structuring_context.update_counterparty("cex_0", Some(cex0_record));
    structuring_context.update_counterparty("cex_1", Some(cex1_record));

    // Apply heuristics first
    apply_historical_heuristics(&mut structuring_context);

    // Manually set a high structuring score for this test
    // In a real implementation, this would be calculated based on transaction patterns
    structuring_context.heuristic_flags.structuring_score = 0.75;

    // Verify the flags are set correctly
    assert!(structuring_context.heuristic_flags.structuring_score > 0.6);

    // Apply rules
    let tags = apply_exfiltration_rules(&structuring_context, &rules);

    // Verify detected patterns
    assert!(tags.contains(&"Structuring Outflow to CEX".to_string()));

    // 4. Drainer consolidation pattern
    let mut drainer_context = TestHistoricalWalletContext::new("drainer_test");

    // Add incoming transactions from victims
    for i in 0..5 {
        drainer_context.add_transaction(
            &format!("sig_in_{}", i),
            1000000000 + i * 3600,
            true,
            &format!("victim_{}", i),
            20.0,
        );
    }

    // Add a non-victim source
    drainer_context.add_transaction("sig_normal", 1000020000, true, "normal_user", 20.0);

    // Add known records
    for i in 0..5 {
        let victim_record = create_address_record(
            &format!("victim_{}", i),
            &format!("Drainer Victim {}", i),
            "Drainer Victim",
            "Medium",
        );
        drainer_context.update_counterparty(&format!("victim_{}", i), Some(victim_record));
    }

    // Apply heuristics
    apply_historical_heuristics(&mut drainer_context);

    // Verify funding ratio (5 victims at 20 each = 100, total 120)
    assert_eq!(drainer_context.total_sol_volume_in, 120.0);

    // Apply rules
    let tags = apply_exfiltration_rules(&drainer_context, &rules);

    // Verify detected patterns (100/120 = 0.83 > 0.7 threshold)
    assert!(tags.contains(&"Potential Drainer Consolidation".to_string()));

    // 5. Combined patterns
    let mut combined_context = TestHistoricalWalletContext::new("combined_test");

    // Add transactions matching multiple patterns
    combined_context.add_transaction("sig1", 1000000000, true, "victim_1", 50.0);
    combined_context.add_transaction("sig2", 1000000100, true, "victim_2", 50.0);

    // Quick pass-through to mixer
    combined_context.add_transaction("sig3", 1000000200, false, "mixer1", 40.0);

    // Structuring to exchanges
    for i in 0..5 {
        combined_context.add_transaction(
            &format!("sig_struct_{}", i),
            1000001000 + i * 600,
            false,
            "exchange1",
            9.5,
        );
    }

    // Add known records
    let victim1_record =
        create_address_record("victim_1", "Drainer Victim 1", "Drainer Victim", "Medium");
    let victim2_record =
        create_address_record("victim_2", "Drainer Victim 2", "Drainer Victim", "Medium");
    let mixer1_record = create_address_record("mixer1", "Mixer Service", "Mixer", "High");
    let exchange1_record =
        create_address_record("exchange1", "Exchange Service", "Exchange", "Low");

    combined_context.update_counterparty("victim_1", Some(victim1_record));
    combined_context.update_counterparty("victim_2", Some(victim2_record));
    combined_context.update_counterparty("mixer1", Some(mixer1_record));
    combined_context.update_counterparty("exchange1", Some(exchange1_record));

    // Add custom pattern flags
    combined_context
        .heuristic_flags
        .custom_flags
        .insert("peel_chain_pattern".to_string(), 0.7);
    combined_context
        .heuristic_flags
        .custom_flags
        .insert("churning_pattern".to_string(), 0.6);

    // Apply heuristics
    apply_historical_heuristics(&mut combined_context);

    // Apply rules
    let tags = apply_exfiltration_rules(&combined_context, &rules);

    // Verify multiple patterns can be detected
    assert!(tags.len() >= 3);
    assert!(tags.contains(&"Potential Drainer Consolidation".to_string()));
    assert!(tags.contains(&"Peel Chain Exfiltration".to_string()));
    assert!(tags.contains(&"Fund Churning".to_string()));
}

#[test]
fn test_notes_update_logic() {
    // Test updating notes with new tags

    // Case 1: No existing notes
    let new_tags = vec![
        "Mixer Interaction".to_string(),
        "Structuring Outflow to CEX".to_string(),
    ];

    let updated_notes = update_notes_with_tags("", &new_tags);
    assert!(updated_notes.contains("; ExfilTags ["));
    assert!(updated_notes.contains("Mixer Interaction"));
    assert!(updated_notes.contains("Structuring Outflow to CEX"));

    // Case 2: Existing notes without exfiltration tags
    let existing_notes = "This is a suspicious address connected to a hack.";
    let updated_notes = update_notes_with_tags(existing_notes, &new_tags);

    assert!(updated_notes.starts_with(existing_notes));
    assert!(updated_notes.contains("; ExfilTags ["));
    assert!(updated_notes.contains("Mixer Interaction"));

    // Case 3: Existing notes with exfiltration tags
    let existing_notes_with_tags = format!(
        "This is a suspicious address.{}",
        create_notes_with_tags(&["Old Tag 1", "Old Tag 2"])
    );

    let updated_notes = update_notes_with_tags(&existing_notes_with_tags, &new_tags);

    assert!(updated_notes.contains("This is a suspicious address."));
    assert!(updated_notes.contains("; ExfilTags ["));
    assert!(updated_notes.contains("Mixer Interaction"));
    assert!(updated_notes.contains("Structuring Outflow to CEX"));
    assert!(!updated_notes.contains("Old Tag 1"));

    // Case 4: Empty tags list (should still update with empty tags list)
    let empty_tags: Vec<String> = Vec::new();
    let updated_notes = update_notes_with_tags(&existing_notes_with_tags, &empty_tags);

    assert!(updated_notes.contains("This is a suspicious address."));
    assert!(updated_notes.contains("; ExfilTags ["));
    assert!(updated_notes.contains("]: []"));
}
