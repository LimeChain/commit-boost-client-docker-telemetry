use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use commit_boost::prelude::*;
use eyre::Result;
use futures::future::{join_all, select_ok};
use reqwest::Client;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::{
    config::ExtraConfig,
    constants::{
        GET_CURRENT_SLOT, MAX_REST_TRANSACTIONS, MAX_TOP_TRANSACTIONS, SET_CONSTRAINTS_PATH,
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

    pub async fn fetch_slots(
        config: &StartPreconfModuleConfig<ExtraConfig>,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let mut handles = Vec::with_capacity(config.relays.len());
        let client = Client::new();

        for relay in &config.relays {
            let url = format!("{}{GET_CURRENT_SLOT}", relay.url);
            handles.push(client.get(&url).send());
        }

        let results = select_ok(handles).await;
        match results {
            Ok((response, _remaining)) => Ok(response),
            Err(e) => Err(e),
        }
    }

    pub async fn set_constraints(&self, payload: ProposerConstraintsV1) -> Result<(), StatusCode> {
        if payload.top.len() > MAX_TOP_TRANSACTIONS {
            error!("Too many top transactions");
            return Err(StatusCode::BAD_REQUEST);
        }

        if payload.rest.len() > MAX_REST_TRANSACTIONS {
            error!("Too many rest transactions");
            return Err(StatusCode::BAD_REQUEST);
        }

        let current_slot = match Self::fetch_slots(&self.config).await {
            Ok(response) => {
                let slot = response.json::<u64>().await.map_err(|err| {
                    error!(?err, "Failed to parse slot");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
                slot
            }
            Err(err) => {
                error!(?err, "Failed to fetch slot");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        info!("Current slot: {}", current_slot);

        let pubkeys = self.config.signer_client.get_pubkeys().await.map_err(|err| {
            error!(?err, "Failed to get pubkeys");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let pubkey = pubkeys.consensus.first().ok_or_else(|| {
            error!("No key available");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let mut constraints: Vec<Constraint> = Vec::with_capacity(payload.top.len());

        for (_, tx_hash) in payload.top.iter().enumerate() {
            let constraint = Constraint { tx: tx_hash.to_string() };
            constraints.push(constraint);
        }

        let message =
            ConstraintsMessage { slot: current_slot + 1, constraints: Vec::from([constraints]) };

        let request = SignRequest::builder(&self.config.id, *pubkey).with_msg(&message);
        let signature =
            self.config.signer_client.request_signature(&request).await.map_err(|err| {
                error!(?err, "Failed to request signature");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let signed_constraints = SignedConstraints { message, signature };

        // Store the latest signed constraints in memory
        let mut latest_constraints = self.latest_signed_constraints.write().await;
        *latest_constraints = Some(signed_constraints.clone());

        let mut handles = Vec::new();

        info!("Received constraints signature: {signature}");
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

        let results = join_all(handles).await;

        for res in results {
            match res {
                Ok(response) => {
                    let status = response.status();
                    let response_bytes = response.bytes().await.expect("failed to get bytes");
                    let ans = String::from_utf8_lossy(&response_bytes).into_owned();
                    if !status.is_success() {
                        error!(err = ans, ?status, "failed sending set constraints request");
                        continue;
                    }

                    info!("Successful set constraints: {ans:?}")
                }
                Err(err) => error!("Failed set constraints: {err}"),
            }
        }

        Ok(())
    }
}
