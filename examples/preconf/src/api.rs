use std::sync::Arc;

use ethereum_consensus::ssz::prelude::{HashTreeRoot, List};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use commit_boost::prelude::*;
use eyre::Result;
use futures::future::select_ok;
use reqwest::Client;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::{
    config::ExtraConfig,
    constants::{
        MAX_TRANSACTIONS_PER_BLOCK, SET_CONSTRAINTS_PATH
    },
    types::{Constraint, ConstraintsMessage, ProposerConstraintsV1, SignedConstraints},
    AppState, VAL_RECEIVED_COUNTER,
};

pub fn create_router(app_state: AppState) -> Router {
    Router::new()
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
        .route("/v1/constraints", post(set_constraints))
        .with_state(app_state)
}

async fn set_constraints(
    State(app_state): State<AppState>,
    Json(payload): Json<ProposerConstraintsV1>,
) -> Result<impl IntoResponse, StatusCode> {
    let service = app_state.service.read().await;

    match service.set_constraints(payload).await {
        Ok(signed_constraints) => Ok((StatusCode::OK, Json(signed_constraints))),
        Err(status_code) => Err(status_code),
    }
}

pub struct PreconfService {
    config: StartPreconfModuleConfig<ExtraConfig>,
    latest_signed_constraints: Arc<RwLock<Option<SignedConstraints>>>,
}

impl PreconfService {
    pub async fn new(config: StartPreconfModuleConfig<ExtraConfig>) -> Self {
        PreconfService { config, latest_signed_constraints: Arc::new(RwLock::new(None)) }
    }

    pub async fn get_pubkeys(&self) -> Result<impl IntoResponse, StatusCode> {
        match self.config.signer_client.get_pubkeys().await {
            Ok(pubkeys_response) => {
                let response = serde_json::json!(pubkeys_response);
                VAL_RECEIVED_COUNTER.inc();
                Ok((StatusCode::OK, Json(response)))
            }
            Err(err) => {
                error!(?err, "Failed to get pubkeys");
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    pub async fn set_constraints(&self, payload: ProposerConstraintsV1) -> Result<(), StatusCode> {
        let pubkeys = self.config.signer_client.get_pubkeys().await.map_err(|err| {
            error!(?err, "Failed to get pubkeys");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let pubkey = pubkeys.consensus.first().ok_or_else(|| {
            error!("No key available");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let mut constraints_inner: List<Constraint, MAX_TRANSACTIONS_PER_BLOCK> = List::default();
        for tx in payload.top.iter() {
            let constraint = Constraint { tx: tx.clone() };
            constraints_inner.push(constraint);
        }

        let mut constraints: List<List<Constraint, MAX_TRANSACTIONS_PER_BLOCK>, MAX_TRANSACTIONS_PER_BLOCK> = List::default();
        constraints.push(constraints_inner);

        let message = ConstraintsMessage { slot: payload.slot_number, constraints };
        let tree_hash_root_result = message.hash_tree_root();
        let tree_hash_root = tree_hash_root_result.as_deref().unwrap(); 

        let request = SignRequest::builder(&self.config.id, *pubkey).with_root(*tree_hash_root);
        let signature =
            self.config.signer_client.request_signature(&request).await.map_err(|err| {
                error!(?err, "Failed to request signature");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let signed_constraints = SignedConstraints { message, signature };

        let mut handles = Vec::new();

        info!("Sending constraints {}", serde_json::to_string(&signed_constraints).unwrap());

        for relay in &self.config.relays {
            let client = Client::new();
            handles.push(
                client
                    .post(format!("{}{SET_CONSTRAINTS_PATH}", relay.url))
                    .json(&signed_constraints)
                    .send(),
            );
        }

        let results = select_ok(handles).await;
        match results {
            Ok((response, _)) => {
              let status = response.status();
              let response_bytes = response.bytes().await.expect("failed to get bytes");
              let ans = String::from_utf8_lossy(&response_bytes).into_owned();
              if !status.is_success() {
                  error!(err = ans, ?status, "failed sending set constraints request");
                  return Err(status)
              }

              // Store the latest signed constraints in memory
              let mut latest_constraints = self.latest_signed_constraints.write().await;
              *latest_constraints = Some(signed_constraints.clone());

              info!("Successful set constraints: {ans:?}");
              Ok(())
            },
            Err(err) => {
              error!("Failed set constraints: {err}");
              Err(StatusCode::BAD_REQUEST)
            },
        }
    }
}
