use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm::types::{Coin, HumanAddr};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub arbiter: HumanAddr,
    pub recipient: HumanAddr,
    /// When set, this is the last height at which the escrow is valid. After that height,
    /// the escrow is expired and can be returned to the original funder (via "refund").
    pub end_height: Option<i64>,
    /// When set, this is the last time (in seconds since epoch 00:00:00 UTC on 1 January 1970)
    /// at which the escrow is valid. After that time, the escrow is expired and can be
    /// returned to the original funder (via "refund").
    pub end_time: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HandleMsg {
    Approve {
        // release some coins - if quantity is None, release all coins in balance
        quantity: Option<Vec<Coin>>,
    },
    Refund {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum QueryMsg {}
