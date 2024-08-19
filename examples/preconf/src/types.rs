use std::sync::Arc;

use alloy::rpc::types::beacon::{BlsPublicKey, BlsSignature};
use bincode;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tree_hash::{merkle_root, Hash256, PackedEncoding, TreeHash, TreeHashType, BYTES_PER_CHUNK};
use tree_hash_derive::TreeHash;

use crate::api::PreconfService;

impl TreeHash for ConstraintsMessage {
    fn tree_hash_type() -> TreeHashType {
        TreeHashType::Vector
    }

    fn tree_hash_packed_encoding(&self) -> PackedEncoding {
        unreachable!("Vector should never be packed.")
    }

    fn tree_hash_packing_factor() -> usize {
        unreachable!("Vector should never be packed.")
    }

    fn tree_hash_root(&self) -> Hash256 {
        let mut serialized_constraints = Vec::new();
        for constraint in &self.constraints {
            serialized_constraints
                .extend(bincode::serialize(constraint).expect("Serialization failed"));
        }

        let values_per_chunk = BYTES_PER_CHUNK;
        let minimum_chunk_count =
            (serialized_constraints.len() + values_per_chunk - 1) / values_per_chunk;

        merkle_root(&serialized_constraints, minimum_chunk_count)
    }
}

/// Details of a signed constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedConstraints {
    pub message: ConstraintsMessage,
    pub signature: BlsSignature,
}

/// Represents the message of a constraint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintsMessage {
    pub slot: u64,
    pub constraints: Vec<Vec<Constraint>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Constraint {
    pub tx: String,
}

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<RwLock<PreconfService>>,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedPreconferElection {
    pub message: PreconferElection,
    /// Signature over `message`. Must be signed by the proposer for `slot`.
    pub signature: BlsSignature,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize, TreeHash)]
pub struct PreconferElection {
    pub preconfer_pubkey: BlsPublicKey,
    pub slot_number: u64,
    pub chain_id: u64,
    pub gas_limit: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProposerConstraintsV1 {
    pub top: Vec<String>,
    pub rest: Vec<String>,
}
