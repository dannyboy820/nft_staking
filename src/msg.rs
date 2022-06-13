use cosmwasm_std::{Addr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cw721::Cw721ReceiveMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub arbiter: String,
    pub recipient: String,
    /// When end height set and block height exceeds this value, the escrow is expired.
    /// Once an escrow is expired, it can be returned to the original funder (via "refund").
    pub end_height: Option<u64>,
    /// When end time (in seconds since epoch 00:00:00 UTC on 1 January 1970) is set and
    /// block time exceeds this value, the escrow is expired.
    /// Once an escrow is expired, it can be returned to the original funder (via "refund").
    pub end_time: Option<u64>,
    pub admin: Option<String>,
    pub nft_721_contract_addr_whitelist: Vec<String>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // Approve {
    //     // release some coins - if quantity is None, release all coins in balance
    //     quantity: Option<Vec<Coin>>,
    // },
    // Refund {},
    UpdateContractInfo(UpdateContractInfoMsg),
    CreateCollectionPool(CreateCollectionPoolMsg),
    UpdateCollectionPool(UpdateCollectionPoolMsg),
    ReceiveNft(Cw721ReceiveMsg),
    Withdraw {
        collection_id: String,
        withdraw_rewards: bool,
        withdraw_nft_ids: Vec<String>,
    },
    // Claim {
    //     collection_id: String,
    // },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct UpdateContractInfoMsg {
    pub nft_721_contract_addr_whitelist: Option<Vec<String>>,
    pub admin: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct CreateCollectionPoolMsg {
    pub collection_id: String,
    pub reward_per_block: Uint128,
    pub expired_after: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct UpdateCollectionPoolMsg {
    pub collection_id: String,
    pub reward_per_block: Option<Uint128>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct DepositeMsg {
    pub collection_id: String,
    pub withdraw_rewards: bool,
    pub signature_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Returns a human-readable representation of the arbiter.
    Arbiter {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ArbiterResponse {
    pub arbiter: Addr,
}
