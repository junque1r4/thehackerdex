use crate::{
    HackerdexError, HackerdexResult,
    config::Config,
    db::{
        models::AddressData,
        repository::{add_known_address, get_address_details},
    },
};
use base64::{Engine as _, engine::general_purpose};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashSet;

/// ChainAbuse API response for reports
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields may be used in future or are part of external API
struct ChainAbuseResponse {
    reports: Vec<ChainAbuseReport>,
}

/// Individual report from ChainAbuse
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields may be used in future or are part of external API
struct ChainAbuseReport {
    id: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    trusted: Option<bool>,
    checked: Option<bool>,
    #[serde(rename = "scamCategory")]
    scam_category: Option<String>,
    #[serde(rename = "entityName")]
    entity_name: Option<String>,
    #[serde(rename = "reportType")]
    report_type: String,
    addresses: Vec<ChainAbuseAddress>,
    tags: Option<Vec<String>>,
    description: Option<String>,
}

/// Address from ChainAbuse report
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields may be used in future or are part of external API
struct ChainAbuseAddress {
    address: String,
    chain: String,
    domain: Option<String>,
}

/// Represents a report for a specific wallet with malicious activity details
#[derive(Debug, Serialize)]
pub struct MaliciousWalletReport {
    /// The wallet address
    pub address: String,
    /// Categories of malicious activity associated with the address
    pub categories: Vec<String>,
    /// Risk level assessment (Medium, High, Critical)
    pub risk_level: String,
    /// Number of reports found
    pub report_count: usize,
    /// Whether the address is already in our database
    pub in_database: bool,
    /// Notes and details from the reports
    pub details: Vec<ReportDetail>,
}

/// Details of a specific report
#[derive(Debug, Serialize)]
pub struct ReportDetail {
    /// Report identifier
    pub id: String,
    /// When the report was created
    pub created_at: String,
    /// Report category
    pub category: String,
    /// Whether the report is from a trusted source
    pub trusted: bool,
    /// Report description
    pub description: Option<String>,
}

/// Create authorized HTTP client for ChainAbuse API access
///
/// This function handles the setup of the HTTP client with proper authentication headers
async fn create_chainabuse_client(
    config: &Config,
) -> HackerdexResult<(reqwest::Client, HeaderMap)> {
    // Check if API key exists
    let api_key = match &config.chainabuse_api_key {
        Some(key) => key,
        None => {
            return Err(HackerdexError::ConfigError(
                "ChainAbuse API key not found in configuration".to_string(),
            ));
        }
    };

    // Setup HTTP client with authorization
    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();

    // Format the header with the API key - encode the key in the format "key:key" as required by ChainAbuse
    let auth_value = format!("{}:{}", api_key, api_key); // Format as "key:key"
    let encoded_auth = general_purpose::STANDARD.encode(auth_value); // Base64 encode

    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Basic {}", encoded_auth))
            .map_err(|e| HackerdexError::Other(format!("Invalid API key format: {}", e)))?,
    );

    // Add accept header for JSON responses
    headers.insert(
        reqwest::header::ACCEPT,
        HeaderValue::from_static("application/json"),
    );

    Ok((client, headers))
}

