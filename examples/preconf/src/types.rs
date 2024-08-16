use std::sync::Arc;

use alloy::{
    primitives::TxHash,
    rpc::types::beacon::{BlsPublicKey, BlsSignature},
};
use bincode;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use tokio::sync::RwLock;
use tree_hash::{merkle_root, Hash256, PackedEncoding, TreeHash, TreeHashType, BYTES_PER_CHUNK};
use tree_hash_derive::TreeHash;

use crate::{
    api::PreconfService,
    constants::{
        MAX_CONSTRAINTS_PER_SLOT, MAX_REST_TRANSACTIONS, MAX_TOP_TRANSACTIONS,
        TX_HASH_SIZE_IN_BYTES,
    },
};

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
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct ConstraintsMessage {
    pub validator_index: u64,
    pub slot: u64,
    #[serde(with = "BigArray")]
    pub constraints: [Constraint; MAX_CONSTRAINTS_PER_SLOT],
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Constraint {
    #[serde(with = "BigArray")]
    pub tx: [u8; TX_HASH_SIZE_IN_BYTES],
    pub index: u64,
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
    #[serde(with = "BigArray")]
    pub top: [TxHash; MAX_TOP_TRANSACTIONS],
    #[serde(with = "BigArray")]
    pub rest: [TxHash; MAX_REST_TRANSACTIONS],
}
