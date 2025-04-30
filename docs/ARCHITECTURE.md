# Hackerdex - Architecture Overview

This document provides a comprehensive overview of the Hackerdex project architecture, designed to help engineers and AI agents quickly understand the system and implement new features efficiently.

## Project Purpose

Hackerdex is a Solana address rank system for classifying wallet behavior and assessing potential risk factors. It analyzes on-chain data and identifies patterns associated with different types of actors (e.g., DEX traders, bridge users, potential illicit actors).

## Core Components

### 1. RPC Client (`src/rpc/`)

Responsible for interacting with the Solana blockchain, fetching transaction history, account information, and other on-chain data.

**Key Features:**
- Rate-limiting implementation to respect Solana RPC endpoint constraints
- Methods for fetching account info, transactions, and signatures
- Error handling for RPC-specific errors
- Exponential backoff with jitter for handling rate limit errors

**Key Components:**
- `client.rs`: Contains the `RateLimitedClient` implementation that wraps Solana's RPC client
  - Implements methods like `get_balance()`, `get_signatures_for_address()`, `get_transaction()`
  - Handles rate limiting using the `governor` crate
  - Implements retry logic with backoff for transient failures

### 2. Database (`src/db/`)

Stores and manages known addresses with risk-relevant context.

**Schema:**
- `address`: Primary Key (Solana address)
- `entity_name`: Name of the entity (e.g., "Wormhole", "Jupiter", "Known Hacker X Wallet")
- `category`: Category type (e.g., "Bridge Contract", "DEX Router", "Known Hacker")
- `risk_level`: Risk assessment (e.g., "Low", "Medium", "High", "Critical")
- `source_of_info`: Source of the address information
- `confidence_score`: How certain the label/risk is (1-5)
- `notes`: Additional context information
- `created_at`: Timestamp when the record was created
- `updated_at`: Timestamp when the record was last updated

**Key Components:**
- `models.rs`: Defines database models and types
  - `AddressData`: Structure for address input data
  - `AddressRecord`: Represents a record in the database
  - `SerializableAddressRecord`: JSON-serializable version of AddressRecord
- `repository.rs`: Provides data access and manipulation functions
  - `Repository`: Struct that provides an interface to the database operations
  - `initialize_db()`: Sets up database connection and tables
  - `add_known_address()`: Adds new address records
  - `get_address_details()`: Retrieves details for a specific address
  - `update_address_details()`: Updates fields for existing records
  - `get_all_addresses_by_category()`: Retrieves addresses by category

### 3. Analysis Engine (`src/analysis/`)

Processes transactions and identifies patterns of interest.

**Components:**
- `transaction_parser.rs`: Parses raw transaction data into structured format
  - `ParsedTransaction`: Structured representation of transaction data
  - Contains fields like `signature`, `program_ids`, `involved_accounts`
- `program_analyzer.rs`: Analyzes program IDs in transactions
  - Identifies known programs and their risk levels
  - Maps program IDs to known entities
- `wallet_analyzer.rs`: Analyzes wallet addresses in transactions
  - Identifies known wallet addresses and their risk levels
  - Maps addresses to known entities
- `transaction_analysis.rs`: Coordinates analysis components and combines results
  - `TransactionAnalysisData`: Main structure containing all analysis results
  - `perform_initial_analysis()`: Entry point for analysis

**Key Data Structures:**
- `ParsedTransaction`: Structured format of transaction data
- `ProgramAnalysis`: Analysis results for programs in transactions
- `WalletDirectAnalysisResult`: Analysis results for wallets in transactions
- `TransactionAnalysisData`: Combined analysis data including program and wallet analysis
- `HeuristicFlags`: Contains boolean and numeric results from heuristic checks

### 4. Heuristic Engine (`src/heuristic_engine/`)

Implements heuristic checks for risk detection.

**Components:**
- `types.rs`: Defines data structures for heuristic analysis
  - `HeuristicFlags`: Stores results of heuristic checks
  - `WalletContext`: Contains wallet analysis context
  - `WalletTransaction`: Represents transaction in wallet context
- `wallet_heuristics.rs`: Implements wallet-based heuristic checks
  - `check_wallet_age_and_activity()`: Detects suspicious new wallets
  - `check_high_frequency_volume()`: Identifies high-frequency trading patterns
  - `check_structuring_patterns()`: Detects potential structuring behaviors
  - `check_pass_through()`: Identifies pass-through wallet behavior
  - `analyze_funding_sources()`: Analyzes wallet funding sources for risks
  - `analyze_spending_destinations()`: Analyzes wallet spending for risks
- `transaction_heuristics.rs`: Implements transaction-based heuristic checks
  - `check_direct_illicit_interaction()`: Detects interactions with known illicit addresses
  - `check_interaction_with_risky_categories()`: Identifies interactions with risky address categories
  - `check_rapid_dispersal()`: Detects one-to-many fund transfers
  - `check_fund_consolidation()`: Detects many-to-one fund transfers
- `context_builder.rs`: Builds wallet context for heuristic analysis
  - `build_wallet_contexts()`: Creates wallet contexts with on-chain and database data
  - Handles RPC rate limits when fetching on-chain data
