use named_type::NamedType;
use named_type_derive::NamedType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm::traits::Storage;
use cosmwasm::types::CanonicalAddr;
use cw_storage::{bucket, bucket_read, Bucket, ReadonlyBucket};

pub static NAME_RESOLVER_KEY: &[u8] = b"nameresolver";
pub static CONFIG_KEY: &[u8] = b"config";

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
