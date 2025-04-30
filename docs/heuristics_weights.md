# Heuristic Weights Configuration

This document explains how to configure heuristic weights for the risk scoring system.

## Overview

The risk scoring system uses a set of weights to determine the importance of different heuristic factors when calculating a risk score for a transaction or wallet. These weights can be configured via a TOML or JSON configuration file, allowing you to adjust the scoring system according to your specific needs.

## Configuration File Format

You can use either TOML or JSON format for your configuration file:

### TOML Format (heuristic_weights.toml)

```toml
[weights]
# Critical risk indicators
direct_illicit_interaction = 2.5
high_risk_program = 2.0
high_risk_wallet = 2.0

# High risk indicators
risky_funding = 1.5
risky_spending = 1.5
rapid_dispersal = 1.2
fund_consolidation = 1.2

# Medium risk indicators
structuring = 1.0
high_frequency = 1.0
pass_through = 1.0
risky_category = 1.0

# Low risk indicators
new_wallet = 0.5

# Custom flags from heuristics
rapid_dispersal_score = 0.8
fund_consolidation_score = 0.8
```

### JSON Format (heuristic_weights.json)

```json
{
  "weights": {
    "direct_illicit_interaction": 2.5,
    "high_risk_program": 2.0,
    "high_risk_wallet": 2.0,
    "risky_funding": 1.5,
    "risky_spending": 1.5,
    "rapid_dispersal": 1.2,
    "fund_consolidation": 1.2,
    "structuring": 1.0,
    "high_frequency": 1.0,
    "pass_through": 1.0,
    "risky_category": 1.0,
    "new_wallet": 0.5,
    "rapid_dispersal_score": 0.8,
    "fund_consolidation_score": 0.8
  }
}
```

## Available Heuristic Weight Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| direct_illicit_interaction | 2.5 | Applied when the transaction directly involves an address known to be illicit |
| high_risk_program | 2.0 | Applied when interacting with a program labeled as high risk |
| high_risk_wallet | 2.0 | Applied when interacting with a wallet labeled as high risk |
| risky_funding | 1.5 | Applied based on the ratio of risky funding sources |
| risky_spending | 1.5 | Applied based on the ratio of risky spending destinations |
| rapid_dispersal | 1.2 | Applied when funds are quickly dispersed to multiple addresses |
| fund_consolidation | 1.2 | Applied when funds from multiple sources are quickly consolidated |
| structuring | 1.0 | Applied when transactions appear to be structured to avoid detection |
| high_frequency | 1.0 | Applied when a wallet shows unusually high transaction frequency |
| pass_through | 1.0 | Applied when funds quickly pass through a wallet |
| risky_category | 1.0 | Applied when interacting with services in risky categories |
| new_wallet | 0.5 | Applied for new wallets with suspicious activity |
| rapid_dispersal_score | 0.8 | A more granular score for rapid dispersal patterns |
| fund_consolidation_score | 0.8 | A more granular score for fund consolidation patterns |

## How to Use

1. Create a configuration file in either TOML or JSON format with your desired weights.
2. Set the `HEURISTIC_WEIGHTS_PATH` environment variable to the path of your configuration file:
   ```
   export HEURISTIC_WEIGHTS_PATH=./config/heuristic_weights.toml
   ```
   or add it to your `.env` file:
   ```
   HEURISTIC_WEIGHTS_PATH=./config/heuristic_weights.toml
   ```

3. When you run the application, it will automatically load the weights from your configuration file.

## Fallback Behavior

If the configuration file cannot be loaded or is not specified, the system will use the default weights defined in the code.

## Adding New Weights

If you implement new heuristics, you can add corresponding weights to your configuration file with appropriate names. The system will automatically use them when calculating risk scores.