- `mod.rs`: Orchestrates heuristic analysis
  - `run_all_heuristics()`: Runs all heuristic checks on a wallet context
  - `run_heuristics()`: Main entry point for heuristic analysis

**Key Functions:**
- `run_all_heuristics()`: Runs individual heuristic functions and combines results
- `run_heuristics()`: Orchestrates fetching wallet contexts and running heuristics

### 5. Command-Line Interface (`src/bin/`)

Provides user interface for analyzing wallets and transactions.

**Key Commands:**
- `analyze-wallet <ADDRESS>`: Analyzes a single wallet address
- `analyze-file <FILE_PATH>`: Analyzes multiple wallet addresses from a file
- `monitor-entity <ENTITY_NAME/PROGRAM_ID>`: Monitors transactions for specific entities
- `db_import`: CLI tool for importing address data into the database

### 6. Error Handling (`src/error/`)

Centralized error types and handling for the application.

**Components:**
- `mod.rs`: Defines the error types and conversion implementations
  - `HackerdexError`: Custom error enum with variants for different error types
  - `HackerdexResult<T>`: Type alias for Result with HackerdexError
  - Implementations for converting from other error types (e.g., `From<sqlx::Error>`)

**Error Types:**
- `RpcError`: Errors from RPC operations
- `InvalidAddress`: Invalid Solana address format
- `RateLimit`: Rate limit exceeded
- `Config`: Configuration errors
- `DatabaseError`: Database operation errors
- `NotFound`: Entity not found
- `DataParsing`: Data parsing errors
- `AnalysisError`: Analysis process errors
- `Io`: I/O errors
- `Other`: Generic errors

### 7. Configuration (`src/config/`)

Manages application configuration settings.

**Components:**
- Environment variables loading
- Default values for various settings
- Configuration for RPC endpoints, rate limits, database connections

## Data Flow

1. **Input**: User provides wallet addresses or transaction signatures via CLI
2. **RPC Fetching**: System fetches on-chain data for the provided addresses/transactions
   - Uses `RateLimitedClient` to respect Solana RPC rate limits
   - Handles retries and backoff for transient failures
3. **Initial Analysis**: Transaction parsing and direct reference against known addresses
   - `ParsedTransaction` is created from raw transaction data
   - `perform_initial_analysis()` performs initial wallet and program checks
4. **Context Building**: System builds contextual information about involved wallets
   - `build_wallet_contexts()` fetches on-chain data for involved wallets
   - Cross-references with known address database
5. **Heuristic Analysis**: Various heuristic checks are run against the data
   - `run_heuristics()` orchestrates the process
   - `run_all_heuristics()` executes individual heuristic functions
6. **Risk Scoring**: Results are combined into an overall risk assessment
   - `HeuristicFlags` stores individual heuristic results
   - `get_overall_suspicion_score()` calculates combined risk score
7. **Output**: Analysis results are formatted and presented to the user
   - CLI formats and displays results
   - Detailed information about triggered heuristics is provided

## Database Structure

The database uses PostgreSQL with the following schema:

```sql
CREATE TABLE known_addresses (
    address TEXT PRIMARY KEY,
    entity_name TEXT NOT NULL,
    category TEXT NOT NULL,
    risk_level TEXT NOT NULL,
    source_of_info TEXT NOT NULL,
    confidence_score INTEGER NOT NULL,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## Development Guidelines

1. **Rate Limiting**: Always respect Solana RPC rate limits in any code that makes RPC calls
   - Use the `RateLimitedClient` or similar mechanisms for all RPC interactions
   - Implement backoff strategies for failures
2. **Testing**: All new functionality should have corresponding tests
   - Unit tests for individual functions
   - Integration tests for component interactions
   - Mock RPC responses for consistent test behavior
3. **Error Handling**: Use the centralized error types for consistent handling
   - Add new error types to `HackerdexError` as needed
   - Implement appropriate `From` traits for conversions
4. **Modularity**: Keep components focused on specific responsibilities
   - Follow the existing module structure
   - Avoid circular dependencies between modules
5. **Configuration**: Avoid hardcoding values; use configuration where appropriate
   - Use environment variables for configurable settings
   - Provide sensible defaults

## Implementing New Heuristics

When adding a new heuristic:

1. Identify the appropriate module (`wallet_heuristics.rs` or `transaction_heuristics.rs`)
2. Implement the heuristic function with appropriate parameters and return types
3. Add relevant fields to `HeuristicFlags` in `types.rs`
4. Update `run_all_heuristics()` in `mod.rs` to call your new heuristic
5. Update the risk factor description in `get_triggered_flags_description()`
6. Add tests for the new heuristic

## Implementation Progress

See `TASKS.md` for detailed implementation progress tracking. The project follows a phased approach:

1. Setup & Core Data Acquisition (mostly complete)
2. Analysis Engine & Heuristics (in progress)
   - Basic heuristic implementation complete
   - Working on heuristic application logic
3. Interface & Crawler (planned)
   - CLI tools for analysis and data management
   - Background crawler for monitoring
4. Testing, Refinement & Application (planned)
   - Comprehensive test suite
   - Performance optimization
   - Production deployment