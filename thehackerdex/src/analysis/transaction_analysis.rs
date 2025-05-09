use crate::analysis::HeuristicFlags;
use crate::analysis::program_analyzer;
use crate::analysis::program_analyzer::ProgramAnalysis;
use crate::analysis::transaction_parser::ParsedTransaction;
use crate::analysis::wallet_analyzer;
use crate::analysis::wallet_analyzer::WalletDirectAnalysisResult;
use crate::db::Repository;
use crate::error::HackerdexResult;
use crate::rpc::client::RateLimitedClient;
use sqlx::PgPool;
use std::fmt;
use tracing::{debug, info};

/// Consolidated structure containing all analysis data for a transaction
///
/// This struct serves as the central container for all analysis results related to a transaction.
/// It includes the raw transaction data, program analysis results, wallet analysis results,
/// and placeholders for future heuristic analysis outputs.
#[derive(Debug)]
#[allow(dead_code)]
pub struct TransactionAnalysisData {
    /// The parsed transaction containing raw transaction data
    pub parsed_transaction: ParsedTransaction,

    /// Analysis results for programs interacted with in the transaction
    pub program_analysis: Vec<ProgramAnalysis>,

    /// Direct analysis results for wallets involved in the transaction
    pub wallet_direct_analysis: Vec<WalletDirectAnalysisResult>,

    // Placeholders for future heuristic results
    /// Placeholder for future implementation of heuristic flags
    /// This will store boolean/numeric results of individual heuristics
    pub heuristic_flags: Option<HeuristicFlags>,

    /// The overall risk assessment of the transaction
    /// This stores the numerical score, risk category, and other risk-related information
    pub risk_score: Option<crate::heuristic_engine::risk_scoring::RiskScore>,

    /// Placeholder for future implementation of risk factors
    /// This will store descriptions of triggered heuristics
    pub risk_factors: Option<Vec<String>>,
}

