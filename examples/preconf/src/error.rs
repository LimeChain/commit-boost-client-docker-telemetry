#[derive(Debug, thiserror::Error)]
pub enum PreconfError {
    #[error("serde decode error: {0}")]
    SerdeDecodeError(#[from] serde_json::Error),

    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("relay response error. Code: {code}, err: {error_msg}")]
    RelayResponse { error_msg: String, code: u16 },
}
