use std::{net::SocketAddr, sync::Arc};

use axum::{http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use commit_boost::prelude::*;
use eyre::{bail, Result};
use lazy_static::lazy_static;
use prometheus::{IntCounter, Registry};
use serde::Deserialize;
use serde_json::json;
use tokio::{net::TcpListener, sync::RwLock};
use tracing::{error, info};

// You can define custom metrics and a custom registry for the business logic of
// your module. These will be automatically scraped by the Prometheus server
lazy_static! {
    pub static ref MY_CUSTOM_REGISTRY: prometheus::Registry =
        Registry::new_custom(Some("preconf".to_string()), None).unwrap();
    pub static ref VAL_RECEIVED_COUNTER: IntCounter =
        IntCounter::new("validators_received", "successful validators requests received").unwrap();
}

struct PreconfService {
    config: StartCommitModuleConfig<ExtraConfig>,
}

#[derive(Debug, Deserialize)]
struct ExtraConfig {
    port: u16,
}

#[derive(Clone)]
struct AppState {
    service: Arc<RwLock<PreconfService>>,
}

impl PreconfService {
    pub async fn run(self) -> Result<()> {
        let port = self.config.extra.port;
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
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    initialize_tracing_log();

    // Remember to register all your metrics before starting the process
    MY_CUSTOM_REGISTRY.register(Box::new(VAL_RECEIVED_COUNTER.clone()))?;
    // Spin up a server that exposes the /metrics endpoint to Prometheus
    MetricsProvider::load_and_run(MY_CUSTOM_REGISTRY.clone())?;

    match load_commit_module_config::<ExtraConfig>() {
        Ok(config) => {
            info!(
                module_id = config.id,
                port = config.extra.port,
                "Starting module with custom data"
            );

            let service = PreconfService { config };

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
