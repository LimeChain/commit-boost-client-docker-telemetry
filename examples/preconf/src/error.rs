#[derive(Debug, thiserror::Error)]
pub enum PreconfError {
    #[error("serde decode error: {0}")]
    SerdeDecodeError(#[from] serde_json::Error),

    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
}
