pub(crate) const STATUS_ENDPOINT_TAG: &str = "status";
pub(crate) const REGISTER_VALIDATOR_ENDPOINT_TAG: &str = "register_validator";
pub(crate) const SUBMIT_BLINDED_BLOCK_ENDPOINT_TAG: &str = "submit_blinded_block";
pub(crate) const GET_HEADER_ENDPOINT_TAG: &str = "get_header";
pub(crate) const GET_PROPOSER_DUTIES_ENDPOINT_TAG: &str = "get_proposer_duties";

/// For metrics recorded when a request times out
pub(crate) const TIMEOUT_ERROR_CODE: u16 = 555;
pub(crate) const TIMEOUT_ERROR_CODE_STR: &str = "555";
