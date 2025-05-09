use crate::analysis::transaction_parser::ParsedTransaction;
use crate::db::{models::AddressRecord, repository};
use crate::error::HackerdexResult;
use sqlx::PgPool;
use tracing::{debug, info};

/// Results of cross-referencing a wallet address against the known address database
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WalletDirectAnalysisResult {
    /// The wallet address that was analyzed
    pub wallet_address: String,
    /// Whether the wallet was found in the known address database
    pub is_known: bool,
    /// The full address record if found, or None if not found
    pub address_record: Option<AddressRecord>,
}

/// Cross-references a list of wallet addresses against the known address database
///
/// # Arguments
///
/// * `pool` - PostgreSQL connection pool
/// * `wallet_addresses` - List of wallet addresses to cross-reference
///
/// # Returns
///
/// A `HackerdexResult` containing a vector of `WalletDirectAnalysisResult` or an error
#[allow(dead_code)]
pub async fn cross_reference_wallet_addresses(
    pool: &PgPool,
    wallet_addresses: &[String],
) -> HackerdexResult<Vec<WalletDirectAnalysisResult>> {
    let mut results = Vec::with_capacity(wallet_addresses.len());

    // Process each wallet address
    for wallet_address in wallet_addresses {
        debug!("Cross-referencing wallet address: {}", wallet_address);

        // Query the database for this wallet address
        let address_record = repository::get_address_details(pool, wallet_address).await;

        let is_known = address_record.is_ok();
        let record = address_record.ok();

        // Store the results
        results.push(WalletDirectAnalysisResult {
            wallet_address: wallet_address.clone(),
            is_known,
            address_record: record,
        });
    }

    Ok(results)
}

/// Analyzes wallet addresses from a parsed transaction
///
/// # Arguments
///
/// * `pool` - PostgreSQL connection pool
/// * `transaction` - The parsed transaction to analyze
///
/// # Returns
///
/// A `HackerdexResult` containing a vector of `WalletDirectAnalysisResult` or an error
#[allow(dead_code)]
pub async fn analyze_transaction_wallets_direct(
    pool: &PgPool,
    transaction: &ParsedTransaction,
) -> HackerdexResult<Vec<WalletDirectAnalysisResult>> {
    info!(
        "Analyzing wallets for transaction: {}",
        transaction.signature
    );
    cross_reference_wallet_addresses(pool, &transaction.involved_accounts).await
}

/// Analyzes and categorizes a transaction's wallets based on known wallet addresses
///
/// # Arguments
///
/// * `pool` - PostgreSQL connection pool
/// * `transaction` - The parsed transaction to analyze
///
/// # Returns
///
/// A `HackerdexResult` containing a summary of findings as a string
#[allow(dead_code)]
pub async fn categorize_transaction_wallets(
    pool: &PgPool,
    transaction: &ParsedTransaction,
) -> HackerdexResult<String> {
    let wallet_analyses = analyze_transaction_wallets_direct(pool, transaction).await?;

    let mut summary = String::new();
    let mut has_high_risk = false;
    let mut risk_details = Vec::new();
    let mut categories = Vec::new();

    for analysis in &wallet_analyses {
        if let Some(record) = &analysis.address_record {
            categories.push(record.category.clone());

            // Check for high risk wallets
            if record.risk_level == "High" || record.risk_level == "Critical" {
                has_high_risk = true;
                risk_details.push(format!(
                    "Wallet {} ({}) has risk level {} (confidence: {}/5)",
                    record.entity_name, record.address, record.risk_level, record.confidence_score
                ));
            }

            summary.push_str(&format!(
                "Wallet {} is known as {} ({}), risk level: {}\n",
                analysis.wallet_address, record.entity_name, record.category, record.risk_level
            ));
        } else {
            summary.push_str(&format!(
                "Wallet {} is unknown in the database\n",
                analysis.wallet_address
            ));
        }
    }

    // Add summary header based on findings
    let mut header = String::new();
    if has_high_risk {
        header.push_str("⚠️ HIGH RISK WALLETS DETECTED ⚠️\n");
        for detail in risk_details {
            header.push_str(&format!("- {}\n", detail));
        }
        header.push('\n');
    }

    if !categories.is_empty() {
        header.push_str("Wallet categories found: ");
        header.push_str(&categories.join(", "));
        header.push_str("\n\n");
    }

    // Combine header and detailed summary
    Ok(format!("{}{}", header, summary))
}
