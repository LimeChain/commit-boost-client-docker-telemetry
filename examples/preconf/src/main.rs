use commit_boost::prelude::*;
use eyre::Result;
use tracing::{error, info};

struct PreconfService {
    config: StartCommitModuleConfig<()>,
}

impl PreconfService {
    pub async fn run(self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    initialize_tracing_log();

    match load_commit_module_config::<()>() {
        Ok(config) => {
            info!(module_id = config.id, "Starting module with custom data");

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
