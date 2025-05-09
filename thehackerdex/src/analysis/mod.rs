pub mod program_analyzer;
pub mod transaction_analysis;
pub mod transaction_parser;
pub mod wallet_analyzer;

// For backward compatibility, create a re-export to the new module
pub use crate::heuristic_engine::HeuristicFlags;