/// Lookup a specific Solana wallet address in the ChainAbuse API
///
/// This function checks if a specific wallet address is associated with any
/// malicious activity reports in the ChainAbuse database. It returns a report
/// with detailed information about any findings.
///
/// # Arguments
///
/// * `config` - Application configuration containing ChainAbuse API key
/// * `pool` - Database connection pool
/// * `wallet_address` - The Solana wallet address to check
///
/// # Returns
///
/// An Option containing MaliciousWalletReport if the wallet is found in any reports,
/// or None if no malicious activity is found for the wallet.
pub async fn lookup_wallet_address(
    config: &Config,
    pool: &PgPool,
    wallet_address: &str,
) -> HackerdexResult<Option<MaliciousWalletReport>> {
    // Create client with proper authorization
    let (client, headers) = create_chainabuse_client(config).await?;

    // Make request to ChainAbuse API specifically for this address
    // Using the search endpoint with the address parameter
    let response = client
        .get("https://api.chainabuse.com/v0/reports")
        .query(&[
            ("address", wallet_address), // The specific address we're looking for
            ("blockchain", "SOLANA"),    // Filter by Solana blockchain
            ("includePrivate", "false"), // Only public reports
        ])
        .headers(headers)
        .send()
        .await
        .map_err(|e| HackerdexError::Other(format!("Failed to fetch from ChainAbuse: {}", e)))?;

    // Check if request was successful
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        // Save error response for later analysis
        save_response_text(&error_text, wallet_address, "error");

        return Err(HackerdexError::Other(format!(
            "ChainAbuse API returned error status {}: {}",
            status, error_text
        )));
    }

    // Get response text first so we can save it and inspect it in case of errors
    let response_text = response
        .text()
        .await
        .map_err(|e| HackerdexError::DataParsing(format!("Failed to get response text: {}", e)))?;

    // Save the raw response for debugging and avoiding repeat API calls
    save_response_text(&response_text, wallet_address, "response");

    if response_text.trim().is_empty() || response_text == "[]" {
        // No reports found
        return Ok(None);
    }

    // Parse JSON response directly from the saved text
    let reports: Vec<ChainAbuseReport> = match serde_json::from_str(&response_text) {
        Ok(reports) => reports,
        Err(e) => {
            // Log the error and the response text for debugging
            eprintln!("Error parsing ChainAbuse response: {}", e);
            eprintln!("Response text: {}", response_text);

            // Try to salvage what we can from the response
            // Sometimes the API returns data in a different format than expected
            match serde_json::from_str::<serde_json::Value>(&response_text) {
                Ok(value) => {
                    if let Some(reports_array) = value.as_array() {
                        let mut parsed_reports = Vec::new();

                        for report_value in reports_array {
                            // Try to parse each report manually
                            if let Some(report) =
                                parse_report_from_value(report_value, wallet_address)
                            {
                                parsed_reports.push(report);
                            }
                        }

                        if !parsed_reports.is_empty() {
                            parsed_reports
                        } else {
                            return Err(HackerdexError::DataParsing(format!(
                                "Failed to parse any reports from response: {}",
                                e
                            )));
                        }
                    } else {
                        return Err(HackerdexError::DataParsing(format!(
                            "Response is not an array: {}",
                            response_text
                        )));
                    }
                }
                Err(_) => {
                    return Err(HackerdexError::DataParsing(format!(
                        "Failed to parse ChainAbuse API response: {}",
                        e
                    )));
                }
            }
        }
    };

    // Check if any reports were found
    if reports.is_empty() {
        // No reports found for this address
        return Ok(None);
    }

    // Double check that the wallet appears in any reports (should always be true given our query)
    let matching_reports: Vec<&ChainAbuseReport> = reports
        .iter()
        .filter(|report| {
            report.addresses.iter().any(|addr| {
                (addr.chain.to_uppercase() == "SOL" || addr.chain.to_uppercase() == "SOLANA")
                    && addr.address == wallet_address
            })
        })
        .collect();

    if matching_reports.is_empty() {
        // No reports found for this address (this is a safeguard check)
        return Ok(None);
    }

    // Check if address is already in our database
    let in_database = get_address_details(pool, wallet_address).await.is_ok();

    // Extract unique categories from all reports
    let mut categories = HashSet::new();
    let mut highest_risk = "Medium".to_string(); // Default risk level

    // Process reports to build the response
    let report_details: Vec<ReportDetail> = matching_reports
        .iter()
        .map(|report| {
            // Get category and risk level for this report
            let (category, risk_level) = categorize_report(report);
            categories.insert(category.clone());

            // Update highest risk level if needed
            if risk_level == "Critical" || (risk_level == "High" && highest_risk != "Critical") {
                highest_risk = risk_level;
            }

            // Build report detail
            ReportDetail {
                id: report.id.clone(),
                created_at: report.created_at.clone(),
                category: report
                    .scam_category
                    .clone()
                    .unwrap_or(report.report_type.clone()),
                trusted: report.trusted.unwrap_or(false),
                description: report.description.clone(),
            }
        })
        .collect();

    // Create final wallet report
    let wallet_report = MaliciousWalletReport {
        address: wallet_address.to_string(),
        categories: categories.into_iter().collect(),
        risk_level: highest_risk,
        report_count: matching_reports.len(),
        in_database,
        details: report_details,
    };

    Ok(Some(wallet_report))
}

