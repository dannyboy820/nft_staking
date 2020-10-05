use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("unauthorized")]
    Unauthorized {},

    #[error("escrow expired (end_height {end_height})")]
    EscrowExpiredHeight { end_height: u64 },

    #[error("escrow expired (end_time {end_time})")]
    EscrowExpiredTime { end_time: u64 },

    #[error("escrow not expired")]
    EscrowNotExpired {},
}
