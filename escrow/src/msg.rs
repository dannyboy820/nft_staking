use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm::types::{Coin, HumanAddr};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
  pub arbiter: HumanAddr,
  pub recipient: HumanAddr,
  // you can set a last time or block height the contract is valid at
  // if *either* is non-zero and below current state, the contract is considered expired
  // and will be returned to the original funder
  pub end_height: i64,
  pub end_time: i64,
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
