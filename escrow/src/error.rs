use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("unauthorized")]
    Unauthorized {},

    #[error("escrow expired (end_height {end_height})")]
    Expired { end_height: u64, end_time: u64 },

    #[error("escrow not expired")]
    NotExpired {},
}
