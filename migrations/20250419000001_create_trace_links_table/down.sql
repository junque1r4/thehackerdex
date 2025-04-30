-- Migration DOWN file for removing the trace_links table
-- This will undo the changes made in the up migration

-- Drop indexes first
DROP INDEX IF EXISTS idx_trace_links_source;
DROP INDEX IF EXISTS idx_trace_links_target;
DROP INDEX IF EXISTS idx_trace_links_initiator;
DROP INDEX IF EXISTS idx_trace_links_relationship_type;
DROP INDEX IF EXISTS idx_trace_links_composite;

-- Drop the table
DROP TABLE IF EXISTS trace_links;