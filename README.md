# HackerDex: Solana Risk Analysis & Exfiltration Pattern Detection

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**HackerDex** is an open-source Rust-based toolkit designed for in-depth analysis of Solana addresses and transactions. It aims to identify suspicious behavior patterns, assess risk factors, and detect potential fund exfiltration routes, contributing to a safer Solana ecosystem.

Born from a hackathon project, HackerDex has evolved with the vision of providing a collaborative, open-source platform for security researchers, developers, and blockchain analysts.

## Key Features

* **Comprehensive Transaction Analysis:** Parses transactions to identify involved programs, accounts, and token balance changes.
* **Known Address Database:** Cross-references addresses against a PostgreSQL database of known entities (programs, wallets, services) with associated categories and risk levels.
* **Heuristic Risk Engine:** Applies a suite of configurable heuristics to detect suspicious patterns, including:
    * **Financial Forensics:** Mixer Interaction, Bridge Hopping, Structuring Outflow, Drainer Consolidation, Peel Chain, Fund Churning.
    * **Behavioral Analysis:** High Frequency, Pass-Through, New Wallet Activity, Bidirectional Flow, Temporal Patterns (Automation).
    * **Risk Association:** Risky Funding Sources, Risky Spending Destinations, Direct Illicit Interactions.
* **Weighted Risk Scoring:** Combines heuristic results using configurable weights to generate a numerical risk score (0-10) and a categorical rating (Low, Medium, High, Critical).
* **Fund Tracing (Experimental):** Tools to trace fund flows forward from a source or backward from a destination (currently uses mock RPC data for tracing).
* **OSINT Integration:** Includes tools to query external threat intelligence sources like ChainAbuse and integrate findings into the local database.
* **Rate-Limited RPC Client:** Interacts safely with Solana RPC endpoints, respecting rate limits with configurable backoff.
* **Configurable & Extensible:** Leverages configuration files for analysis parameters, heuristic weights, and feature flags. Modular design allows for adding new heuristics.
* **Database Migrations:** Uses SQLx for managing PostgreSQL database schema evolution.

## Roadmap & Future Enhancements

HackerDex is an evolving project, and we envision several key areas for future development to enhance its capabilities and utility for the community:

* **Deeper SPL Token Analysis:** Extend analysis beyond SOL transfers to track the flow of specific SPL tokens, understand their origins (mints, swaps), and destinations, adding another layer to fund tracing.
* **Liquidity Calculation:** Integrate with on-chain data sources (DEX pools) and potentially off-chain APIs (CEXs) to estimate the *actual* liquidity available for specific assets along identified exfiltration routes. This would help quantify the feasibility of laundering specific amounts.
* **Expanded OSINT Integration:** Add support for more threat intelligence feeds beyond ChainAbuse to build a more comprehensive internal database of risky entities.
* **Real RPC Fund Tracing:** Replace the current mock/experimental fund tracing RPC calls (`get_outgoing_transfers`, `get_incoming_transfers`) with robust implementations that parse actual transaction history to accurately trace funds across hops.
* **Enhanced Heuristics:**
    * Refine existing heuristics based on real-world data and feedback.
    * Develop new heuristics for emerging patterns (e.g., specific DeFi exploit patterns, NFT wash trading indicators).
    * Improve Peel Chain detection by incorporating amount analysis alongside temporal patterns.
* **User Interface & Visualization:** Develop a web-based UI or integrate with existing graph visualization tools (e.g., Gephi, Cytoscape.js) to make exploring transaction data, fund flows, and risk assessments more intuitive.
* **Machine Learning Integration:** Explore using ML models trained on labeled data to identify complex or subtle patterns that rule-based heuristics might miss, or to optimize risk scoring weights.
* **Automated Reporting:** Implement functionality to automatically generate reports in specified formats (CSV, PDF) summarizing analysis findings, identified routes, and liquidity assessments, aligning with common investigation deliverables.
* **Monitoring & Alerting:** Enhance the `monitor` binary with real-time analysis capabilities and configurable alerting mechanisms for high-risk events.
* **Cross-Chain Capabilities:** Develop methods to track funds more effectively *across* different blockchains after they leave Solana via bridges.

