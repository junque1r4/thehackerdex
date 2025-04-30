use crate::error::{HackerdexError, HackerdexResult};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction, UiInstruction, UiMessage,
    UiParsedInstruction, UiTransactionTokenBalance, option_serializer::OptionSerializer,
};
use std::collections::HashSet;
use std::str::FromStr;
use tracing::debug;

/// Represents a parsed transaction with relevant information extracted
#[derive(Debug, Clone)]
pub struct ParsedTransaction {
    /// The transaction signature
    pub signature: String,
    /// The program IDs interacted with in this transaction
    pub program_ids: Vec<String>,
    /// The accounts involved in this transaction
    pub involved_accounts: Vec<String>,
    /// Token balances before transaction execution
    pub pre_token_balances: Vec<UiTransactionTokenBalance>,
    /// Token balances after transaction execution
    pub post_token_balances: Vec<UiTransactionTokenBalance>,
    /// Information about transaction execution
    pub execution_status: Option<String>,
    /// Transaction execution fee in lamports
    pub fee: u64,
}

/// Extracts all unique program IDs from a transaction
///
/// This function takes a transaction and returns a vector of all program IDs
/// that were interacted with in that transaction.
///
/// # Arguments
///
/// * `transaction` - The transaction to extract program IDs from
///
/// # Returns
///
/// A vector of program ID strings
pub fn extract_program_ids(transaction: &EncodedConfirmedTransactionWithStatusMeta) -> Vec<String> {
    let mut program_ids = HashSet::new();

    let tx = &transaction.transaction;
    if let EncodedTransaction::Json(ui_tx) = &tx.transaction {
        // UiMessage is not an Option, so directly pass it to collect_program_ids_from_message, lol
        collect_program_ids_from_message(&ui_tx.message, &mut program_ids);
    }

    // Convert the HashSet to a Vec for the return value
    program_ids.into_iter().collect()
}

/// Extracts all involved accounts from a transaction
///
/// # Arguments
///
/// * `transaction` - The transaction to extract accounts from
///
/// # Returns
///
/// A vector of account address strings
pub fn extract_involved_accounts(
    transaction: &EncodedConfirmedTransactionWithStatusMeta,
) -> Vec<String> {
    let mut accounts = HashSet::new();

    let tx = &transaction.transaction;
    if let EncodedTransaction::Json(ui_tx) = &tx.transaction {
        match &ui_tx.message {
            UiMessage::Parsed(parsed_message) => {
                // Add all account keys
                for account_key in &parsed_message.account_keys {
                    accounts.insert(account_key.pubkey.clone());
                }
            }
            UiMessage::Raw(raw_message) => {
                // Add all account keys from raw message
                for account_key in &raw_message.account_keys {
                    accounts.insert(account_key.clone());
                }
            }
        }
    }

    // Convert the HashSet to a Vec for the return value
    accounts.into_iter().collect()
}

/// Parses a transaction and extracts relevant information
///
/// # Arguments
///
/// * `transaction` - The transaction to parse
/// * `signature` - The transaction signature
///
/// # Returns
///
/// A `HackerdexResult` containing a `ParsedTransaction` or an error
pub fn parse_transaction(
    transaction: &EncodedConfirmedTransactionWithStatusMeta,
    signature: &str,
) -> HackerdexResult<ParsedTransaction> {
    // Extract program IDs
    let program_ids = extract_program_ids(transaction);

    // Extract involved accounts
    let involved_accounts = extract_involved_accounts(transaction);

    // Extract token balances and other metadata
    let pre_token_balances: Vec<UiTransactionTokenBalance> = transaction
        .transaction
        .meta
        .as_ref()
        .map(|meta| match meta.pre_token_balances {
            OptionSerializer::Some(ref balances) => balances.clone(),
            OptionSerializer::None | OptionSerializer::Skip => Vec::new(),
        })
        .unwrap_or_default();

    let post_token_balances: Vec<UiTransactionTokenBalance> = transaction
        .transaction
        .meta
        .as_ref()
        .map(|meta| match meta.post_token_balances {
            OptionSerializer::Some(ref balances) => balances.clone(),
            OptionSerializer::None | OptionSerializer::Skip => Vec::new(),
        })
        .unwrap_or_default();

    // Extract execution status
    let execution_status = transaction
        .transaction
        .meta
        .as_ref()
        .and_then(|meta| Some(format!("{:?}", meta.status)));

    // Extract fee
    let fee = transaction
        .transaction
        .meta
        .as_ref()
        .map(|meta| meta.fee)
        .unwrap_or_default();

    Ok(ParsedTransaction {
        signature: signature.to_string(),
        program_ids,
        involved_accounts,
        pre_token_balances,
        post_token_balances,
        execution_status,
        fee,
    })
}

