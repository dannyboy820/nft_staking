use cosmwasm_std::{Coin, StdError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{}", original)]
    Std {
        // let thiserror implement From<StdError> for you
        #[from]
        original: StdError,
    },

    #[error("expired option (expired {expired:?})")]
    OptionExpired { expired: u64 },

    #[error("not expired option (expires {expires:?})")]
    OptionNotExpired { expires: u64 },

    #[error("unauthorized")]
    Unauthorized {},

    #[error("must send exact counter offer (offer {offer:?}, counter_offer: {counter_offer:?})")]
    CounterOfferMismatch {
        offer: Vec<Coin>,
        counter_offer: Vec<Coin>,
    },

    #[error("do not send funds with burn")]
    FundsSentWithBurn {},
}