We welcome contributions from the community to help achieve these goals. If you're interested in contributing, please check the [Issue Tracker](https://github.com/junque1r4/thehackerdex/issues) or start a discussion!


## Analysis Engine: Detected Patterns

HackerDex employs a range of heuristics to identify specific patterns often associated with illicit activities. The detailed methodology for each pattern is documented separately (See `/docs` folder

Detected patterns include:

* **Mixer Interaction:** Sending funds to known mixer/anonymizing services.
* **Bridge Hopping:** Rapidly moving funds across different blockchains using bridges, often combined with pass-through behavior.
* **Structuring Outflow:** Sending structured (multiple small) transactions to Centralized Exchanges (CEXs).
* **Drainer Consolidation:** Receiving a high proportion of funds from known drainer/exploit victims.
* **Peel Chain Exfiltration:** Sequentially "peeling off" small amounts while moving the bulk of funds through new addresses. (Requires `peel_chain_exfiltration_enabled` flag)
* **Fund Churning:** Repeatedly moving funds between a set of controlled addresses (excluding legitimate trading venues). (Requires `fund_churning_enabled` flag)
* **Automated Exfiltration Pattern:** Temporal analysis suggesting automated scripts based on transaction timing.
* **Bidirectional Flow:** Significant back-and-forth fund movement with specific counterparties (excluding legitimate trading venues).
* **Risky Funding/Spending:** High proportion of funds received from or sent to high-risk addresses.
* **Direct Illicit Interaction:** Transaction directly involves a known high/critical risk address or program.
* **New Wallet High Activity:** A recently created wallet exhibiting unusually high transaction count or volume.
* **High Frequency:** Abnormally high number of transactions or volume within a short period.
* *(Plus others based on custom flags and risk scoring)*

## Project Structure

The project is organized as a Cargo workspace:

```
. (Workspace Root: thehackerdex)
├── thehackerdex/                 # Core library crate
│   ├── api_responses/          # Saved OSINT API responses (debugging/rate limits)
│   ├── config/                 # Default configuration files for the library's CLI tools
│   │   ├── analyze_config.toml
│   │   ├── feature_flags.toml
│   │   ├── heuristic_weights.toml
│   │   └── monitoring.toml
│   ├── docs/                   # Detailed documentation for the library
│   ├── init-db/                # Initial SQL schema (reference only)
│   ├── migrations/             # SQLx database migration files for the library
│   ├── src/                    # Library source code
│   │   ├── analysis/
│   │   ├── bin/                # Executable CLI tools provided by the library
│   │   ├── config/             # Internal configuration logic for the library
│   │   ├── db/
│   │   ├── demo/
│   │   ├── discovery/
│   │   ├── error/
│   │   ├── heuristic_engine/
│   │   ├── osint/
│   │   ├── rpc/
│   │   ├── lib.rs              # Library root
│   │   └── main.rs             # Main executable for the 'thehackerdex' binary (if any)
│   └── Cargo.toml              # Manifest for the 'thehackerdex' library crate
├── thehackerdex-api/             # Axum API server crate
│   ├── src/                    # API server source code
│   │   ├── config.rs           # API specific configuration
│   │   ├── errors.rs           # API specific error handling
│   │   ├── handlers/           # API route handlers
│   │   ├── models.rs           # API request/response models
│   │   └── main.rs             # API server entry point
│   └── Cargo.toml              # Manifest for the 'thehackerdex-api' binary crate
├── .env                        # Environment variables for the workspace (DATABASE_URL, etc.)
├── .env.example                # Example environment file
├── .gitignore
├── Cargo.lock                  # Workspace lock file
├── Cargo.toml                  # Workspace manifest (defines members: thehackerdex, thehackerdex-api)
└── README.md                   # This file
```

## Getting Started

### Prerequisites

* **Rust & Cargo:** Install from [rustup.rs](https://rustup.rs/) (Latest stable version recommended).
* **PostgreSQL:** A running PostgreSQL server (v12+ recommended). Docker is a convenient option.
* **Solana RPC Endpoint:** Access to a Solana RPC node (e.g., QuickNode, Helius, Triton, or a public endpoint). Performance and rate limits vary significantly.
* **ChainAbuse API Key (Optional):** Required *only* for using the `lookup_wallet` tool with the `--query` flag or the `fetch_malicious_addresses` tool. Obtain from [ChainAbuse](https://chainabuse.com/).

### Installation & Setup

1.  **Clone the Repository:**
    ```bash
    git clone https://github.com/junque1r4/thehackerdex.git
    cd thehackerdex
    ```

2.  **Configure Environment (`.env`):**
    * Copy the example file: `cp .env.example .env`
    * Edit `.env` and set **at least** `DATABASE_URL`.
    * Optionally set `SOLANA_RPC_URL` (defaults to a public endpoint) and `CHAINABUSE_API`.
    ```dotenv
    # Example .env - Ensure these values match your local setup.
    # The DATABASE_URL, for instance, should match your PostgreSQL user, password, and database name.
    # If using the provided Docker setup, it might be: postgresql://hackerdex:hackerdex_password@localhost:5432/hackerdex
    DATABASE_URL=postgresql://user:password@localhost:5432/your_db_name
    SOLANA_RPC_URL=https://your-rpc-endpoint.com/
    CHAINABUSE_API=your_chainabuse_api_key
    ```

3.  **Setup Database:**
    * Ensure your PostgreSQL server is running.
    * Create the database specified in `DATABASE_URL`.
        ```bash
        # Example using psql
        psql -U user -h localhost -c "CREATE DATABASE hackerdex;"
        ```
    * **(Using Docker)** Alternatively, use the provided `docker-compose.yml` (if available) for a quick setup: `docker-compose up -d`

4.  **Build the Project:**
    ```bash
    cargo build --release # Use --release for optimized binaries
    ```
    This will compile all binaries in `src/bin/`.

5.  **Run Database Migrations:**
    * Migrations should run automatically the first time a tool interacts with the DB.
    * To verify the connection and run migrations manually:
        ```bash
        cargo run --release --bin db_check
        ```

### Populating the Database

The effectiveness of HackerDex relies heavily on the `known_addresses` table. In samples you'll find know programs, wallets and the blacklist from Kelsier Ventures. You can populate it using:

* **Manual Import:** Use the `db_import` tool with a CSV file.
    ```bash
    # CSV Format: address,entity_name,category,risk_level,source_of_info,confidence_score,notes
    cargo run --release --bin db_import -- path/to/your_addresses.csv
    ```
* **OSINT Fetching:** Use the `fetch_malicious_addresses` tool (requires `CHAINABUSE_API` key, **uses 1 API call**).
    ```bash
    cargo run --release --bin fetch_malicious_addresses
    ```
* **Specific Imports:** Use tools like `import_hacked_wallets` for specific formats if needed.

## Usage

HackerDex provides several command-line tools (binaries located in `target/release/` after building).

### 1. Analyzing Known Wallets (`analyze_known_wallets`)

This is the core analysis tool. It fetches wallets from the database based on criteria defined in `config/analyze_config.toml`, analyzes their historical transactions, applies heuristics, detects exfiltration patterns, and optionally updates the database notes with tags.

* **Configure Analysis:** Edit `thehackerdex/config/analyze_config.toml` (relative to the workspace root) to specify which wallets to analyze (e.g., by category, risk level, specific addresses, or `analyze_all = true`), history depth, concurrency, and exfiltration rule parameters.
* **Run Analysis:**
    ```bash
    cargo run --release --bin analyze_known_wallets
    ```
* **Output:** Logs analysis progress, detected patterns, and heuristic scores to the console. If `update_db_notes = true` in the config, it updates the `notes` field of analyzed wallets in the database with identified tags (e.g., `; ExfilTags [timestamp]: [Mixer Interaction, Structuring Outflow]`).

### 2. Looking Up a Specific Wallet (`lookup_wallet`)

Checks a single wallet against the local database and optionally ChainAbuse.

```bash
cargo run --release --bin lookup_wallet <WALLET_ADDRESS> [--query] [--save]
```

  * `<WALLET_ADDRESS>`: The Solana address to check.
  * `--query`: (Optional) Queries ChainAbuse if not found locally (**uses 1 API call**).
  * `--save`: (Optional) If `--query` finds a malicious report, saves/updates the address in the local database.

### 3. Tracing Funds (`run_tracer`) *(Experimental)*

Traces fund flows forward or backward from a starting address (Experimental - uses mock RPC data for tracing currently).

```bash
cargo run --release --bin run_tracer <MODE> [--source ADDRESS] [--hops COUNT]
```

  * `<MODE>`: `forward-track` (from source) or `backward-track` (to destination).
  * `--source ADDRESS`: (Optional) Specify the starting address. Defaults to the first high-risk address found in the DB.
  * `--hops COUNT`: (Optional) Max number of hops (default: 2).

### 4. Monitoring Transactions (`monitor`)

Runs a continuous monitor (using polling by default) based on `config/monitoring.toml` to watch for transactions involving specific programs or addresses. (Further development needed for alerting/actioning).

```bash
cargo run --release --bin monitor
```

## Configuration Files

  * `.env`: Located at the workspace root (`thehackerdex/.env`). Contains environment variables (DB URL, RPC URL, API Keys). **Crucial for basic operation.**
  * `thehackerdex/config/analyze_config.toml`: Controls the `analyze_known_wallets` CLI tool - which wallets to analyze, history depth, concurrency, and parameters for exfiltration rules (structuring thresholds, mixer categories, etc.).
  * `thehackerdex/config/heuristic_weights.toml`: Defines the weight (importance) of each heuristic flag for the CLI tools when calculating the final risk score. Allows tuning the risk assessment sensitivity.
  * `thehackerdex/config/feature_flags.toml`: Enables or disables specific, potentially resource-intensive or experimental analysis features for the CLI tools (e.g., Peel Chain, Fund Churning).
  * `thehackerdex/config/monitoring.toml`: Configures the `monitor` CLI tool - target programs/addresses, monitoring strategy (polling/websocket), alert criteria.
  * `thehackerdex-api/src/config.rs` (and its corresponding `.env` variables): Manages configuration for the API server.

## Contributing

Contributions are welcome! Please follow these guidelines:

1.  **Issues:** Report bugs or suggest features by opening an issue on the GitHub repository.
2.  **Pull Requests:**
      * Fork the repository.
      * Create a new branch for your feature or bug fix.
      * Ensure your code builds (`cargo build`) and passes tests (`cargo test`).
      * Format your code using `cargo fmt`.
      * Commit your changes and push to your fork.
      * Open a pull request against the main repository branch.
3.  **Coding Style:** Follow standard Rust conventions and the formatting applied by `cargo fmt`. Add comments to explain complex logic.
4.  **Documentation:** Update the README or relevant documentation if you add or change functionality. For specific functionalities add it to the docs folder.

## License

HackerDex is licensed under the **MIT License**. See the [LICENSE](https://github.com/your-username/hackerdex/blob/main/LICENSE) file for details.

**Attribution:** If you use, modify, or distribute this software, please provide attribution to the HackerDex open-source project and its contributors. While not legally required by the MIT license, it is appreciated by the community.
