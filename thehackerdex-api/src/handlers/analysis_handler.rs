use axum::{extract::State, routing::post, Json, Router};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, instrument, warn};

use crate::{models::*, AppState, errors::ApiError};

// Core library imports
use thehackerdex::{
    config::Config as CoreLibConfig,
    // config::FeatureFlags, // Will be removed if unused after fixes
    // db::models::AddressRecord, // Used for KnownEntityInfo mapping - type is inferred
    db::Repository as DbRepository,
    error::HackerdexError,
    heuristic_engine::{
        config::HeuristicWeightsConfig,
        context_builder, // For build_wallet_contexts
        risk_scoring::RiskScore, // RiskCategory is used via risk_score.category.to_string()
        types::HeuristicFlags, // WalletContext is used
        run_all_heuristics, // The main function to get HeuristicFlags
    },
    rpc::client::RateLimitedClient as SolanaClient, // Renamed for consistency
    // For dummy TransactionAnalysisData, if necessary for run_all_heuristics
    analysis::{
        transaction_parser::ParsedTransaction, // TransactionInstruction removed
        transaction_analysis::TransactionAnalysisData,
    }
};


pub fn analysis_routes() -> Router<AppState> {
    Router::new().route("/", post(trigger_wallet_analysis))
}

#[instrument(skip(state, payload), fields(wallet_address = %payload.wallet_address))]
async fn trigger_wallet_analysis(
    State(state): State<AppState>,
    Json(payload): Json<AnalyzeWalletRequest>,
) -> Result<Json<AnalyzeWalletResponse>, ApiError> {
    info!("Received analysis request for wallet: {}", payload.wallet_address);
    let wallet_address_str = payload.wallet_address.clone();

    // --- 1. Input Validation ---
    if wallet_address_str.len() < 32 || wallet_address_str.len() > 44 {
        warn!("Invalid wallet address format received: {}", wallet_address_str);
        return Err(ApiError::Validation("Invalid wallet address format.".to_string()));
    }

    // --- 2. Configuration & Setup ---
    let core_config = Arc::new(CoreLibConfig::from_env());
    // --- 2. Configuration & Setup ---\n    let core_config = Arc::new(CoreLibConfig::from_env());
    let weights_config = Arc::new(HeuristicWeightsConfig::load(&core_config));
    // let feature_flags = Arc::new(core_config.load_feature_flags().map_err(|e| { // FeatureFlags currently unused
    //     error!("Failed to load feature flags: {:?}", e);
    //     ApiError::InternalServerError("Configuration error: Failed to load feature flags.".to_string())
    // })?);

    let solana_client = Arc::new(SolanaClient::new(Some(core_config.rpc_url.clone())));
    let db_repo = Arc::new(DbRepository::new(state.db_pool.clone()));

    // --- 3. Core Wallet Analysis ---
    // This section adapts logic found in the `analyze_known_wallets.rs` binary.
    // It's simplified due to the complexity of directly porting the binary's deep historical analysis.

    // A. Build WalletContext
    // `build_wallet_contexts` returns a Vec, we expect one for the single address.
    let wallet_contexts = context_builder::build_wallet_contexts(
        &[wallet_address_str.clone()], // build_wallet_contexts expects &[String]
        &db_repo,
        &*solana_client.client, // Pass the inner RpcClient
    )
    .await
    .map_err(|e| {
        error!("Failed to build wallet context for {}: {:?}", wallet_address_str, e);
        e.into() // Converts HackerdexError to ApiError via #[from]
    })?;

    let wallet_context = wallet_contexts.into_iter().next().ok_or_else(|| {
        error!("WalletContext not found for {} after build_wallet_contexts", wallet_address_str);
        ApiError::InternalServerError("Failed to build wallet context.".to_string())
    })?;


    // B. Get HeuristicFlags
    // `run_all_heuristics` requires `TransactionAnalysisData`.
    // For a wallet-centric API, we create a minimal/dummy `TransactionAnalysisData`
    // because some heuristics might still use parts of it, even if it's mostly empty.
    // The primary input for wallet-specific heuristics will be the `wallet_context`.
    let dummy_parsed_tx = ParsedTransaction {
        signature: format!("dummy_tx_for_wallet_{}", wallet_address_str),
        program_ids: Vec::new(),
        involved_accounts: vec![wallet_address_str.clone()],
        pre_token_balances: Vec::new(),
        post_token_balances: Vec::new(),
        execution_status: Some("Success".to_string()), // Or None
        fee: 0,
    };
    let dummy_tx_analysis_data = TransactionAnalysisData::new(dummy_parsed_tx);

    let heuristic_flags = run_all_heuristics(&dummy_tx_analysis_data, &wallet_context);


    // C. Determine Critical Flags & Create RiskScore
    // This is a simplified way to determine `has_critical_flags`.
    // A more robust implementation would check specific flags based on defined critical criteria.
    let has_critical_flags_val = heuristic_flags.direct_illicit_interaction; // Example critical flag

    let wallet_score_val = heuristic_flags.get_overall_suspicion_score();
    let risk_factors_val = heuristic_flags.get_triggered_flags_description();

    let mut risk_score_obj = RiskScore::from_score_and_factors(
        wallet_score_val,
        risk_factors_val.clone(), // clone because risk_factors_val is used later for HeuristicInfo
        has_critical_flags_val,
    );
    // `from_score_and_factors` doesn't populate `score_components`.
    // For a more complete RiskScore, we might need a custom constructor or method.
    // For now, score_components will be empty, affecting HeuristicInfo.score_impact.


    // --- 4. Fetch KnownEntityInfo ---
    let known_entity_info_db_result = db_repo.get_address_details(&wallet_address_str).await;
    let known_entity_info_api = match known_entity_info_db_result {
        Ok(address_record) => Some(KnownEntityInfo {
            name: address_record.entity_name,
            category: address_record.category,
            source_of_info: Some(address_record.source_of_info),
            confidence_score: Some(address_record.confidence_score as f64),
            notes: address_record.notes,
        }),
        Err(HackerdexError::NotFound(_)) => None,
        Err(e) => {
            error!("Failed to fetch known entity info for {}: {:?}", wallet_address_str, e);
            return Err(e.into()); // Converts HackerdexError to ApiError via #[from]
        }
    };

    // --- 5. Build API Response ---
    let summary_response = build_wallet_summary_response(
        &wallet_address_str,
        &heuristic_flags,
        &risk_score_obj, // Pass the constructed RiskScore
        known_entity_info_api,
        &weights_config, // Pass weights for score_impact
    )?;

    Ok(Json(AnalyzeWalletResponse {
        job_id: None, // Synchronous completion
        status: "completed".to_string(),
        message: "Wallet analysis completed successfully.".to_string(),
        analysis_summary: Some(summary_response),
    }))
}

