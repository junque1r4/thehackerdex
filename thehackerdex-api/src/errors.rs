
// In thehackerdex-api/src/errors.rs
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Configuration error: {0}")]
    Config(#[from] crate::config::ConfigError),

    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Core toolkit error: {0}")]
    Toolkit(#[from] thehackerdex::HackerdexError), // Using the actual error from the core lib

    #[error("HTTP client error: {0}")]
    Reqwest(#[from] reqwest::Error), // If you make HTTP calls directly from API

    #[error("Input validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal server error: {0}")] // Added {0} to match other detailed errors
    InternalServerError(String),

    #[error("Analysis task failed: {0}")]
    AnalysisTaskFailed(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::Config(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            ApiError::Sqlx(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Database operation failed: {}", e)),
            ApiError::Toolkit(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Analysis engine error: {}", e)),
            ApiError::Reqwest(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("External service request failed: {}", e)),
            ApiError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::InternalServerError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ApiError::AnalysisTaskFailed(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        let body = Json(json!({ "error": error_message }));
        (status, body).into_response()
    }
}