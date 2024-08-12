use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use commit_boost::prelude::*;
use ethereum_types::H256;
use eyre::Result;
use tiny_keccak::{Hasher, Keccak};
use tokio::sync::RwLock;
use tracing::error;

use crate::{
    config::ExtraConfig,
    types::{SignedValidatorConditionsV1, ValidatorConditionsV1},
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
        .route("/v1/conditions", post(post_conditions))
        .with_state(app_state)
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

pub struct PreconfService {
    config: StartPreconfModuleConfig<ExtraConfig>,
    latest_signed_conditions: Arc<RwLock<Option<SignedValidatorConditionsV1>>>,
}

impl PreconfService {
    pub async fn new(config: StartPreconfModuleConfig<ExtraConfig>) -> Self {
        PreconfService { config, latest_signed_conditions: Arc::new(RwLock::new(None)) }
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

    pub async fn post_conditions(
        &self,
        payload: ValidatorConditionsV1,
    ) -> Result<SignedValidatorConditionsV1, StatusCode> {
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
