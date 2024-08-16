use std::{net::SocketAddr, sync::Arc};

use api::PreconfService;
use beacon_client::client::MultiBeaconClient;
use commit_boost::prelude::*;
use config::ExtraConfig;
use elector::PreconfElector;
use eyre::{bail, Result};
use lazy_static::lazy_static;
use prometheus::{IntCounter, Registry};
use tokio::{
    net::TcpListener,
    sync::{mpsc, RwLock},
};
use tracing::{error, info};
use types::AppState;

use crate::api::create_router;

mod api;
mod beacon_client;
mod config;
mod constants;
mod elector;
mod types;

// You can define custom metrics and a custom registry for the business logic of
// your module. These will be automatically scraped by the Prometheus server
lazy_static! {
    pub static ref MY_CUSTOM_REGISTRY: prometheus::Registry =
        Registry::new_custom(Some("preconf".to_string()), None).unwrap();
    pub static ref VAL_RECEIVED_COUNTER: IntCounter =
        IntCounter::new("validators_received", "successful validators requests received").unwrap();
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    initialize_tracing_log();

    // Register metrics
    MY_CUSTOM_REGISTRY.register(Box::new(VAL_RECEIVED_COUNTER.clone()))?;
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

            let service = PreconfService::new(config.clone()).await;
            let app_state = AppState { service: Arc::new(RwLock::new(service)) };

            let router = create_router(app_state);
            let port = config.server_port;
            let address = SocketAddr::from(([0, 0, 0, 0], port));
            let listener = TcpListener::bind(&address).await?;

            tokio::spawn(async move {
                if let Err(err) = axum::serve(listener, router).await {
                    error!(?err, "Axum server encountered an error");
                }
            });

            let elector = PreconfElector::new(config.clone(), duties_rx);

            if let Err(err) = elector.run().await {
                error!(?err, "Error running elector")
            }

            bail!("Server stopped unexpectedly");
        }
        Err(err) => {
            error!(?err, "Failed to load module config");
        }
    }

    Ok(())
}
