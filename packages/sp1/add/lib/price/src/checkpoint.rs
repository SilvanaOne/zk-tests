use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    pub sequence_number: u64,
    pub timestamp_ms: u64,
    pub digest: String,
    pub epoch: u64,
}
