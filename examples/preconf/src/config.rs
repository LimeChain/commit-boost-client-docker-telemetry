use commit_boost::prelude::RelayEntry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ExtraConfig {
    pub relays: Vec<RelayEntry>,
    pub beacon_nodes: Vec<String>,
    pub chain_id: u64,
}
