use named_type::NamedType;
use named_type_derive::NamedType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm::traits::Storage;
use cosmwasm::types::{CanonicalAddr, Coin};
use cw_storage::{bucket, bucket_read, singleton, Bucket, ReadonlyBucket, Singleton};

pub static NAME_RESOLVER_KEY: &[u8] = b"nameresolver";
pub static CONFIG_KEY: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, NamedType)]
pub struct Config {
    pub purchase_price: Option<Coin>,
    pub transfer_price: Option<Coin>,
}

pub fn config<S: Storage>(storage: &mut S) -> Singleton<S, Config> {
    singleton(storage, CONFIG_KEY)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, NamedType)]
pub struct NameRecord {
    pub owner: CanonicalAddr,
}

pub fn resolver<S: Storage>(storage: &mut S) -> Bucket<S, NameRecord> {
    bucket(NAME_RESOLVER_KEY, storage)
}

pub fn resolver_read<S: Storage>(storage: &S) -> ReadonlyBucket<S, NameRecord> {
    bucket_read(NAME_RESOLVER_KEY, storage)
}
