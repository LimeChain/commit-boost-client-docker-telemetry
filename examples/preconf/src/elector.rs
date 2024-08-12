use std::{collections::HashMap, time::Duration};

use alloy::rpc::types::beacon::BlsPublicKey;
use commit_boost::prelude::*;
use futures::future::join_all;
use reqwest::Client;
use tokio::{sync::mpsc, time::sleep};
use tracing::{error, info, warn};

use crate::{
    beacon_client::types::ProposerDuty,
    config::ExtraConfig,
    types::{PreconferElection, SignedPreconferElection, ELECT_PRECONFER_PATH},
};

pub struct PreconfElector {
    pub config: StartPreconfModuleConfig<ExtraConfig>,

    _next_slot: u64,

    _elections: HashMap<u64, BlsPublicKey>,

    duties_rx: mpsc::UnboundedReceiver<Vec<ProposerDuty>>,
}

impl PreconfElector {
    pub fn new(
        config: StartPreconfModuleConfig<ExtraConfig>,
        duties_rx: mpsc::UnboundedReceiver<Vec<ProposerDuty>>,
    ) -> Self {
        Self { config, _next_slot: 0, _elections: HashMap::new(), duties_rx }
    }
}

impl PreconfElector {
    pub async fn run(mut self) -> eyre::Result<()> {
        info!("Fetching validator pubkeys");

        let consensus_pubkeys = match self.config.signer_client.get_pubkeys().await {
            Ok(pubkeys) => pubkeys.consensus,
            Err(err) => {
                warn!("Failed to fetch pubkeys: {err}");
                info!("Waiting a bit before retrying");
                sleep(Duration::from_secs(10)).await;
                self.config.signer_client.get_pubkeys().await?.consensus
            }
        };
        info!("Fetched {} pubkeys", consensus_pubkeys.len());

        while let Some(duties) = self.duties_rx.recv().await {
            let l = duties.len();
            let our_duties: Vec<_> =
                duties.into_iter().filter(|d| consensus_pubkeys.contains(&d.public_key)).collect();

            info!("Received {l} duties, we have {} to elect", our_duties.len());

            for duty in our_duties {
                // this could be done in parallel
                if let Err(err) = self.elect_proposer(duty).await {
                    error!("Failed to elect gateway: {err}");
                };
            }
        }

        Ok(())
    }

    /// Delegate preconf rights
    async fn elect_proposer(&mut self, duty: ProposerDuty) -> eyre::Result<()> {
        info!(slot = duty.slot, validator_pubkey = %duty.public_key,  "Sending gateway delegation");

        let election_message = PreconferElection {
            preconfer_pubkey: duty.public_key,
            slot_number: duty.slot,
            chain_id: self.config.extra.chain_id,
            gas_limit: 0,
        };

        let request =
            SignRequest::builder(&self.config.id, duty.public_key).with_msg(&election_message);

        let signature = self.config.signer_client.request_signature(&request).await?;

        let signed_election = SignedPreconferElection { message: election_message, signature };

        let mut handles = Vec::new();

        info!("Received delegation signature: {signature}");
        info!("Sending delegation {}", serde_json::to_string(&signed_election).unwrap());

        for relay in &self.config.extra.relays {
            let client = Client::new();
            handles.push(
                client
                    .post(format!("{}{ELECT_PRECONFER_PATH}", relay.url))
                    .json(&signed_election)
                    .send(),
            );
        }

        let results = join_all(handles).await;

        for res in results {
            match res {
                Ok(response) => {
                    let status = response.status();
                    let response_bytes = response.bytes().await.expect("failed to get bytes");
                    let ans = String::from_utf8_lossy(&response_bytes).into_owned();
                    if !status.is_success() {
                        error!(err = ans, ?status, "failed sending delegation sign request");
                        continue;
                    }

                    info!("Successful election: {ans:?}")
                }
                Err(err) => error!("Failed election: {err}"),
            }
        }

        Ok(())
    }
}
