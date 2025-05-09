-- Initial database schema for HackerDex
-- Migration UP file for SQLx

-- Create table for known addresses
CREATE TABLE IF NOT EXISTS known_addresses (
    address VARCHAR(44) PRIMARY KEY,
    entity_name VARCHAR(255) NOT NULL,
    category VARCHAR(100) NOT NULL,
    risk_level VARCHAR(20) NOT NULL,
    source_of_info VARCHAR(255) NOT NULL,
    confidence_score INTEGER NOT NULL CHECK (confidence_score BETWEEN 1 AND 5),
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_known_addresses_category ON known_addresses(category);

CREATE INDEX IF NOT EXISTS idx_known_addresses_risk_level ON known_addresses(risk_level);

-- Create function for updating the updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger for updating the updated_at timestamp
CREATE TRIGGER update_known_addresses_updated_at
BEFORE UPDATE ON known_addresses
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- Create table for address interactions
CREATE TABLE IF NOT EXISTS address_interactions (
    id SERIAL PRIMARY KEY,
    source_address VARCHAR(44) NOT NULL,
    target_address VARCHAR(44) NOT NULL,
    interaction_count INTEGER DEFAULT 1,
    first_seen_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_source_address FOREIGN KEY (source_address) REFERENCES known_addresses (address) ON DELETE CASCADE,
    CONSTRAINT fk_target_address FOREIGN KEY (target_address) REFERENCES known_addresses (address) ON DELETE CASCADE,
    CONSTRAINT unique_interaction UNIQUE (source_address, target_address)
);

CREATE INDEX IF NOT EXISTS idx_address_interactions_source ON address_interactions(source_address);
CREATE INDEX IF NOT EXISTS idx_address_interactions_target ON address_interactions(target_address);

-- Create trigger for updating the last_seen_at timestamp
CREATE TRIGGER update_address_interactions_last_seen
BEFORE UPDATE ON address_interactions
FOR EACH ROW
WHEN (NEW.interaction_count <> OLD.interaction_count)
EXECUTE FUNCTION update_updated_at_column();