use cosmwasm_std::{CanonicalAddr, StdError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{}", original)]
    Std (#[from] StdError),

    #[error("insufficient funds sent")]
    InsufficientFunds {},

    #[error("excessive withdrawal amount (max_amount {max_amount:?})")]
    ExcessiveWithdraw { max_amount: u128 },

    #[error("description too short (minimum description length {min_desc_length:?})")]
    DescriptionTooShort { min_desc_length: usize },

    #[error("description too long (maximum description length {max_desc_length:?})")]
    DescriptionTooLong { max_desc_length: usize },

    #[error("no stake")]
    PollNoStake {},

    #[error("poll do not exist")]
    PollNotExist {},

    #[error("poll cannot end in past")]
    PollCannotEndInPast {},

    #[error("sender is not the creator of the poll (sender {sender:?} creator {creator:?})")]
    PollNotCreator {
        sender: CanonicalAddr,
        creator: CanonicalAddr,
    },

    #[error("poll is not in progress")]
    PollNotInProgress {},

    #[error("poll voting period has not started (start_height {start_height:?})")]
    PoolVotingPeriodNotStarted { start_height: u64 },

    #[error("poll voting period has not expired (expire_height {expire_height:?})")]
    PollVotingPeriodNotExpired { expire_height: u64 },

    #[error("sender has already voted in poll")]
    PollSenderVoted {},

    #[error("sender staked tokens insufficient")]
    PollInsufficientStake {},

    #[error("quorum percentage must be 0 to 100 ( quorum_percentage: {quorum_percentage:?})")]
    PollQuorumPercentageMismatch { quorum_percentage: u8 },
}
