use hackerdex::config;
use sqlx::postgres::PgPoolOptions;
use std::env;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present
    dotenv::dotenv().ok();

    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    info!("Starting HackerDex Database Check");

    // Load configuration
    let _config = config::Config::from_env();
    info!("Loaded configuration");

    // Initialize database connection
    info!("Initializing database connection...");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Create a connection pool with a timeout
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    info!("Database connection successful");

    // Run migrations
    info!("Running migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;

    info!("Migrations completed successfully");

    // Simple test query to verify connection
    let result = sqlx::query!("SELECT 1 as test").fetch_one(&pool).await?;

    info!("Test query result: {:?}", result.test);

    // Close the connection pool
    pool.close().await;

    info!("Database check completed successfully");
    Ok(())
}
