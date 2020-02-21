use cosmwasm::types::{Coin, HumanAddr};
use named_type::NamedType;
use named_type_derive::NamedType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub name: String,
    pub purchase_price: Option<Coin>,
    pub transfer_price: Option<Coin>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HandleMsg {
    Register { name: String },
    Transfer { name: String, to: HumanAddr },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum QueryMsg {
    // ResolveAddress returns the current address that the name resolves to
    ResolveRecord { name: String },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, NamedType)]
pub struct ResolveRecordResponse {
    pub address: HumanAddr,
}
