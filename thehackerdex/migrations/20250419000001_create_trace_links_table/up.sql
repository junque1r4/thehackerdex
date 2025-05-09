-- Migration UP file for creating the trace_links table
-- This table stores explicit relationships between addresses discovered during forward/backward tracing

-- Create table for trace links
CREATE TABLE IF NOT EXISTS trace_links (
    id SERIAL PRIMARY KEY,
    source_address VARCHAR(44) NOT NULL,
    target_address VARCHAR(44) NOT NULL,
    relationship_type TEXT NOT NULL, -- e.g., 'forward_trace_hop', 'backward_trace_hop'
    trace_initiator VARCHAR(44) NOT NULL,
    hop_count INTEGER NOT NULL,
    discovery_timestamp TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    transaction_signature TEXT,
    
    -- Foreign key constraints
    CONSTRAINT fk_source_address FOREIGN KEY (source_address) 
        REFERENCES known_addresses (address) ON DELETE CASCADE,
    CONSTRAINT fk_target_address FOREIGN KEY (target_address) 
        REFERENCES known_addresses (address) ON DELETE CASCADE,
    CONSTRAINT fk_trace_initiator FOREIGN KEY (trace_initiator) 
        REFERENCES known_addresses (address) ON DELETE CASCADE
);

-- Create indexes to improve query performance
CREATE INDEX IF NOT EXISTS idx_trace_links_source ON trace_links(source_address);
CREATE INDEX IF NOT EXISTS idx_trace_links_target ON trace_links(target_address);
CREATE INDEX IF NOT EXISTS idx_trace_links_initiator ON trace_links(trace_initiator);
CREATE INDEX IF NOT EXISTS idx_trace_links_relationship_type ON trace_links(relationship_type);

-- Add a composite index for efficient queries across multiple columns
CREATE INDEX IF NOT EXISTS idx_trace_links_composite ON trace_links(trace_initiator, source_address, target_address);

-- Create a constraint to ensure source and target are different addresses
ALTER TABLE trace_links ADD CONSTRAINT different_addresses CHECK (source_address != target_address);