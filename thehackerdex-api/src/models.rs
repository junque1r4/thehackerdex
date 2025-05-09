use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct AnalyzeWalletRequest {
    pub wallet_address: String,
    // pub force_reanalyze: Option<bool>, // Optional: to bypass cache
    // pub trace_depth: Option<u8>,      // Optional: for fund tracing
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AnalyzeWalletResponse {
    pub job_id: Option<String>, // For async processing
    pub status: String, // e.g., "pending", "completed", "cached"
    pub message: String,
    pub analysis_summary: Option<WalletSummaryResponse>, // Direct response if synchronous/cached
}

// More models will be added here for summary, graph, chat, etc.
// For example, WalletSummaryResponse:
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WalletSummaryResponse {
    pub wallet_address: String,
    pub risk_score: Option<f64>, // Or an appropriate numeric type from your lib
    pub risk_category: Option<String>, // e.g., Low, Medium, High, Critical
    pub detected_heuristics: Vec<HeuristicInfo>,
    pub known_entity_info: Option<KnownEntityInfo>,
    // ... other summary fields from your db::models or heuristic_engine::types
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeuristicInfo {
    pub name: String,
    pub description: String,
    pub score_impact: f64,
    // pub severity: String, // From your heuristic_engine::types::Severity
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KnownEntityInfo {
    pub name: String,
    pub category: String, // e.g., Exchange, Mixer, Scam
    pub source_of_info: Option<String>,
    pub confidence_score: Option<f64>,
    pub notes: Option<String>,
}

// Placeholder for graph data structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GraphNode {
    pub id: String, // Wallet address
    pub label: String, // Short address or entity name
    pub risk_score: Option<f64>,
    pub category: Option<String>, // E.g., EOA, Contract, Mixer
    // Add other node properties as needed by the frontend
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GraphEdge {
    pub source: String, // Source wallet address
    pub target: String, // Target wallet address
    pub label: Option<String>, // e.g., transaction amount, type
    pub relationship_type: Option<String>, // e.g., "transaction", "trace_link"
    // Add other edge properties
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WalletGraphResponse {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChatQueryRequest {
    // pub session_id: Option<String>, // If you want to maintain chat context
    pub query_text: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChatQueryResponse {
    pub response_text: String,
    // pub follow_up_questions: Option<Vec<String>>, // Optional
}