/// Implement Display trait for TransactionAnalysisData to provide a readable
/// representation of the analysis results
impl fmt::Display for TransactionAnalysisData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Create a top summary banner
        let has_high_risk = self.has_high_risk_programs() || self.has_high_risk_wallets();
        let summary_banner = if has_high_risk {
            "⚠️  WARNING: HIGH RISK TRANSACTION DETECTED  ⚠️\n\
             ==================================================\n\n"
        } else {
            ""
        };

        // Write transaction signature and basic info
        let signature = &self.parsed_transaction.signature;
        writeln!(f, "{}TRANSACTION ANALYSIS: {}\n", summary_banner, signature)?;

        // Transaction fee
        writeln!(f, "Fee: {} lamports", self.parsed_transaction.fee)?;

        // Status
        if let Some(status) = &self.parsed_transaction.execution_status {
            writeln!(f, "Status: {}", status)?;
        } else {
            writeln!(f, "Status: Unknown")?;
        }
        writeln!(f, "")?;

        // PROGRAM ANALYSIS SECTION
        writeln!(f, "PROGRAM ANALYSIS\n---------------")?;
        if self.program_analysis.is_empty() {
            writeln!(f, "No programs analyzed.")?;
        } else {
            let high_risk_programs: Vec<_> = self
                .program_analysis
                .iter()
                .filter(|p| {
                    p.address_record.as_ref().map_or(false, |r| {
                        r.risk_level == "High" || r.risk_level == "Critical"
                    })
                })
                .collect();

            if !high_risk_programs.is_empty() {
                writeln!(f, "⚠️  HIGH RISK PROGRAMS  ⚠️")?;
                for program in high_risk_programs {
                    if let Some(record) = &program.address_record {
                        writeln!(f, "- {} ({})", record.entity_name, program.program_id)?;
                        writeln!(f, "  Category: {}", record.category)?;
                        writeln!(f, "  Risk Level: {}", record.risk_level)?;
                        writeln!(f, "  Confidence: {}/5", record.confidence_score)?;
                        if let Some(notes) = &record.notes {
                            writeln!(f, "  Notes: {}", notes)?;
                        }
                        writeln!(f, "")?;
                    }
                }
            }

            writeln!(f, "All Programs:")?;
            for (i, program) in self.program_analysis.iter().enumerate() {
                if program.is_known {
                    if let Some(record) = &program.address_record {
                        writeln!(
                            f,
                            "{}. {} ({})",
                            i + 1,
                            record.entity_name,
                            program.program_id
                        )?;
                        writeln!(f, "   Category: {}", record.category)?;
                        writeln!(f, "   Risk Level: {}", record.risk_level)?;
                    }
                } else {
                    writeln!(f, "{}. Unknown Program: {}", i + 1, program.program_id)?;
                }
            }
        }
        writeln!(f, "")?;

        // WALLET ANALYSIS SECTION
        writeln!(f, "WALLET ANALYSIS\n--------------")?;
        if self.wallet_direct_analysis.is_empty() {
            writeln!(f, "No wallets analyzed.")?;
        } else {
            let high_risk_wallets: Vec<_> = self
                .wallet_direct_analysis
                .iter()
                .filter(|w| {
                    w.address_record.as_ref().map_or(false, |r| {
                        r.risk_level == "High" || r.risk_level == "Critical"
                    })
                })
                .collect();

            if !high_risk_wallets.is_empty() {
                writeln!(f, "⚠️  HIGH RISK WALLETS  ⚠️")?;
                for wallet in high_risk_wallets {
                    if let Some(record) = &wallet.address_record {
                        writeln!(f, "- {} ({})", record.entity_name, wallet.wallet_address)?;
                        writeln!(f, "  Category: {}", record.category)?;
                        writeln!(f, "  Risk Level: {}", record.risk_level)?;
                        writeln!(f, "  Confidence: {}/5", record.confidence_score)?;
                        if let Some(notes) = &record.notes {
                            writeln!(f, "  Notes: {}", notes)?;
                        }
                        writeln!(f, "")?;
                    }
                }
            }

            writeln!(f, "All Involved Wallets:")?;
            for (i, wallet) in self.wallet_direct_analysis.iter().enumerate() {
                if wallet.is_known {
                    if let Some(record) = &wallet.address_record {
                        writeln!(
                            f,
                            "{}. {} ({})",
                            i + 1,
                            record.entity_name,
                            wallet.wallet_address
                        )?;
                        writeln!(f, "   Category: {}", record.category)?;
                        writeln!(f, "   Risk Level: {}", record.risk_level)?;
                    }
                } else {
                    writeln!(f, "{}. Unknown Wallet: {}", i + 1, wallet.wallet_address)?;
                }
            }
        }
        writeln!(f, "")?;

        // RISK ASSESSMENT SECTION
        if let Some(risk_score) = &self.risk_score {
            writeln!(f, "RISK ASSESSMENT\n--------------")?;
            writeln!(
                f,
                "Risk Score: {:.2} ({})",
                risk_score.numerical_score, risk_score.category
            )?;

            // Display additional risk summary from the risk score
            writeln!(f, "{}", risk_score.summary())?;
            writeln!(f, "")?;
        }

        // RISK FACTORS SECTION
        if let Some(risk_factors) = &self.risk_factors {
            if !risk_factors.is_empty() {
                writeln!(f, "RISK FACTORS\n------------")?;
                for (i, factor) in risk_factors.iter().enumerate() {
                    writeln!(f, "{}. {}", i + 1, factor)?;
                }
                writeln!(f, "")?;
            }
        }

        // HEURISTIC FLAGS SECTION
        if let Some(heuristic_flags) = &self.heuristic_flags {
            writeln!(f, "HEURISTIC DETAILS\n----------------")?;
            writeln!(
                f,
                "Overall Suspicion Score: {:.2}/10.0",
                heuristic_flags.get_overall_suspicion_score()
            )?;

            if heuristic_flags.direct_illicit_interaction {
                writeln!(
                    f,
                    "⚠️  Direct interaction with known illicit address detected!"
                )?;
            }

            if heuristic_flags.is_high_frequency {
                writeln!(f, "- High frequency transaction patterns")?;
            }

            if heuristic_flags.is_pass_through {
                writeln!(f, "- Pass-through wallet behavior")?;
            }

            if heuristic_flags.structuring_score > 0.0 {
                writeln!(
                    f,
                    "- Structuring score: {:.2}",
                    heuristic_flags.structuring_score
                )?;
            }

            if heuristic_flags.risky_funding_source_ratio > 0.0 {
                writeln!(
                    f,
                    "- Risky funding source ratio: {:.2}",
                    heuristic_flags.risky_funding_source_ratio
                )?;
            }

            if heuristic_flags.risky_spending_destination_ratio > 0.0 {
                writeln!(
                    f,
                    "- Risky spending destination ratio: {:.2}",
                    heuristic_flags.risky_spending_destination_ratio
                )?;
            }

            if heuristic_flags.rapid_dispersal_pattern {
                writeln!(f, "- Rapid fund dispersal pattern detected")?;
            }

            if heuristic_flags.fund_consolidation_pattern {
                writeln!(f, "- Fund consolidation pattern detected")?;
            }
        }

        Ok(())
    }
}

