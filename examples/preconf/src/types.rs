use std::sync::Arc;

use alloy::rpc::types::beacon::{BlsPublicKey, BlsSignature};
use ethereum_types::H256;
use serde::{Deserialize, Serialize};
use ssz_derive::{Decode, Encode};
use tokio::sync::RwLock;
use tree_hash_derive::TreeHash;

use crate::api::PreconfService;

pub const ELECT_PRECONFER_PATH: &str = "/eth/v1/builder/elect_preconfer";

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

#[derive(Clone, Debug, Encode, Decode, Serialize, Deserialize)]
pub struct ValidatorConditionsV1 {
    pub top: Vec<u8>,
    pub rest: Vec<u8>,
}

#[derive(Clone, Debug, Encode, Decode, Serialize, Deserialize)]
pub struct SignedValidatorConditionsV1 {
    pub message: ValidatorConditionsV1,
    pub conditions_hash: H256,
    pub signature: BlsSignature,
}
