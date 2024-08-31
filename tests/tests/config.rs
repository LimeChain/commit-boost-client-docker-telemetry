use cb_common::{config::CommitBoostConfig, types::Chain};
use eyre::Result;

#[tokio::test]
async fn test_load_config() -> Result<()> {
    let config = CommitBoostConfig::from_file("../config.example.toml")?;

    assert_eq!(config.chain, Chain::Custom);
    // TODO: add more
    Ok(())
}
