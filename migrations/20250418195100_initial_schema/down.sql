-- Migration DOWN file for SQLx
-- This will undo the changes made in the up migration

-- Drop triggers first
DROP TRIGGER IF EXISTS update_address_interactions_last_seen ON address_interactions;
DROP TRIGGER IF EXISTS update_known_addresses_updated_at ON known_addresses;

-- Drop functions
DROP FUNCTION IF EXISTS update_updated_at_column();

-- Drop tables - order matters for foreign key constraints
DROP TABLE IF EXISTS address_interactions;
DROP TABLE IF EXISTS known_addresses;

-- Drop indexes - should be dropped when tables are dropped, but just to be safe
DROP INDEX IF EXISTS idx_address_interactions_source;
DROP INDEX IF EXISTS idx_address_interactions_target;
DROP INDEX IF EXISTS idx_known_addresses_category;
DROP INDEX IF EXISTS idx_known_addresses_risk_level;