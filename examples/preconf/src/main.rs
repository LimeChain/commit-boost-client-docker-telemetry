use std::{net::SocketAddr, sync::Arc};

use alloy::rpc::types::beacon::{BlsPublicKey, BlsSignature};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use beacon_client::client::MultiBeaconClient;
use commit_boost::prelude::*;
use elector::GatewayElector;
use ethereum_types::H256;
use eyre::{bail, Result};
use lazy_static::lazy_static;
use prometheus::{IntCounter, Registry};
use serde::{Deserialize, Serialize};
use serde_json::json;
use ssz_derive::{Decode, Encode};
use tiny_keccak::{Hasher, Keccak};
use tokio::{
    net::TcpListener,
    sync::{mpsc, RwLock},
};
use tracing::{error, info};

mod beacon_client;
mod elector;
mod types;

#[derive(Debug, Deserialize, Clone)]
struct ExtraConfig {
    pub relays: Vec<RelayEntry>,
    pub beacon_nodes: Vec<String>,
    pub chain_id: u64,
}

// You can define custom metrics and a custom registry for the business logic of
// your module. These will be automatically scraped by the Prometheus server
lazy_static! {
    pub static ref MY_CUSTOM_REGISTRY: prometheus::Registry =
        Registry::new_custom(Some("preconf".to_string()), None).unwrap();
    pub static ref VAL_RECEIVED_COUNTER: IntCounter =
        IntCounter::new("validators_received", "successful validators requests received").unwrap();
}

struct PreconfService {
    config: StartPreconfModuleConfig<ExtraConfig>,
    latest_signed_conditions: Arc<RwLock<Option<SignedValidatorConditionsV1>>>,
}

#[derive(Clone, Debug, Encode, Decode, Serialize, Deserialize)]
struct ValidatorConditionsV1 {
    top: Vec<u8>,
    rest: Vec<u8>,
}

#[derive(Clone, Debug, Encode, Decode, Serialize, Deserialize)]
struct SignedValidatorConditionsV1 {
    message: ValidatorConditionsV1,
    conditions_hash: H256,
    signature: BlsSignature,
}

#[derive(Clone)]
struct AppState {
    service: Arc<RwLock<PreconfService>>,
}

impl PreconfService {
    pub async fn new(config: StartPreconfModuleConfig<ExtraConfig>) -> Self {
        PreconfService { config, latest_signed_conditions: Arc::new(RwLock::new(None)) }
    }

    pub async fn run(self) -> Result<()> {
        let port = self.config.server_port;
        info!("Starting server on port {}", port);

        let app_state = AppState { service: Arc::new(RwLock::new(self)) };

        let router = Router::new()
            .route(
                "/v1/validators",
                get({
                    let app_state = app_state.clone();
                    move || {
                        let app_state = app_state.clone();
                        async move {
                            let service = app_state.service.read().await;
                            service.get_pubkeys().await
                        }
                    }
                }),
            )
            .route("/v1/conditions", post(post_conditions))
            .with_state(app_state);

        let address = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = TcpListener::bind(&address).await?;

        axum::serve(listener, router).await?;

        bail!("Server stopped unexpectedly")
    }

    async fn get_pubkeys(&self) -> Result<impl IntoResponse, StatusCode> {
        match self.config.signer_client.get_pubkeys().await {
            Ok(pubkeys_response) => {
                let response = json!(pubkeys_response);
                VAL_RECEIVED_COUNTER.inc();
                Ok((StatusCode::OK, Json(response)))
            }
            Err(err) => {
                error!(?err, "Failed to get pubkeys");
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    async fn post_conditions(
        &self,
        payload: ValidatorConditionsV1,
    ) -> Result<SignedValidatorConditionsV1, StatusCode> {
        // TODO: Check if the current validator is available

        let pubkeys = self.config.signer_client.get_pubkeys().await.map_err(|err| {
            error!(?err, "Failed to get pubkeys");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let pubkey = pubkeys.consensus.first().ok_or_else(|| {
            error!("No key available");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let mut hasher = Keccak::v256();
        let mut conditions_hash = [0u8; 32];
        hasher.update(&payload.top);
        hasher.finalize(&mut conditions_hash);

        let request = SignRequest::builder(&self.config.id, *pubkey).with_msg(&conditions_hash);
        let signature =
            self.config.signer_client.request_signature(&request).await.map_err(|err| {
                error!(?err, "Failed to request signature");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let signed_conditions = SignedValidatorConditionsV1 {
            message: payload,
            conditions_hash: H256::from(conditions_hash),
            signature,
        };

        // Store the latest signed conditions in memory
        let mut latest_conditions = self.latest_signed_conditions.write().await;
        *latest_conditions = Some(signed_conditions.clone());

        // TODO: Call the relay's /relay/v1/builder/conditions/{pubkey}

        Ok(signed_conditions)
    }
}

async fn post_conditions(
    State(app_state): State<AppState>,
    Json(payload): Json<ValidatorConditionsV1>,
) -> Result<impl IntoResponse, StatusCode> {
    let service = app_state.service.read().await;

    match service.post_conditions(payload).await {
        Ok(signed_conditions) => Ok((StatusCode::OK, Json(signed_conditions))),
        Err(status_code) => Err(status_code),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    initialize_tracing_log();

    // Remember to register all your metrics before starting the process
    MY_CUSTOM_REGISTRY.register(Box::new(VAL_RECEIVED_COUNTER.clone()))?;
    // Spin up a server that exposes the /metrics endpoint to Prometheus
    MetricsProvider::load_and_run(MY_CUSTOM_REGISTRY.clone())?;

    match load_preconf_module_config::<ExtraConfig>() {
        Ok(config) => {
            let (beacon_tx, _) = tokio::sync::broadcast::channel(10);
            let multi_beacon_client =
                MultiBeaconClient::from_endpoint_strs(&config.extra.beacon_nodes);
            multi_beacon_client.subscribe_to_payload_attributes_events(beacon_tx.clone()).await;
            let (duties_tx, duties_rx) = mpsc::unbounded_channel();
            tokio::spawn(
                multi_beacon_client.subscribe_to_proposer_duties(duties_tx, beacon_tx.subscribe()),
            );

            info!(
                module_id = config.id,
                port = config.server_port,
                "Starting module with custom data"
            );

            let elector = GatewayElector::new(config.clone(), duties_rx);

            if let Err(err) = elector.run().await {
                error!(?err, "Error running elector")
            }

            let service = PreconfService::new(config).await;

            if let Err(err) = service.run().await {
                error!(?err, "Service failed");
            }
        }
        Err(err) => {
            error!(?err, "Failed to load module config");
        }
    }
    Ok(())
}
