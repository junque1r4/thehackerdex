# HackerDex Fund Tracing Module

The Fund Tracing module provides powerful tools to detect and monitor flows of funds between different addresses on the Solana blockchain, with a particular focus on identifying addresses receiving funds from or sending funds to high-risk entities.

## Overview

Fund tracing operates in two primary directions:

1. **Forward Tracing**: Tracks funds flowing OUT from known high-risk sources, helping you identify where stolen funds or funds from known bad actors are going.

2. **Backward Tracing**: Tracks funds flowing INTO your monitored addresses, helping you identify if your monitored addresses are receiving funds from suspicious sources.

## Configuration

Fund tracing can be configured with several parameters:

- **High-Risk Categories**: Types of addresses considered high-risk (e.g., "Known Hacker", "Sanctioned Entity").
- **High-Risk Levels**: Risk levels considered high-risk (e.g., "High", "Critical").
- **Maximum Hop Count**: How many levels deep to trace the flow of funds (e.g., 3 hops).
- **Batch Size**: Number of addresses to process in parallel for improved performance.

## Usage

### Command-line Interface

The fund tracing functionality is available through the `run_tracer` command-line tool:

```bash
# Forward trace from high-risk sources
cargo run --bin run_tracer forward-track

# Forward trace from a specific address
cargo run --bin run_tracer forward-track --source <ADDRESS>

# Backward trace to monitored addresses
cargo run --bin run_tracer backward-track --addresses <ADDRESS1,ADDRESS2>

# Find connections between high-risk sources and monitored addresses
cargo run --bin run_tracer find-connections --monitored <ADDRESS1,ADDRESS2>
```

### Configuration Options

All commands accept these additional options:

```bash
# Set maximum hop count (default: 3)
--max-hops 2

# Set batch size for parallel processing (default: 10)
--batch-size 5

# Define high-risk categories (comma-separated, no spaces)
--categories "Known Hacker,Sanctioned Entity,Mixer"

# Define high-risk levels (comma-separated, no spaces)
--risk-levels "High,Critical"
```

### Scheduled Jobs

Use the `schedule_fund_trace` tool to run fund tracing operations on a schedule:

```bash
# Run scheduled fund tracing with default settings
cargo run --bin schedule_fund_trace

# Specify output directory for reports
cargo run --bin schedule_fund_trace --output-dir ./my-reports
```

Set up a cron job to run this regularly:

```bash
# Add to crontab (run daily at 2 AM)
0 2 * * * cd /path/to/hackerdex && ./target/release/schedule_fund_trace > /var/log/fund_trace.log 2>&1
```

## Database Integration

Fund tracing is integrated with the HackerDex database, using:

- **Known Addresses**: Existing high-risk addresses used as starting points for tracing.
- **Trace Links**: Relationships discovered between addresses during tracing.

## Example Workflow

1. **Identify High-Risk Seeds**: The system first identifies high-risk addresses based on configured criteria.
2. **Trace Forward**: For each high-risk address, trace the flow of funds forward through the blockchain.
3. **Record Relationships**: Each transaction relationship is recorded as a TraceLink in the database.
4. **Analysis**: Review the discovered links to identify patterns, common destinations, or suspicious behavior.

## Extending the System

The fund tracing system can be extended in several ways:

1. **Custom Heuristics**: Develop additional heuristics to flag suspicious patterns in fund flows.
2. **Risk Scoring**: Implement automatic risk scoring for newly discovered addresses.
3. **Alert System**: Create alerts when funds flow to/from specific addresses of interest.
4. **Visualization**: Build a visualization layer to display fund flows as a network graph.

## API Usage

```rust
use hackerdex::discovery::FundTracer;
use hackerdex::db::repository::Repository;
use hackerdex::rpc::RateLimitedClient;

// Initialize components
let repo = Repository::new(pool);
let rpc = RateLimitedClient::new(Some("https://api.mainnet-beta.solana.com".to_string()));
let tracer = FundTracer::with_defaults(repo, rpc);

// Get high-risk addresses
let high_risk_addresses = tracer.get_high_risk_addresses().await?;

// Trace funds forward from a specific address (3 hops max)
let forward_links = tracer.trace_funds_forward("source_address", Some(3)).await?;

// Trace funds backward to a specific address (2 hops max)
let backward_links = tracer.trace_funds_backward("target_address", Some(2)).await?;

// Find connections between high-risk sources and monitored addresses
let monitored_addresses = vec!["address1".to_string(), "address2".to_string()];
let suspicious_sources = tracer.find_suspicious_fund_sources(&monitored_addresses, Some(3)).await?;
```