/// Helper function to save API responses to files
fn save_response_text(response_text: &str, wallet_address: &str, response_type: &str) {
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;

    // Create responses directory if it doesn't exist
    let dir_path = "api_responses";
    let _ = fs::create_dir_all(dir_path);

    // Create filename based on wallet address and response type
    let filename = format!(
        "{}/chainabuse_{}_{}.json",
        dir_path, response_type, wallet_address
    );
    let path = Path::new(&filename);

    // Write response to file
    if let Ok(mut file) = File::create(path) {
        let _ = file.write_all(response_text.as_bytes());
    }
}

/// Helper function to manually parse a report from a JSON value
/// This is used as a fallback when automatic deserialization fails
fn parse_report_from_value(
    value: &serde_json::Value,
    wallet_address: &str,
) -> Option<ChainAbuseReport> {
    // Try to extract the basic fields from the JSON value
    let id = value.get("id")?.as_str()?.to_string();
    let created_at = value.get("createdAt")?.as_str()?.to_string();

    // Get optional fields
    let trusted = value.get("trusted").and_then(|v| v.as_bool());
    let checked = value.get("checked").and_then(|v| v.as_bool());

    // Try to get report_type from different possible fields
    let report_type = value
        .get("reportType")
        .or_else(|| value.get("type"))
        .or_else(|| value.get("scamCategory"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();

    let scam_category = value
        .get("scamCategory")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let entity_name = value
        .get("entityName")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let description = value
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Parse tags
    let tags = value.get("tags").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|t| t.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>()
    });

    // Parse addresses
    let mut addresses = Vec::new();

    // If there's an addresses array, parse it
    if let Some(addr_array) = value.get("addresses").and_then(|v| v.as_array()) {
        for addr_value in addr_array {
            if let Some(addr_obj) = addr_value.as_object() {
                if let (Some(addr), Some(chain)) = (
                    addr_obj.get("address").and_then(|v| v.as_str()),
                    addr_obj
                        .get("chain")
                        .or_else(|| addr_obj.get("blockchain"))
                        .and_then(|v| v.as_str()),
                ) {
                    let domain = addr_obj
                        .get("domain")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    addresses.push(ChainAbuseAddress {
                        address: addr.to_string(),
                        chain: chain.to_string(),
                        domain,
                    });
                }
            }
        }
    }

    // If no addresses were found in the array but our wallet address is the one we're looking for,
    // create a synthetic address entry
    if addresses.is_empty() {
        addresses.push(ChainAbuseAddress {
            address: wallet_address.to_string(),
            chain: "SOL".to_string(),
            domain: None,
        });
    }

    // Construct the report
    Some(ChainAbuseReport {
        id,
        created_at,
        trusted,
        checked,
        scam_category,
        entity_name,
        report_type,
        addresses,
        tags,
        description,
    })
}