/// Collects all program IDs from a transaction message
fn collect_program_ids_from_message(message: &UiMessage, program_ids: &mut HashSet<String>) {
    match message {
        UiMessage::Parsed(parsed_message) => {
            // For each instruction in the message, extract the program ID
            for instruction in &parsed_message.instructions {
                match instruction {
                    UiInstruction::Parsed(UiParsedInstruction::Parsed(parsed_instr)) => {
                        // For parsed instructions with program ID
                        program_ids.insert(parsed_instr.program_id.clone());

                        // Check if it's a system program or other known program by name
                        let program = &parsed_instr.program;
                        match program.as_str() {
                            "System Program" => {
                                program_ids.insert("11111111111111111111111111111111".to_string());
                            }
                            "Vote Program" => {
                                program_ids.insert(
                                    "Vote111111111111111111111111111111111111111".to_string(),
                                );
                            }
                            "Stake Program" => {
                                program_ids.insert(
                                    "Stake11111111111111111111111111111111111111".to_string(),
                                );
                            }
                            "SPL Token" => {
                                program_ids.insert(
                                    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
                                );
                            }
                            _ => {}
                        }
                    }
                    UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(partial)) => {
                        // For partially decoded instructions, we can get the program ID directly
                        program_ids.insert(partial.program_id.clone());
                    }
                    UiInstruction::Compiled(_) => {
                        // Cannot directly extract program ID from compiled instructions
                        // Would need message context to resolve the program ID index
                        debug!(
                            "Skipping compiled instruction - cannot extract program ID directly"
                        );
                    }
                }
            }
        }
        UiMessage::Raw(raw_message) => {
            // For raw messages, we need to use the instruction's program_id_index to get the program ID
            for instruction in &raw_message.instructions {
                let program_idx = instruction.program_id_index as usize;
                if program_idx < raw_message.account_keys.len() {
                    program_ids.insert(raw_message.account_keys[program_idx].clone());
                }
            }
        }
    }
}

/// Validates a Solana address (pubkey)
///
/// # Arguments
///
/// * `address` - The address to validate
///
/// # Returns
///
/// A `HackerdexResult` with `()` if valid, otherwise an error
pub fn validate_address(address: &str) -> HackerdexResult<()> {
    match Pubkey::from_str(address) {
        Ok(_) => Ok(()),
        Err(_) => Err(HackerdexError::InvalidAddress(format!(
            "Invalid Solana address: {}",
            address
        ))),
    }
}

/// Gets a program's address by its name
///
/// Returns the base-58 encoded address of well-known Solana programs.
/// These are the actual program addresses on the Solana blockchain,
/// not mock data.
pub fn get_program_address_by_name(name: &str) -> Option<String> {
    match name {
        // Core Solana programs
        "System Program" => Some("11111111111111111111111111111111".to_string()),
        "Vote Program" => Some("Vote111111111111111111111111111111111111111".to_string()),
        "Stake Program" => Some("Stake11111111111111111111111111111111111111".to_string()),
        "BPF Loader" => Some("BPFLoader1111111111111111111111111111111111".to_string()),
        "BPF Loader Upgradeable" => Some("BPFLoaderUpgradeab1e11111111111111111111111".to_string()),
        "BPF Loader Deprecated" => Some("BPFLoader2111111111111111111111111111111111".to_string()),

        // SPL Programs
        "SPL Token" => Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string()),
        "SPL Associated Token Account" => {
            Some("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".to_string())
        }
        "SPL Memo" => Some("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr".to_string()),
        "SPL Name Service" => Some("namesLPneVptA9Z5rqUDD9tMTWEJwofgaYwp8cawRkX".to_string()),

        // Other popular programs
        "Metaplex Token Metadata" => {
            Some("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s".to_string())
        }
        "Jupiter Aggregator" => Some("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4".to_string()),
        "Marinade Staking" => Some("MarBmsSgKXdrN1egZf5sqe1TMai9K1rChYNDJgjq7aD".to_string()),
        "Magic Eden v2" => Some("M2mx93ekt1fmXSVkTrUL9xVFHkmME8HTUi5Cyc5aF7K".to_string()),

        _ => None,
    }
}
