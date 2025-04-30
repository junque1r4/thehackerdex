use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::time::OffsetDateTime;

/// Represents an address record in the database with risk-relevant context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressData {
    /// The Solana address (base58 encoded)
    pub address: String,
    /// Name of the entity associated with the address
    pub entity_name: String,
    /// Category of address (e.g., "Bridge Contract", "DEX Router", "Known Hacker")
    pub category: String,
    /// Risk level (e.g., "Low", "Medium", "High", "Critical")
    pub risk_level: String,
    /// Source of information (e.g., "Official Docs", "OSINT Report-MM-DD", "OFAC List")
    pub source_of_info: String,
    /// Confidence score (1-5) indicating certainty of the label/risk
    pub confidence_score: i32,
    /// Additional context, links to reports, etc.
    pub notes: Option<String>,
}

/// Represents a full address record including timestamps from the database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AddressRecord {
    /// The Solana address (base58 encoded)
    pub address: String,
    /// Name of the entity associated with the address
    pub entity_name: String,
    /// Category of address (e.g., "Bridge Contract", "DEX Router", "Known Hacker")
    pub category: String,
    /// Risk level (e.g., "Low", "Medium", "High", "Critical")
    pub risk_level: String,
    /// Source of information (e.g., "Official Docs", "OSINT Report-MM-DD", "OFAC List")
    pub source_of_info: String,
    /// Confidence score (1-5) indicating certainty of the label/risk
    pub confidence_score: i32,
    /// Additional context, links to reports, etc.
    pub notes: Option<String>,
    /// When the record was created
    pub created_at: OffsetDateTime,
    /// When the record was last updated
    pub updated_at: OffsetDateTime,
}

impl From<AddressRecord> for AddressData {
    fn from(record: AddressRecord) -> Self {
        Self {
            address: record.address,
            entity_name: record.entity_name,
            category: record.category,
            risk_level: record.risk_level,
            source_of_info: record.source_of_info,
            confidence_score: record.confidence_score,
            notes: record.notes,
        }
    }
}

/// Serializable version of AddressRecord for JSON responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableAddressRecord {
    /// The Solana address (base58 encoded)
    pub address: String,
    /// Name of the entity associated with the address
    pub entity_name: String,
    /// Category of address (e.g., "Bridge Contract", "DEX Router", "Known Hacker")
    pub category: String,
    /// Risk level (e.g., "Low", "Medium", "High", "Critical")
    pub risk_level: String,
    /// Source of information (e.g., "Official Docs", "OSINT Report-MM-DD", "OFAC List")
    pub source_of_info: String,
    /// Confidence score (1-5) indicating certainty of the label/risk
    pub confidence_score: i32,
    /// Additional context, links to reports, etc.
    pub notes: Option<String>,
    /// When the record was created
    pub created_at: DateTime<Utc>,
    /// When the record was last updated
    pub updated_at: DateTime<Utc>,
}

impl From<AddressRecord> for SerializableAddressRecord {
    fn from(record: AddressRecord) -> Self {
        Self {
            address: record.address,
            entity_name: record.entity_name,
            category: record.category,
            risk_level: record.risk_level,
            source_of_info: record.source_of_info,
            confidence_score: record.confidence_score,
            notes: record.notes,
            // Convert OffsetDateTime to DateTime<Utc>
            created_at: DateTime::<Utc>::from_timestamp(record.created_at.unix_timestamp(), 0)
                .unwrap_or_default(),
            updated_at: DateTime::<Utc>::from_timestamp(record.updated_at.unix_timestamp(), 0)
                .unwrap_or_default(),
        }
    }
}

/// Represents an interaction between two addresses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressInteraction {
    /// ID of the interaction
    pub id: i32,
    /// Source address that initiated the interaction
    pub source_address: String,
    /// Target address that received the interaction
    pub target_address: String,
    /// Count of interactions between these addresses
    pub interaction_count: i32,
    /// When the interaction was first seen
    pub first_seen_at: OffsetDateTime,
    /// When the interaction was last seen
    pub last_seen_at: OffsetDateTime,
}

