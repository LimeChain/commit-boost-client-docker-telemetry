pub const MAX_TOP_TRANSACTIONS: usize = 1000;
pub const MAX_REST_TRANSACTIONS: usize = 1000;

pub const ELECT_PRECONFER_PATH: &str = "/eth/v1/builder/elect_preconfer";
pub const SET_CONSTRAINTS_PATH: &str = "/eth/v1/builder/set_constraints";
pub const GET_NEXT_ACTIVE_SLOT: &str = "/eth/v1/builder/next_active_slot/:pubkey";
