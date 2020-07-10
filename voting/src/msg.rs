use crate::state::PollStatus;
use cosmwasm_std::{HumanAddr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    CastVote {
        poll_id: u64,
        encrypted_vote: String,
        weight: Uint128,
    },
    StakeVotingTokens {},
    WithdrawVotingTokens {
        amount: Option<Uint128>,
    },
    CreatePoll {
        quorum_percentage: Option<u8>,
        description: String,
        start_height: Option<u64>,
        end_height: Option<u64>,
    },
    EndPoll {
        poll_id: u64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    TokenStake { address: HumanAddr },
    Poll { poll_id: u64 },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct PollResponse {
    pub creator: HumanAddr,
    pub status: PollStatus,
    pub quorum_percentage: Option<u8>,
    pub end_height: Option<u64>,
    pub start_height: Option<u64>,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct CreatePollResponse {
    pub poll_id: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct PollCountResponse {
    pub poll_count: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct TokenStakeResponse {
    pub token_balance: Uint128,
}
