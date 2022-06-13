use cosmwasm_std::{Addr, Env, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");

pub const COLLECTION_POOL_INFO: Map<&[u8], CollectionPoolInfo> = Map::new("collection_pool_info_map");

pub const STAKING_INFO: Map<&[u8], StakerInfo> = Map::new("staker_info_map");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ContractInfo {
    pub source: Addr,
    pub end_height: Option<u64>,
    pub end_time: Option<u64>,
    pub admin: Option<String>,
    pub nft_721_contract_addr_whitelist: Vec<String>
}

impl ContractInfo {
    pub fn is_expired(&self, env: &Env) -> bool {
        if let Some(end_height) = self.end_height {
            if env.block.height > end_height {
                return true;
            }
        }

        if let Some(end_time) = self.end_time {
            if env.block.time.nanos() > end_time * 1000 {
                return true;
            }
        }
        false
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct CollectionPoolInfo {
    pub collection_id: String,
    pub reward_per_block: Uint128,
    pub total_nfts: Uint128,
    pub acc_per_share: Uint128,
    pub last_reward_block: u64,
    pub expired_block: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct StakerInfo {
    pub total_staked: Uint128,
    pub reward_debt: Uint128,
    pub pending: Uint128,
    pub total_earned: Uint128,
    pub staked_tokens: Vec<CollectionStakedTokenInfo>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct CollectionStakedTokenInfo {
    pub token_id: String,
    pub contract_addr: Addr,
}