impl TransactionAnalysisData {
    /// Creates a new TransactionAnalysisData instance with the given parsed transaction
    ///
    /// # Arguments
    ///
    /// * `parsed_transaction` - The parsed transaction data
    ///
    /// # Returns
    ///
    /// A new `TransactionAnalysisData` instance with empty analysis results
    #[allow(dead_code)]
    pub fn new(parsed_transaction: ParsedTransaction) -> Self {
        TransactionAnalysisData {
            parsed_transaction,
            program_analysis: Vec::new(),
            wallet_direct_analysis: Vec::new(),
            heuristic_flags: None,
            risk_score: None,
            risk_factors: None,
        }
    }

    /// Creates a new TransactionAnalysisData instance with all initial analysis data
    ///
    /// # Arguments
    ///
    /// * `parsed_transaction` - The parsed transaction data
    /// * `program_analysis` - The program analysis results
    /// * `wallet_direct_analysis` - The direct wallet analysis results
    ///
    /// # Returns
    ///
    /// A new `TransactionAnalysisData` instance with the provided analysis results
    #[allow(dead_code)]
    pub fn with_initial_analysis(
        parsed_transaction: ParsedTransaction,
        program_analysis: Vec<ProgramAnalysis>,
        wallet_direct_analysis: Vec<WalletDirectAnalysisResult>,
    ) -> Self {
        TransactionAnalysisData {
            parsed_transaction,
            program_analysis,
            wallet_direct_analysis,
            heuristic_flags: None,
            risk_score: None,
            risk_factors: None,
        }
    }

    /// Presents the analysis results in a formatted string
    ///
    /// # Returns
    ///
    /// A string containing a readable presentation of the analysis results
    #[allow(dead_code)]
    pub fn print_analysis(&self) -> String {
        format!("{}", self)
    }

    /// Check if any programs in the transaction are flagged as high risk
    fn has_high_risk_programs(&self) -> bool {
        self.program_analysis.iter().any(|p| {
            p.address_record
                .as_ref()
                .map(|r| r.risk_level == "High" || r.risk_level == "Critical")
                .unwrap_or(false)
        })
    }

