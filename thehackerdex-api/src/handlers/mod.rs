pub mod analysis_handler;
pub mod data_handler;
pub mod chat_handler;

use axum::Router;
use crate::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new()
        .nest("/analyze", analysis_handler::analysis_routes())
        .nest("/wallet", data_handler::wallet_data_routes())
        .nest("/chat", chat_handler::chat_routes())
    // Add more top-level routes if needed
}