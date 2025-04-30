# HackerDex Database Setup

This directory contains the database implementation for the HackerDex project, which is used to store and retrieve information about Solana addresses.

## Database Schema

The database schema follows the requirements specified in Task 2.1 of the project plan:

### Tables

#### `known_addresses`
- `address`: Primary key, the Solana address in base58 encoding
- `entity_name`: Name of the entity associated with the address
- `category`: Type of entity (e.g., "Bridge Contract", "DEX Router", "Known Hacker")
- `risk_level`: Risk assessment of the address ("Low", "Medium", "High", "Critical")
- `source_of_info`: Where the information was obtained from
- `confidence_score`: How confident we are in the label/risk (1-5)
- `notes`: Additional context or information
- `created_at`: When the record was created
- `updated_at`: When the record was last updated

#### `address_interactions`
- `source_address`: Address initiating an interaction
- `target_address`: Address receiving the interaction
- `interaction_count`: Number of interactions between these addresses
- `first_seen_at`: When the interaction was first observed
- `last_seen_at`: When the interaction was last observed

## Setup Instructions

### Using Docker (Recommended)

1. Start the PostgreSQL database using Docker Compose:
   ```bash
   docker-compose up -d
   ```

2. The database will be initialized with the schema defined in `init-db/01-create-tables.sql`.

3. To import sample data:
   ```bash
   cargo run --bin db_import -- samples/known_addresses.csv
   ```

### Database Connection

The database connection URL is specified in your `.env` file:
```
DATABASE_URL=postgresql://hackerdex:hackerdex_password@localhost:5432/hackerdex
```

## API Functions

The database module provides the following functions as required by Task 2.2:

- `initialize_db()`: Sets up the database connection and tables
- `add_known_address(address_data)`: Adds a new address record
- `get_address_details(address)`: Retrieves all stored details for a specific address
- `update_address_details(address, updates)`: Updates specific fields for an existing address record
- `get_all_addresses_by_category(category)`: Retrieves addresses matching a specific category
- `delete_address(address)`: Removes an address from the database
- `record_address_interaction(source_address, target_address)`: Records an interaction between two addresses

## Manual Data Entry

To manually add or update addresses in the database, you can:

1. Use the provided command-line utility:
   ```bash
   cargo run --bin db_import -- samples/known_addresses.csv
   ```

2. Create a CSV file with addresses in the format:
   ```
   address,entity_name,category,risk_level,source_of_info,confidence_score,notes
   ```

3. Or import addresses programmatically using the `add_known_address` function.