/// Fetch malicious Solana addresses from ChainAbuse and store them in the database
///
/// This function queries the ChainAbuse API for reports related to Solana addresses,
/// filters for malicious activity, and stores the results in the database.
///
/// Note: According to task 2.7, this function should be called directly, not by another method,
/// as the ChainAbuse API is limited to 10 calls per month.
pub async fn fetch_malicious_solana_addresses(
    config: &Config,
    pool: &PgPool,
) -> HackerdexResult<usize> {
    // Create client with proper authorization
    let (client, headers) = create_chainabuse_client(config).await?;

    // Make request to ChainAbuse API
    // Using endpoint for reports with Solana addresses and proper API version (v0)
    let response = client
        .get("https://api.chainabuse.com/v0/reports")
        .query(&[
            ("blockchain", "SOLANA"),
            ("includePrivate", "false"),
            ("page", "1"),
            ("perPage", "50"),
        ])
        .headers(headers)
        .send()
        .await
        .map_err(|e| HackerdexError::Other(format!("Failed to fetch from ChainAbuse: {}", e)))?;

    // Check if request was successful
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(HackerdexError::Other(format!(
            "ChainAbuse API returned error status {}: {}",
            status, error_text
        )));
    }

    // Parse response
    let reports: Vec<ChainAbuseReport> = response.json().await.map_err(|e| {
        HackerdexError::DataParsing(format!("Failed to parse ChainAbuse API response: {}", e))
    })?;

    // Process reports and add addresses to database
    let mut added_count = 0;

    for report in reports {
        // Filter for Solana addresses only
        let solana_addresses = report
            .addresses
            .iter()
            .filter(|addr| addr.chain.to_uppercase() == "SOL")
            .collect::<Vec<_>>();

        if solana_addresses.is_empty() {
            continue;
        }

        // Determine risk level and category based on report type
        let (category, risk_level) = categorize_report(&report);

        // Extract notes from report
        let notes = format!(
            "Report type: {}\nTags: {}\nDescription: {}",
            report.report_type,
            report.tags.clone().unwrap_or_default().join(", "),
            report
                .description
                .clone()
                .unwrap_or_else(|| "No description available".to_string())
        );

        // Create entity name
        let entity_name = report
            .entity_name
            .clone()
            .unwrap_or_else(|| format!("Malicious Actor - {}", report.report_type));

        // Add each address to the database
        for addr in solana_addresses {
            let address_data = AddressData {
                address: addr.address.clone(),
                entity_name: entity_name.clone(),
                category: category.clone(),
                risk_level: risk_level.clone(),
                source_of_info: "ChainAbuse API".to_string(),
                confidence_score: 4, // High confidence as this comes from a specialized reporting platform
                notes: Some(notes.clone()),
            };

            // Add to database, ignoring duplicates
            match add_known_address(pool, &address_data).await {
                Ok(_) => {
                    added_count += 1;
                }
                Err(HackerdexError::DatabaseError(e)) if e.contains("duplicate key") => {
                    // Duplicate entry, continue silently
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }

    Ok(added_count)
}

/// Categorize a ChainAbuse report into risk category and level
fn categorize_report(report: &ChainAbuseReport) -> (String, String) {
    // If the scam_category is available, use that for categorization
    if let Some(scam_cat) = &report.scam_category {
        let scam_cat_lower = scam_cat.to_lowercase();

        match scam_cat_lower.as_str() {
            "contract_exploit" | "other_hack" => {
                ("Known Hacker".to_string(), "Critical".to_string())
            }

            "money_laundering" | "sanctions_violation" => {
                ("Money Laundering".to_string(), "Critical".to_string())
            }

            "ransomware" => ("Ransomware".to_string(), "Critical".to_string()),

            "phishing" | "impersonation" | "nft_scam" | "romance_scam" => {
                ("Scam".to_string(), "High".to_string())
            }

            "rug_pull" | "fake_project" => ("Exit Scam".to_string(), "Critical".to_string()),

            "stolen_funds" => ("Stolen Funds".to_string(), "High".to_string()),

            "market_manipulation" => ("Market Manipulation".to_string(), "High".to_string()),

            _ => {
                // Fallback to report type if scam category doesn't match known categories
                categorize_by_report_type(report)
            }
        }
    } else {
        // Fall back to report type if scam category is not available
        categorize_by_report_type(report)
    }
}

/// Categorize a report based on report_type when scam_category is not available
fn categorize_by_report_type(report: &ChainAbuseReport) -> (String, String) {
    let report_type = report.report_type.to_lowercase();

    // Match report type to appropriate category and risk level
    match report_type.as_str() {
        "hack" | "exploit" | "smart contract exploit" => {
            ("Known Hacker".to_string(), "Critical".to_string())
        }

        "money laundering" | "sanctions violation" => {
            ("Money Laundering".to_string(), "Critical".to_string())
        }

        "ransomware" => ("Ransomware".to_string(), "Critical".to_string()),

        "scam" | "fraudulent activity" | "phishing" | "impersonation" => {
            ("Scam".to_string(), "High".to_string())
        }

        "stolen funds" => ("Stolen Funds".to_string(), "High".to_string()),

        "market manipulation" => ("Market Manipulation".to_string(), "High".to_string()),

        _ => {
            // Check for keywords in tags or description for further categorization
            let combined_text = format!(
                "{} {} {}",
                report.report_type,
                report.tags.clone().unwrap_or_default().join(" "),
                report.description.clone().unwrap_or_default()
            )
            .to_lowercase();

            if combined_text.contains("exit scam") || combined_text.contains("rug pull") {
                ("Exit Scam".to_string(), "Critical".to_string())
            } else if combined_text.contains("mixer") || combined_text.contains("tumbler") {
                ("Mixer".to_string(), "High".to_string())
            } else {
                // Default category for other types
                ("Other Malicious Activity".to_string(), "Medium".to_string())
            }
        }
    }
}

/// Add a wallet address to the database with information from the ChainAbuse API
///
/// This function is used when a specific wallet is found in reports and we want to
/// add it to our database.
pub async fn add_wallet_to_database(
    pool: &PgPool,
    wallet_report: &MaliciousWalletReport,
) -> HackerdexResult<()> {
    // If the address is already in the database, don't add it again
    if wallet_report.in_database {
        return Ok(());
    }

    // Determine overall category based on reported categories
    let category = if wallet_report
        .categories
        .contains(&"Known Hacker".to_string())
    {
        "Known Hacker"
    } else if wallet_report
        .categories
        .contains(&"Money Laundering".to_string())
    {
        "Money Laundering"
    } else if wallet_report.categories.contains(&"Exit Scam".to_string()) {
        "Exit Scam"
    } else if wallet_report.categories.contains(&"Scam".to_string()) {
        "Scam"
    } else {
        "Other Malicious Activity"
    };

    // Create notes from report details
    let notes = wallet_report
        .details
        .iter()
        .map(|detail| {
            format!(
                "Report ID: {}\nCreated: {}\nCategory: {}\nTrusted Source: {}\nDescription: {}",
                detail.id,
                detail.created_at,
                detail.category,
                detail.trusted,
                detail
                    .description
                    .clone()
                    .unwrap_or_else(|| "No description".to_string())
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    // Create address data
    let address_data = AddressData {
        address: wallet_report.address.clone(),
        entity_name: format!("Malicious Actor - {} Reports", wallet_report.report_count),
        category: category.to_string(),
        risk_level: wallet_report.risk_level.clone(),
        source_of_info: "ChainAbuse API".to_string(),
        confidence_score: 4, // High confidence from specialized reporting platform
        notes: Some(notes),
    };

    // Add to database
    add_known_address(pool, &address_data).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_report() {
        // Test hack report with scam_category
        let hack_report = ChainAbuseReport {
            id: "test-id".to_string(),
            created_at: "2023-01-01T00:00:00Z".to_string(),
            trusted: Some(true),
            checked: None,
            scam_category: Some("CONTRACT_EXPLOIT".to_string()),
            entity_name: Some("Test Hacker".to_string()),
            report_type: "Hack".to_string(),
            addresses: vec![],
            tags: Some(vec!["defi".to_string(), "exploit".to_string()]),
            description: Some("Smart contract exploit".to_string()),
        };

        let (category, risk_level) = categorize_report(&hack_report);
        assert_eq!(category, "Known Hacker");
        assert_eq!(risk_level, "Critical");

        // Test scam report with report_type only
        let scam_report = ChainAbuseReport {
            id: "test-id".to_string(),
            created_at: "2023-01-01T00:00:00Z".to_string(),
            trusted: Some(false),
            checked: None,
            scam_category: None,
            entity_name: Some("Test Scammer".to_string()),
            report_type: "Scam".to_string(),
            addresses: vec![],
            tags: Some(vec!["nft".to_string()]),
            description: Some("NFT scam".to_string()),
        };

        let (category, risk_level) = categorize_report(&scam_report);
        assert_eq!(category, "Scam");
        assert_eq!(risk_level, "High");

        // Test rug pull with scam_category
        let rug_pull_report = ChainAbuseReport {
            id: "test-id".to_string(),
            created_at: "2023-01-01T00:00:00Z".to_string(),
            trusted: Some(true),
            checked: None,
            scam_category: Some("RUG_PULL".to_string()),
            entity_name: Some("Unknown".to_string()),
            report_type: "Other".to_string(),
            addresses: vec![],
            tags: Some(vec!["nft".to_string()]),
            description: Some("Project exit scam".to_string()),
        };

        let (category, risk_level) = categorize_report(&rug_pull_report);
        assert_eq!(category, "Exit Scam");
        assert_eq!(risk_level, "Critical");
    }
}
