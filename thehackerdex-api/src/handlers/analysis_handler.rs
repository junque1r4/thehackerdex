// In thehackerdex-api/src/handlers/analysis_handler.rs
use axum::{routing::post, Router, Json, extract::State};
use crate::models::{AnalyzeWalletRequest, AnalyzeWalletResponse};
use crate::{AppState, errors::ApiError};
use tracing::instrument;

pub fn analysis_routes() -> Router<AppState> {
    Router::new().route("/", post(trigger_wallet_analysis))
    // Potentially a GET route for analysis status if using async tasks
    // .route("/status/:job_id", get(get_analysis_status))
}

#[instrument(skip(state, payload), fields(wallet_address = %payload.wallet_address))]
async fn trigger_wallet_analysis(
    State(state): State<AppState>,
    Json(payload): Json<AnalyzeWalletRequest>,
) -> Result<Json<AnalyzeWalletResponse>, ApiError> {
    // TODO: Implement actual analysis logic in Phase 2
    tracing::info!("Received analysis request for wallet: {}", payload.wallet_address);

    // Placeholder response
    Ok(Json(AnalyzeWalletResponse {
        job_id: None,
        status: "pending_implementation".to_string(),
        message: format!("Analysis for {} is pending implementation.", payload.wallet_address),
        analysis_summary: None,
    }))
}