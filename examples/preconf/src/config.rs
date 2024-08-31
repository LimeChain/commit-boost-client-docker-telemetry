use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ExtraConfig {
    pub beacon_nodes: Vec<String>,
    pub chain_id: u64,
}
