// In thehackerdex-api/src/handlers/data_handler.rs
use axum::Router;
use crate::AppState;
// use axum::{extract::State, routing::get, Json};
// use crate::models::{WalletSummaryResponse, WalletGraphResponse}; // Example models
// use crate::errors::ApiError;
// use tracing::instrument;

pub fn wallet_data_routes() -> Router<AppState> {
    Router::new()
    // .route("/:wallet_address/summary", get(get_wallet_summary))
    // .route("/:wallet_address/graph", get(get_wallet_graph))
    // Define other data-retrieval routes here
}

/*
#[instrument(skip(state), fields(wallet_address = %wallet_address))]
async fn get_wallet_summary(
    State(_state): State<AppState>,
    axum::extract::Path(wallet_address): axum::extract::Path<String>,
) -> Result<Json<WalletSummaryResponse>, ApiError> {
    tracing::info!("Request for wallet summary: {}", wallet_address);
    // TODO: Implement actual data fetching from DB/cache
    Err(ApiError::NotFound(format!(
        "Summary for {} not yet implemented or not found.",
        wallet_address
    )))
}

#[instrument(skip(state), fields(wallet_address = %wallet_address))]
async fn get_wallet_graph(
    State(_state): State<AppState>,
    axum::extract::Path(wallet_address): axum::extract::Path<String>,
) -> Result<Json<WalletGraphResponse>, ApiError> {
    tracing::info!("Request for wallet graph: {}", wallet_address);
    // TODO: Implement actual graph data generation
    Err(ApiError::NotFound(format!(
        "Graph for {} not yet implemented or not found.",
        wallet_address
    )))
}
*/