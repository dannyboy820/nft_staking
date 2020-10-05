use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("unauthorized")]
    Unauthorized {},

    #[error("insufficient funds sent")]
    InsufficientFundsSend {},

    #[error("name does not exist (name {name})")]
    NameNotExists { name: String },

    #[error("name has been taken (name {name})")]
    NameTaken { name: String },

    #[error("name too short (length {length} min_length {min_length})")]
    NameTooShort { length: u64, min_length: u64 },

    #[error("name too long (length {length} min_length {max_length})")]
    NameTooLong { length: u64, max_length: u64 },

    #[error("invalid character( char {c}")]
    InvalidCharacter { c: char },
}