    /// Check if any wallets in the transaction are flagged as high risk
    fn has_high_risk_wallets(&self) -> bool {
        // Check for direct wallet risk from known addresses
        let has_known_risky_wallets = self.wallet_direct_analysis.iter().any(|w| {
            w.address_record
                .as_ref()
                .map(|r| r.risk_level == "High" || r.risk_level == "Critical")
                .unwrap_or(false)
        });

        // Check if heuristics indicate high risk
        let has_risky_heuristics = self.heuristic_flags.as_ref().map_or(false, |flags| {
            flags.direct_illicit_interaction || flags.get_overall_suspicion_score() > 5.0
        });

        // Check if risk score is High or Critical
        let has_high_risk_score = self.risk_score.as_ref().map_or(false, |score| {
            score.category.to_string() == "High" || score.category.to_string() == "Critical"
        });

        has_known_risky_wallets || has_risky_heuristics || has_high_risk_score
    }
}

/// Performs initial analysis on a transaction
///
/// This function performs the initial analysis on a transaction by:
/// 1. Analyzing the programs involved in the transaction
/// 2. Analyzing the wallets involved in the transaction
/// 3. Returning a consolidated TransactionAnalysisData object with all results
///
/// # Arguments
///
/// * `pool` - PostgreSQL connection pool
/// * `transaction` - The parsed transaction to analyze
///
/// # Returns
///
/// A `HackerdexResult` containing the `TransactionAnalysisData` or an error
#[allow(dead_code)]
pub async fn perform_initial_analysis(
    pool: &PgPool,
    transaction: &ParsedTransaction,
) -> HackerdexResult<TransactionAnalysisData> {
    info!(
        "Performing initial analysis for transaction: {}",
        transaction.signature
    );

    // Call analyze_transaction_programs to get program analysis results
    debug!("Analyzing transaction programs");
    let program_analysis =
        program_analyzer::analyze_transaction_programs(pool, transaction).await?;

    // Call analyze_transaction_wallets_direct to get wallet analysis results
    debug!("Analyzing transaction wallets");
    let wallet_analysis =
        wallet_analyzer::analyze_transaction_wallets_direct(pool, transaction).await?;

    // Populate and return the TransactionAnalysisData struct
    debug!("Creating TransactionAnalysisData with analysis results");
    let analysis_data = TransactionAnalysisData::with_initial_analysis(
        transaction.clone(),
        program_analysis,
        wallet_analysis,
    );

    Ok(analysis_data)
}

