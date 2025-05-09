pub mod chainabuse;

// Re-export main items for easier usage
pub use chainabuse::{
    MaliciousWalletReport, ReportDetail, add_wallet_to_database, fetch_malicious_solana_addresses,
    lookup_wallet_address,
};