/// Serializable version of AddressInteraction for JSON responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableAddressInteraction {
    /// ID of the interaction
    pub id: i32,
    /// Source address that initiated the interaction
    pub source_address: String,
    /// Target address that received the interaction
    pub target_address: String,
    /// Count of interactions between these addresses
    pub interaction_count: i32,
    /// When the interaction was first seen
    pub first_seen_at: DateTime<Utc>,
    /// When the interaction was last seen
    pub last_seen_at: DateTime<Utc>,
}

impl From<AddressInteraction> for SerializableAddressInteraction {
    fn from(interaction: AddressInteraction) -> Self {
        Self {
            id: interaction.id,
            source_address: interaction.source_address,
            target_address: interaction.target_address,
            interaction_count: interaction.interaction_count,
            // Convert OffsetDateTime to DateTime<Utc>
            first_seen_at: DateTime::<Utc>::from_timestamp(
                interaction.first_seen_at.unix_timestamp(),
                0,
            )
            .unwrap_or_default(),
            last_seen_at: DateTime::<Utc>::from_timestamp(
                interaction.last_seen_at.unix_timestamp(),
                0,
            )
            .unwrap_or_default(),
        }
    }
}

/// Represents a trace link between addresses discovered during forward/backward tracing
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TraceLink {
    /// ID of the trace link
    pub id: i32,
    /// Source address of the trace link
    pub source_address: String,
    /// Target address of the trace link
    pub target_address: String,
    /// Type of relationship (e.g., 'forward_trace_hop', 'backward_trace_hop')
    pub relationship_type: String,
    /// The initiating address of the trace
    pub trace_initiator: String,
    /// How many hops from the initiator this link represents
    pub hop_count: i32,
    /// When this link was discovered
    pub discovery_timestamp: OffsetDateTime,
    /// Optional transaction signature that established this link
    pub transaction_signature: Option<String>,
}

/// Serializable version of TraceLink for JSON responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableTraceLink {
    /// ID of the trace link
    pub id: i32,
    /// Source address of the trace link
    pub source_address: String,
    /// Target address of the trace link
    pub target_address: String,
    /// Type of relationship (e.g., 'forward_trace_hop', 'backward_trace_hop')
    pub relationship_type: String,
    /// The initiating address of the trace
    pub trace_initiator: String,
    /// How many hops from the initiator this link represents
    pub hop_count: i32,
    /// When this link was discovered
    pub discovery_timestamp: DateTime<Utc>,
    /// Optional transaction signature that established this link
    pub transaction_signature: Option<String>,
}

impl From<TraceLink> for SerializableTraceLink {
    fn from(link: TraceLink) -> Self {
        Self {
            id: link.id,
            source_address: link.source_address,
            target_address: link.target_address,
            relationship_type: link.relationship_type,
            trace_initiator: link.trace_initiator,
            hop_count: link.hop_count,
            // Convert OffsetDateTime to DateTime<Utc>
            discovery_timestamp: DateTime::<Utc>::from_timestamp(
                link.discovery_timestamp.unix_timestamp(),
                0,
            )
            .unwrap_or_default(),
            transaction_signature: link.transaction_signature,
        }
    }
}

/// Data structure for creating a new trace link record (omitting id and auto-generated timestamp)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceLinkData {
    /// Source address of the trace link
    pub source_address: String,
    /// Target address of the trace link
    pub target_address: String,
    /// Type of relationship (e.g., 'forward_trace_hop', 'backward_trace_hop')
    pub relationship_type: String,
    /// The initiating address of the trace
    pub trace_initiator: String,
    /// How many hops from the initiator this link represents
    pub hop_count: i32,
    /// Optional transaction signature that established this link
    pub transaction_signature: Option<String>,
}