/// Performs comprehensive analysis on a transaction, including heuristic checks and risk scoring
///
/// This function runs the initial analysis, then applies heuristics to the transaction and
/// calculates a risk score based on the results. It integrates all available analysis components
/// into a complete picture of the transaction's risk profile.
///
/// # Arguments
///
/// * `pool` - Database pool for querying known addresses
/// * `transaction` - The parsed transaction data to analyze
/// * `rpc_client` - RPC client for fetching on-chain data required for heuristic analysis
///
/// # Returns
///
/// A `HackerdexResult` containing the comprehensive `TransactionAnalysisData` or an error
pub async fn perform_comprehensive_analysis(
    pool: &PgPool,
    transaction: &ParsedTransaction,
    rpc_client: &RateLimitedClient, // Changed to RateLimitedClient
) -> HackerdexResult<TransactionAnalysisData> {
    // First, perform the initial analysis
    info!(
        "Performing comprehensive analysis for transaction: {}",
        transaction.signature
    );
    let initial_analysis = perform_initial_analysis(pool, transaction).await?;

    // Create a repository instance for the database operations
    let repo = Repository::new(pool.clone());

    // Run heuristic checks and risk scoring on the transaction
    debug!("Running heuristics and risk scoring");
    let analysis_with_heuristics =
        crate::heuristic_engine::run_heuristics(initial_analysis, &repo, rpc_client).await?;

    info!(
        "Completed comprehensive analysis for transaction: {}",
        transaction.signature
    );
    Ok(analysis_with_heuristics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{AddressData, AddressRecord};
    use crate::db::repository;
    use solana_transaction_status::UiTransactionTokenBalance;
    use sqlx::postgres::PgPoolOptions;
    use std::env;

    // Helper function to create a test database connection
    async fn setup_test_db() -> HackerdexResult<Option<PgPool>> {
        // If SKIP_DB_TESTS environment variable is set, skip these tests
        if env::var("SKIP_DB_TESTS").is_ok() {
            return Ok(None);
        }

        // Get connection string from environment or use default test DB
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@localhost:5432/hackerdex_test".to_string()
        });

        // Connect to the database
        match PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
        {
            Ok(pool) => {
                // Clear the test database to ensure clean state
                match sqlx::query!("DELETE FROM known_addresses")
                    .execute(&pool)
                    .await
                {
                    Ok(_) => Ok(Some(pool)),
                    Err(e) => {
                        println!("Skipping DB tests - failed to clear test database: {}", e);
                        Ok(None)
                    }
                }
            }
            Err(e) => {
                println!("Skipping DB tests - failed to connect to database: {}", e);
                Ok(None)
            }
        }
    }

    // Helper function to populate test data
    async fn populate_test_data(pool: &PgPool) -> HackerdexResult<()> {
        // Add some known program addresses for testing
        let test_addresses = vec![
            AddressData {
                address: "11111111111111111111111111111111".to_string(),
                entity_name: "System Program".to_string(),
                category: "Core Program".to_string(),
                risk_level: "Low".to_string(),
                source_of_info: "Official Docs".to_string(),
                confidence_score: 5,
                notes: Some("Solana System Program".to_string()),
            },
            AddressData {
                address: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
                entity_name: "SPL Token".to_string(),
                category: "Token Program".to_string(),
                risk_level: "Low".to_string(),
                source_of_info: "Official Docs".to_string(),
                confidence_score: 5,
                notes: Some("Solana Program Library Token Program".to_string()),
            },
            AddressData {
                address: "MaliciousPr0gramxxxxxxxxxxxxxxxxxxxxx111111".to_string(),
                entity_name: "Known Hacker Program".to_string(),
                category: "Malicious Contract".to_string(),
                risk_level: "High".to_string(),
                source_of_info: "OSINT Report-2023-01-01".to_string(),
                confidence_score: 4,
                notes: Some("Associated with multiple fund thefts".to_string()),
            },
            // Add a known wallet address for testing wallet analysis
            AddressData {
                address: "SuspiciousWallet111111111111111111111111111".to_string(),
                entity_name: "Known Suspicious Wallet".to_string(),
                category: "Suspicious Entity".to_string(),
                risk_level: "Medium".to_string(),
                source_of_info: "OSINT Report-2023-02-15".to_string(),
                confidence_score: 3,
                notes: Some("Involved in suspicious transactions".to_string()),
            },
        ];

        // Add each test address to the database
        for address in test_addresses {
            repository::add_known_address(pool, &address).await?;
        }

        Ok(())
    }

    // Test for perform_initial_analysis function
    #[tokio::test]
    async fn test_perform_initial_analysis() -> HackerdexResult<()> {
        // Skip if SKIP_DB_TESTS environment variable is set
        if env::var("SKIP_DB_TESTS").is_ok() {
            return Ok(());
        }

        // Set up test database
        let pool_result = setup_test_db().await?;

        // Skip test if we couldn't connect to the database
        let pool = match pool_result {
            Some(pool) => pool,
            None => {
                println!(
                    "Skipping test_perform_initial_analysis due to database connection issues"
                );
                return Ok(());
            }
        };

        // Populate test data
        populate_test_data(&pool).await?;

        // Create a sample parsed transaction with known programs and wallets
        let parsed_tx = ParsedTransaction {
            signature: "test_signature_for_initial_analysis".to_string(),
            program_ids: vec![
                "11111111111111111111111111111111".to_string(), // System Program (known)
                "MaliciousPr0gramxxxxxxxxxxxxxxxxxxxxx111111".to_string(), // Malicious Program (known)
                "UnknownProgramxxxxxxxxxxxxxxxxxxxxxxxxx11111".to_string(), // Unknown Program
            ],
            involved_accounts: vec![
                "SuspiciousWallet111111111111111111111111111".to_string(), // Known suspicious wallet
                "AnotherWallet22222222222222222222222222222222".to_string(), // Unknown wallet
            ],
            pre_token_balances: Vec::<UiTransactionTokenBalance>::new(),
            post_token_balances: Vec::<UiTransactionTokenBalance>::new(),
            execution_status: Some("confirmed".to_string()),
            fee: 5000,
        };

        // Perform initial analysis
        let analysis_data = perform_initial_analysis(&pool, &parsed_tx).await?;

        // Verify program analysis results
        assert_eq!(analysis_data.program_analysis.len(), 3);

        // Check for System Program
        let system_result = analysis_data
            .program_analysis
            .iter()
            .find(|r| r.program_id == "11111111111111111111111111111111");
        assert!(system_result.is_some());
        let system_result = system_result.unwrap();
        assert!(system_result.is_known);
        assert!(system_result.address_record.is_some());
        assert_eq!(
            system_result.address_record.as_ref().unwrap().entity_name,
            "System Program"
        );

        // Check for Malicious Program
        let malicious_result = analysis_data
            .program_analysis
            .iter()
            .find(|r| r.program_id == "MaliciousPr0gramxxxxxxxxxxxxxxxxxxxxx111111");
        assert!(malicious_result.is_some());
        let malicious_result = malicious_result.unwrap();
        assert!(malicious_result.is_known);
        assert!(malicious_result.address_record.is_some());
        assert_eq!(
            malicious_result
                .address_record
                .as_ref()
                .unwrap()
                .entity_name,
            "Known Hacker Program"
        );
        assert_eq!(
            malicious_result.address_record.as_ref().unwrap().risk_level,
            "High"
        );

        // Verify wallet analysis results
        assert_eq!(analysis_data.wallet_direct_analysis.len(), 2);

        // Check for Suspicious Wallet
        let suspicious_wallet = analysis_data
            .wallet_direct_analysis
            .iter()
            .find(|r| r.wallet_address == "SuspiciousWallet111111111111111111111111111");
        assert!(suspicious_wallet.is_some());
        let suspicious_wallet = suspicious_wallet.unwrap();
        assert!(suspicious_wallet.is_known);
        assert!(suspicious_wallet.address_record.is_some());
        assert_eq!(
            suspicious_wallet
                .address_record
                .as_ref()
                .unwrap()
                .entity_name,
            "Known Suspicious Wallet"
        );
        assert_eq!(
            suspicious_wallet
                .address_record
                .as_ref()
                .unwrap()
                .risk_level,
            "Medium"
        );

        // Check for Unknown Wallet
        let unknown_wallet = analysis_data
            .wallet_direct_analysis
            .iter()
            .find(|r| r.wallet_address == "AnotherWallet22222222222222222222222222222222");
        assert!(unknown_wallet.is_some());
        let unknown_wallet = unknown_wallet.unwrap();
        assert!(!unknown_wallet.is_known);
        assert!(unknown_wallet.address_record.is_none());

        // Verify the parsed transaction is correctly stored
        assert_eq!(
            analysis_data.parsed_transaction.signature,
            "test_signature_for_initial_analysis"
        );

        // Verify placeholders for future heuristics are None
        assert!(analysis_data.heuristic_flags.is_none());
        assert!(analysis_data.risk_score.is_none());
        assert!(analysis_data.risk_factors.is_none());

        Ok(())
    }

    // Test for Display implementation
    #[test]
    fn test_display_transaction_analysis() {
        // Create a sample transaction data structure
        let parsed_tx = ParsedTransaction {
            signature: "testSignature123".to_string(),
            program_ids: vec![
                "11111111111111111111111111111111".to_string(),
                "MaliciousPr0gramxxxxxxxxxxxxxxxxxxxxx111111".to_string(),
            ],
            involved_accounts: vec![
                "SuspiciousWallet111111111111111111111111111".to_string(),
                "NormalWallet222222222222222222222222222222".to_string(),
            ],
            pre_token_balances: Vec::new(),
            post_token_balances: Vec::new(),
            execution_status: Some("confirmed".to_string()),
            fee: 5000,
        };

        // Current timestamp for testing
        let now = sqlx::types::time::OffsetDateTime::now_utc();

        // Create program analysis results
        let program_analysis = vec![
            ProgramAnalysis {
                program_id: "11111111111111111111111111111111".to_string(),
                is_known: true,
                address_record: Some(AddressRecord {
                    address: "11111111111111111111111111111111".to_string(),
                    entity_name: "System Program".to_string(),
                    category: "Core Program".to_string(),
                    risk_level: "Low".to_string(),
                    source_of_info: "Official Docs".to_string(),
                    confidence_score: 5,
                    notes: Some("Solana System Program".to_string()),
                    created_at: now,
                    updated_at: now,
                }),
            },
            ProgramAnalysis {
                program_id: "MaliciousPr0gramxxxxxxxxxxxxxxxxxxxxx111111".to_string(),
                is_known: true,
                address_record: Some(AddressRecord {
                    address: "MaliciousPr0gramxxxxxxxxxxxxxxxxxxxxx111111".to_string(),
                    entity_name: "Known Hacker Program".to_string(),
                    category: "Malicious Contract".to_string(),
                    risk_level: "High".to_string(),
                    source_of_info: "OSINT Report-2023-01-01".to_string(),
                    confidence_score: 4,
                    notes: Some("Associated with multiple fund thefts".to_string()),
                    created_at: now,
                    updated_at: now,
                }),
            },
        ];

        // Create wallet analysis results
        let wallet_analysis = vec![
            WalletDirectAnalysisResult {
                wallet_address: "SuspiciousWallet111111111111111111111111111".to_string(),
                is_known: true,
                address_record: Some(AddressRecord {
                    address: "SuspiciousWallet111111111111111111111111111".to_string(),
                    entity_name: "Known Suspicious Wallet".to_string(),
                    category: "Suspicious Entity".to_string(),
                    risk_level: "Medium".to_string(),
                    source_of_info: "OSINT Report-2023-02-15".to_string(),
                    confidence_score: 3,
                    notes: Some("Involved in suspicious transactions".to_string()),
                    created_at: now,
                    updated_at: now,
                }),
            },
            WalletDirectAnalysisResult {
                wallet_address: "NormalWallet222222222222222222222222222222".to_string(),
                is_known: false,
                address_record: None,
            },
        ];

        // Create the TransactionAnalysisData
        let analysis_data = TransactionAnalysisData::with_initial_analysis(
            parsed_tx,
            program_analysis,
            wallet_analysis,
        );

        // Convert to string and check contents
        let display_string = format!("{}", analysis_data);

        // Verify key elements are present
        assert!(display_string.contains("HIGH RISK TRANSACTION DETECTED"));
        assert!(display_string.contains("TRANSACTION ANALYSIS: testSignature123"));
        assert!(display_string.contains("Fee: 5000 lamports"));
        assert!(display_string.contains("Status: confirmed"));

        // Verify program section
        assert!(display_string.contains("PROGRAM ANALYSIS"));
        assert!(display_string.contains("HIGH RISK PROGRAMS"));
        assert!(display_string.contains("Known Hacker Program"));
        assert!(display_string.contains("System Program"));

        // Verify wallet section
        assert!(display_string.contains("WALLET ANALYSIS"));
        assert!(display_string.contains("Known Suspicious Wallet"));
        assert!(display_string.contains("Unknown Wallet: NormalWallet"));

        // Check that the string outputs cleanly without panic
        println!("Transaction Analysis Display Test:\n{}", display_string);
    }
}
