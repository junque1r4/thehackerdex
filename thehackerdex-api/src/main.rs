mod config;
mod errors;
mod handlers;
mod models;

use axum::{
    routing::get,
    Router,
    // extract::State, // Not directly used in main, but AppState is passed to with_state
    http::Method,
};
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::config::AppConfig;
use crate::errors::ApiError;

// Application State
#[derive(Clone)]
pub struct AppState {
    db_pool: PgPool,
    #[allow(dead_code)] // Field might be used in the future or by other parts of the application
    config: Arc<AppConfig>,
    // You might also want to initialize and store an instance of your
    // core analysis engine components if they are stateful and expensive to create.
    // For example:
    // solana_client: Arc<thehackerdex::rpc::RateLimitedClient>, // Corrected type
    // heuristic_engine_config: Arc<thehackerdex::heuristic_engine::config::HeuristicWeightsConfig> // Corrected type
}

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    // Initialize tracing (logging)
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting HackerDex API Server...");

    // Load configuration
    let app_config = Arc::new(AppConfig::load()?);
    info!("Configuration loaded: {:?}", app_config);

    // Create PostgreSQL connection pool
    let db_pool = PgPoolOptions::new()
        .max_connections(10) // Adjust as needed
        .connect(&app_config.database_url)
        .await?;
    info!("Database connection pool created.");

    // Run database migrations (if you manage them from the API server)
    // sqlx::migrate!("../thehackerdex/migrations").run(&db_pool).await?; // Adjusted path to migrations
    // info!("Database migrations applied.");
    // Note: Your project has a `migrations` folder at `thehackerdex/migrations`.
    // Ensure sqlx-cli is used or integrate migrations here.

    // Initialize core toolkit components if needed
    // let solana_client = Arc::new(thehackerdex::rpc::RateLimitedClient::new(Some(app_config.solana_rpc_url.clone())));
    // let heuristic_engine_config = Arc::new(thehackerdex::heuristic_engine::config::HeuristicWeightsConfig::default()); 
    // Or:
    // let heuristic_engine_config = Arc::new(
    //     thehackerdex::heuristic_engine::config::HeuristicWeightsConfig::load_default_or_from_file(
    //         app_config.heuristic_weights_path.as_deref() // Assuming a path field in AppConfig
    //     )?
    // );


    // Create application state
    let app_state = AppState {
        db_pool,
        config: Arc::clone(&app_config),
        // solana_client,
        // heuristic_engine_config,
    };

    // Define CORS policy
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_origin(Any) // In production, restrict this to your frontend's origin
        .allow_headers(Any); // Or specify allowed headers

    // Define application routes
    let app = Router::new()
        .route("/", get(root_handler))
        .nest("/api/v1", handlers::api_router()) // We'll define this next
        .with_state(app_state)
        .layer(cors); // Apply CORS middleware

    // Start the server
    let listener = tokio::net::TcpListener::bind(&app_config.server_address).await.unwrap(); // .unwrap() is fine for .bind here
    info!("Server listening on {}", app_config.server_address);
    axum::serve(listener, app.into_make_service()).await.unwrap(); // .unwrap() is fine for axum::serve

    Ok(())
}

async fn root_handler() -> &'static str {
    "HackerDex API Server is running!"
}