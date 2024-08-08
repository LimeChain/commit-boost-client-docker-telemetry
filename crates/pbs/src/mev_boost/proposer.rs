use std::time::{Duration, Instant};

use alloy::rpc::types::beacon::BlsPublicKey;
use axum::http::{HeaderMap, HeaderValue};
use cb_common::{
    pbs::{GetProposerEpochResponse, RelayClient, HEADER_START_TIME_UNIX_MS},
    utils::{get_user_agent, utcnow_ms},
};
use eyre::bail;
use futures::future::select_ok;
use reqwest::header::USER_AGENT;
use tracing::{debug, warn};

use crate::{
    constants::{GET_PROPOSER_DUTIES_ENDPOINT_TAG, TIMEOUT_ERROR_CODE_STR},
    error::PbsError,
    metrics::{RELAY_LATENCY, RELAY_STATUS_CODE},
    state::{BuilderApiState, PbsState},
};

pub async fn check_proposers_slot<S: BuilderApiState>(
    pubkeys: Vec<BlsPublicKey>,
    req_headers: HeaderMap,
    state: PbsState<S>,
) -> eyre::Result<()> {
    let (slot, _) = state.get_slot_and_uuid();

    let mut send_headers = HeaderMap::new();
    send_headers
        .insert(HEADER_START_TIME_UNIX_MS, HeaderValue::from_str(&utcnow_ms().to_string())?);
    if let Some(ua) = get_user_agent(&req_headers) {
        send_headers.insert(USER_AGENT, HeaderValue::from_str(&ua)?);
    }

    let relays = state.relays();
    let mut handles = Vec::with_capacity(relays.len());
    for relay in relays.iter() {
        handles.push(Box::pin(get_proposer_duties_url(
            relay,
            send_headers.clone(),
            state.config.pbs_config.timeout_get_payload_ms,
        )));
    }

    let results = select_ok(handles).await;
    match results {
        Ok((res, _)) => {
            res.check_proposers_slots(pubkeys, slot)?;
            Ok(())
        }
        Err(_) => bail!("No relay returned get_proposer_duties successfully"),
    }
}

#[tracing::instrument(skip_all, name = "handler", fields(relay_id = relay.id.as_ref()))]
pub async fn get_proposer_duties_url(
    relay: &RelayClient,
    headers: HeaderMap,
    timeout_ms: u64,
) -> Result<GetProposerEpochResponse, PbsError> {
    // Use the current epoch to get the proposer duties, the relay has it
    let url = relay.get_proposer_duties_url(0);

    let start_request = Instant::now();
    let res = match relay
        .client
        .get(url)
        .timeout(Duration::from_millis(timeout_ms))
        .headers(headers)
        .send()
        .await
    {
        Ok(res) => res,
        Err(err) => {
            RELAY_STATUS_CODE
                .with_label_values(&[
                    TIMEOUT_ERROR_CODE_STR,
                    GET_PROPOSER_DUTIES_ENDPOINT_TAG,
                    &relay.id,
                ])
                .inc();
            return Err(err.into());
        }
    };

    let request_latency = start_request.elapsed();
    RELAY_LATENCY
        .with_label_values(&[GET_PROPOSER_DUTIES_ENDPOINT_TAG, &relay.id])
        .observe(request_latency.as_secs_f64());

    let code = res.status();
    RELAY_STATUS_CODE
        .with_label_values(&[code.as_str(), GET_PROPOSER_DUTIES_ENDPOINT_TAG, &relay.id])
        .inc();

    let response_bytes = res.bytes().await?;
    if !code.is_success() {
        let err = PbsError::RelayResponse {
            error_msg: String::from_utf8_lossy(&response_bytes).into_owned(),
            code: code.as_u16(),
        };

        // we request payload to all relays, but some may have not received it
        warn!(?err, "failed to get payload (this might be ok if other relays have it)");
        return Err(err)
    };

    let response: GetProposerEpochResponse = serde_json::from_slice(&response_bytes)?;

    debug!(
        latency = ?request_latency,
        "received proposer epoch"
    );

    Ok(response)
}
