use alloy::rpc::types::beacon::BlsPublicKey;
use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};
use cb_common::{pbs::BuilderEvent, utils::get_user_agent};
use reqwest::StatusCode;
use tracing::{error, info, trace};
use uuid::Uuid;

use crate::{
    api::BuilderApi,
    constants::GET_PROPOSER_DUTIES_ENDPOINT_TAG,
    error::PbsClientError,
    metrics::BEACON_NODE_STATUS,
    state::{BuilderApiState, PbsState},
};

#[tracing::instrument(skip_all, name = "check_proposers_slot", fields(req_id = %Uuid::new_v4()))]
pub async fn handle_check_proposers_slot<S: BuilderApiState, T: BuilderApi<S>>(
    req_headers: HeaderMap,
    State(state): State<PbsState<S>>,
    Json(pubkeys): Json<Vec<BlsPublicKey>>,
) -> Result<impl IntoResponse, PbsClientError> {
    trace!(?pubkeys);
    state.publish_event(BuilderEvent::CheckProposersSlot);

    let ua = get_user_agent(&req_headers);

    info!(?ua, relay_check = state.config.pbs_config.relay_check);

    match T::check_proposers_slot(pubkeys, req_headers, state.clone()).await {
        Ok(_) => {
            state.publish_event(BuilderEvent::CheckProposersSlotResponse);
            info!("relay check successful");

            BEACON_NODE_STATUS.with_label_values(&["200", GET_PROPOSER_DUTIES_ENDPOINT_TAG]).inc();
            Ok(StatusCode::OK)
        }
        Err(err) => {
            error!(?err, "all relays failed get_proposer_duties");

            let err = PbsClientError::NoResponse;
            BEACON_NODE_STATUS
                .with_label_values(&[err.status_code().as_str(), GET_PROPOSER_DUTIES_ENDPOINT_TAG])
                .inc();
            Err(err)
        }
    }
}
