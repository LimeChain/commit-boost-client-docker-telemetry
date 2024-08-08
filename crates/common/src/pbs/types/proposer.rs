use std::fmt;

use alloy::rpc::types::beacon::BlsPublicKey;
use serde::{Deserialize, Serialize};
use ssz_derive::{Decode, Encode};

use super::utils::VersionedResponse;

#[derive(Debug)]
pub enum ProposerError {
    ProposerNotFound(String),
}

impl fmt::Display for ProposerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ProposerError::ProposerNotFound(msg) => write!(f, "Proposer not found: {}", msg),
        }
    }
}

impl std::error::Error for ProposerError {}

impl From<String> for ProposerError {
    fn from(msg: String) -> ProposerError {
        ProposerError::ProposerNotFound(msg)
    }
}

pub type GetProposerEpochResponse = VersionedResponse<Vec<ProposerSlot>>;

impl GetProposerEpochResponse {
    pub fn check_proposers_slots(
        &self,
        pubkeys: Vec<BlsPublicKey>,
        slot: u64,
    ) -> Result<(), ProposerError> {
        let proposer = self.data.iter().find(|p| pubkeys.contains(&p.pub_key) && p.slot == slot);

        if let Some(_) = proposer {
            Ok(())
        } else {
            Err("Proposer not found".to_string().into())
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct ProposerSlot {
    pub pub_key: BlsPublicKey,
    pub validator_index: u64,
    pub slot: u64,
}
