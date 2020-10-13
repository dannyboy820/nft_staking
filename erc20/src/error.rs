use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("unauthorized")]
    Unauthorized {},

    #[error("must reflect at least one message")]
    NoReflectMsg {},

    #[error("Name is not in the expected format (3-30 UTF-8 bytes)")]
    NameWrongFormat {},

    #[error("Ticker symbol is not in expected format [A-Z]{{3,6}}")]
    TickerWrongSymbolFormat {},

    #[error("Decimals must not exceed 18")]
    DecimalsExceeded {},

    #[error("Insufficient allowance (allowance {allowance}, required={required})")]
    InsufficientAllowance { allowance: u128, required: u128 },

    #[error("Insufficient funds (balance {balance}, required={required})")]
    InsufficientFunds { balance: u128, required: u128 },

    #[error("Corrupted data found 16 byte expected")]
    CorruptedDataFound {},
}
/*
StdError::generic_err(
            "",
        )
StdError::generic_err(
            "Name is not in the expected format (3-30 UTF-8 bytes)",
        )
        StdError::generic_err(

        )
        StdError::generic_err()
        StdError::generic_err(format!(
            "Insufficient allowance: allowance={}, required={}",
            allowance, amount_raw
        ))
        StdError::generic_err(format!(
            "insufficient funds to burn: balance={}, required={}",
            account_balance, amount_raw
        )
 */
