use std::sync::Arc;

use alloy::rpc::types::beacon::{BlsPublicKey, BlsSignature};
use tokio::sync::RwLock;
use ethereum_consensus::{
    deneb::{minimal::MAX_BYTES_PER_TRANSACTION, Transaction}, ssz::prelude::*
};
use tree_hash_derive::TreeHash;

use crate::{api::PreconfService, constants::{MAX_REST_TRANSACTIONS, MAX_TOP_TRANSACTIONS, MAX_TRANSACTIONS_PER_BLOCK}};

/// Details of a signed constraints.
#[derive(Debug, Default, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignedConstraints {
    pub message: ConstraintsMessage,
    pub signature: BlsSignature,
}

/// Represents the message of a constraint.
#[derive(Debug, Default, PartialEq, Eq, Clone, SimpleSerialize, serde::Serialize, serde::Deserialize)]
pub struct ConstraintsMessage {
    pub slot: u64,
    pub constraints: List<List<Constraint, MAX_TRANSACTIONS_PER_BLOCK>, MAX_TRANSACTIONS_PER_BLOCK>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, SimpleSerialize, serde::Serialize, serde::Deserialize)]

pub struct Constraint {
    pub tx: Transaction<MAX_BYTES_PER_TRANSACTION>,
}

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<RwLock<PreconfService>>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignedPreconferElection {
    pub message: PreconferElection,
    pub signature: BlsSignature,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, TreeHash, serde::Serialize, serde::Deserialize)]
pub struct PreconferElection {
    pub preconfer_pubkey: BlsPublicKey,
    pub slot_number: u64,
    pub chain_id: u64,
    pub gas_limit: u64,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProposerConstraintsV1 {
    pub top: List<Transaction<MAX_BYTES_PER_TRANSACTION>, MAX_TOP_TRANSACTIONS>,
    pub rest: List<Transaction<MAX_BYTES_PER_TRANSACTION>, MAX_REST_TRANSACTIONS>,
}
