# HackerDex Monitoring System Configuration

This document describes how to configure the monitoring system using the TOML configuration format.

## Configuration File

The monitoring system uses a TOML configuration file that can be specified using the `MONITORING_CONFIG_PATH` environment variable or through the main application configuration.

## Configuration Structure

The monitoring configuration includes the following main components:

### Target Monitoring

- `target_programs` - A list of Solana program IDs to monitor for interactions.
- `watch_addresses` - A list of specific high-risk addresses to watch for interactions.

### Monitoring Strategy

- `strategy` - Can be either `"polling"` or `"websocket"`. Currently defaults to `"polling"`.
- `polling_interval_seconds` - How frequently to poll the blockchain when using polling strategy.

### WebSocket Parameters

These parameters are used when the WebSocket monitoring strategy is selected:

```toml
[websocket_params]
max_reconnects = 5
reconnect_backoff_seconds = 10
batch_size = 100
```

### Critical Transaction Criteria

Define when a transaction should be flagged as critical:

```toml
[critical_criteria]
# Minimum risk score to consider critical (0-100)
risk_score_threshold = 80

# List of heuristic flags that indicate critical status
required_flags = [
    "direct_illicit_interaction",
    "suspicious_approval", 
    "total_balance_sweep"
]
```

### Auto-added Wallets Configuration

Settings for automatically adding wallets detected by the monitoring system:

```toml
[auto_added_wallets]
# Default category for auto-added wallets
default_category = "monitored_suspicious"

# Default risk level (1-5) for auto-added wallets
default_risk_level = 4

# Source note template (will be stored in DB)
source_note = "Automated Monitor v1.0 (Critical Transaction Detection)"
```

## Example Configuration

Here's a complete example of a monitoring configuration file:

```toml
# HackerDex Monitoring Configuration

# List of program IDs to monitor
target_programs = [
    "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s", # Metaplex token metadata program
    "vau1zxA2LbssAUEF7Gpw91zMM1LvXrvpzJtmZ58rPsn", # Wormhole token bridge
    "wormDTUJ6AWPNvk59vGQbDvGJmqbDTdgWgAqcLBCgUb"  # Wormhole core bridge
]

# List of high-risk addresses to watch interactions for
watch_addresses = [
    "d41a158e2677072944e0fed98cfd654686b405c0578557c46b3e7ab5e5df27d8",
    "55d398326f99059ff775485246999027b3197955",
]

# Monitoring strategy: "polling" or "websocket"
strategy = "polling"

# Polling interval in seconds
polling_interval_seconds = 60

[websocket_params]
max_reconnects = 5
reconnect_backoff_seconds = 10
batch_size = 100

[critical_criteria]
risk_score_threshold = 80

required_flags = [
    "direct_illicit_interaction",
    "suspicious_approval", 
    "total_balance_sweep"
]

[auto_added_wallets]
default_category = "monitored_suspicious"
default_risk_level = 4
source_note = "Automated Monitor v1.0 (Critical Transaction Detection)"
```

## Usage in Code

The monitoring configuration can be loaded in your code using:

```rust
let config = Config::from_env();
let monitoring_config = match config.load_monitoring_config() {
    Ok(Some(config)) => config,
    Ok(None) => MonitoringConfig::default(),
    Err(e) => panic!("Failed to load monitoring configuration: {}", e),
};
```

You can then access the configuration fields:

- `monitoring_config.target_programs` - HashSet of program IDs
- `monitoring_config.watch_addresses` - HashSet of addresses
- `monitoring_config.strategy` - Enum (Polling or WebSocket)
- `monitoring_config.polling_interval()` - Get interval as Duration
- `monitoring_config.critical_criteria.risk_score_threshold` - Risk score threshold
- `monitoring_config.critical_criteria.required_flags` - List of critical heuristic flags
- `monitoring_config.auto_added_wallets.*` - Configuration for auto-added wallets