fn build_wallet_summary_response(
    wallet_address: &str,
    flags: &HeuristicFlags,
    risk_score: &RiskScore,
    known_info: Option<KnownEntityInfo>,
    weights_config: &HeuristicWeightsConfig,
) -> Result<WalletSummaryResponse, ApiError> {
    let mut detected_heuristics_api: Vec<HeuristicInfo> = Vec::new();

    // Manual mapping from HeuristicFlags fields to HeuristicInfo
    // This requires knowing the "name" of each flag and finding its description and score impact.
    // Using risk_score.risk_factors for descriptions where possible.
    // Score impact can be estimated from weights_config or default values.

    let descriptions_map: HashMap<String, String> = risk_score.risk_factors.iter().fold(HashMap::new(), |mut acc, desc| {
        // This is a simplification. We need a robust way to map descriptions to flag names.
        // For example, if desc is "High frequency transaction patterns detected", map it to "is_high_frequency".
        if desc.to_lowercase().contains("high frequency") { acc.insert("is_high_frequency".to_string(), desc.clone()); }
        if desc.to_lowercase().contains("structuring") { acc.insert("structuring_score".to_string(), desc.clone()); }
        if desc.to_lowercase().contains("pass-through") { acc.insert("is_pass_through".to_string(), desc.clone()); }
        if desc.to_lowercase().contains("illicit interaction") { acc.insert("direct_illicit_interaction".to_string(), desc.clone()); }
        if desc.to_lowercase().contains("risky categories") { acc.insert("risky_category_interaction_score".to_string(), desc.clone()); }
        if desc.to_lowercase().contains("newly created wallet") { acc.insert("is_new_wallet".to_string(), desc.clone()); }
        if desc.to_lowercase().contains("risky sources") { acc.insert("risky_funding_source_ratio".to_string(), desc.clone()); }
        if desc.to_lowercase().contains("risky destinations") { acc.insert("risky_spending_destination_ratio".to_string(), desc.clone()); }
        if desc.to_lowercase().contains("rapid dispersal") { acc.insert("rapid_dispersal_pattern".to_string(), desc.clone()); }
        if desc.to_lowercase().contains("fund consolidation") { acc.insert("fund_consolidation_pattern".to_string(), desc.clone()); }
        if desc.to_lowercase().contains("bidirectional flow") { acc.insert("bidirectional_flow_pattern".to_string(), desc.clone()); }
        acc
    });

    let add_heuristic = |heuristics_vec: &mut Vec<HeuristicInfo>, name: &str, is_triggered: bool, _value: f64, default_desc: &str| {
        if is_triggered {
            heuristics_vec.push(HeuristicInfo {
                name: name.to_string(),
                description: descriptions_map.get(name).cloned().unwrap_or_else(|| default_desc.to_string()),
                score_impact: risk_score.score_components.get(name).map(|&v| v as f64).unwrap_or_else(|| weights_config.get_weight(name) as f64),
            });
        }
    };

    add_heuristic(&mut detected_heuristics_api, "is_high_frequency", flags.is_high_frequency, 0.0, "High frequency transaction patterns detected.");
    add_heuristic(&mut detected_heuristics_api, "structuring_score", flags.structuring_score > 0.0, flags.structuring_score as f64, "Possible structuring behavior.");
    add_heuristic(&mut detected_heuristics_api, "is_pass_through", flags.is_pass_through, 0.0, "Pass-through wallet behavior detected.");
    add_heuristic(&mut detected_heuristics_api, "direct_illicit_interaction", flags.direct_illicit_interaction, 0.0, "Direct interaction with known illicit address.");
    add_heuristic(&mut detected_heuristics_api, "risky_category_interaction_score", flags.risky_category_interaction_score > 0.0, flags.risky_category_interaction_score as f64, "Interaction with risky categories.");
    add_heuristic(&mut detected_heuristics_api, "is_new_wallet", flags.is_new_wallet, 0.0, "Newly created wallet.");
    add_heuristic(&mut detected_heuristics_api, "risky_funding_source_ratio", flags.risky_funding_source_ratio > 0.0, flags.risky_funding_source_ratio as f64, "Receives funds from risky sources.");
    add_heuristic(&mut detected_heuristics_api, "risky_spending_destination_ratio", flags.risky_spending_destination_ratio > 0.0, flags.risky_spending_destination_ratio as f64, "Sends funds to risky destinations.");
    add_heuristic(&mut detected_heuristics_api, "rapid_dispersal_pattern", flags.rapid_dispersal_pattern, 0.0, "Rapid fund dispersal pattern detected.");
    add_heuristic(&mut detected_heuristics_api, "fund_consolidation_pattern", flags.fund_consolidation_pattern, 0.0, "Fund consolidation pattern detected.");
    add_heuristic(&mut detected_heuristics_api, "bidirectional_flow_pattern", flags.bidirectional_flow_pattern, 0.0, "Bidirectional fund flow pattern detected.");

    for (custom_flag_name, &custom_flag_value) in &flags.custom_flags {
        if custom_flag_value > 0.0 { // Or some other threshold for custom flags
             detected_heuristics_api.push(HeuristicInfo {
                name: custom_flag_name.clone(),
                description: format!("Custom heuristic '{}' triggered.", custom_flag_name), // Generic description
                score_impact: custom_flag_value as f64,
            });
        }
    }


    Ok(WalletSummaryResponse {
        wallet_address: wallet_address.to_string(),
        risk_score: Some(risk_score.numerical_score as f64),
        risk_category: Some(risk_score.category.to_string()),
        detected_heuristics: detected_heuristics_api,
        known_entity_info: known_info,
    })
}