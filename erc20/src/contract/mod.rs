use schemars::JsonSchema;

use cw_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use std::convert::TryInto;

use serde::{Deserialize, Serialize};
use snafu::ResultExt;

use cosmwasm::errors::{ContractErr, DynContractErr, Result, SerializeErr};
use cosmwasm::serde::to_vec;
use cosmwasm::traits::{Api, Extern, ReadonlyStorage, Storage};
use cosmwasm::types::{CanonicalAddr, HumanAddr, Params, Response};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct InitialBalance {
    pub address: HumanAddr,
    pub amount: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InitMsg {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub initial_balances: Vec<InitialBalance>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HandleMsg {
    Approve {
        spender: HumanAddr,
        amount: String,
    },
    Transfer {
        recipient: HumanAddr,
        amount: String,
    },
    TransferFrom {
        owner: HumanAddr,
        recipient: HumanAddr,
        amount: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum QueryMsg {
    Balance {
        address: HumanAddr,
    },
    Allowance {
        owner: HumanAddr,
        spender: HumanAddr,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct BalanceResponse {
    pub balance: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct AllowanceResponse {
    pub allowance: String,
}

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct Constants {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

pub const PREFIX_CONFIG: &[u8] = b"config";
pub const PREFIX_BALANCES: &[u8] = b"balances";
pub const PREFIX_ALLOWANCES: &[u8] = b"allowances";

pub const KEY_CONSTANTS: &[u8] = b"constants";
pub const KEY_TOTAL_SUPPLY: &[u8] = b"total_supply";

pub fn init<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    _params: Params,
    msg: InitMsg,
) -> Result<Response> {
    let mut total_supply: u128 = 0;
    {
        // Initial balances
        let mut balances_store = PrefixedStorage::new(PREFIX_BALANCES, &mut deps.storage);
        for row in msg.initial_balances {
            let raw_address = deps.api.canonical_address(&row.address)?;
            let amount_raw = parse_u128(&row.amount)?;
            balances_store.set(raw_address.as_bytes(), &amount_raw.to_be_bytes());
            total_supply += amount_raw;
        }
    }

    // Check name, symbol, decimals
    if !is_valid_name(&msg.name) {
        return ContractErr {
            msg: "Name is not in the expected format (3-30 UTF-8 bytes)",
        }
        .fail();
    }
    if !is_valid_symbol(&msg.symbol) {
        return ContractErr {
            msg: "Ticker symbol is not in expected format [A-Z]{3,6}",
        }
        .fail();
    }
    if msg.decimals > 18 {
        return ContractErr {
            msg: "Decimals must not exceed 18",
        }
        .fail();
    }

    let mut config_store = PrefixedStorage::new(PREFIX_CONFIG, &mut deps.storage);
    let constants = to_vec(&Constants {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
    })
    .context(SerializeErr { kind: "Constants" })?;
    config_store.set(KEY_CONSTANTS, &constants);
    config_store.set(KEY_TOTAL_SUPPLY, &total_supply.to_be_bytes());

    Ok(Response::default())
}

pub fn handle<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    params: Params,
    msg: HandleMsg,
) -> Result<Response> {
    match msg {
        HandleMsg::Approve { spender, amount } => try_approve(deps, params, &spender, &amount),
        HandleMsg::Transfer { recipient, amount } => {
            try_transfer(deps, params, &recipient, &amount)
        }
        HandleMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => try_transfer_from(deps, params, &owner, &recipient, &amount),
    }
}

pub fn query<S: Storage, A: Api>(deps: &Extern<S, A>, msg: QueryMsg) -> Result<Vec<u8>> {
    match msg {
        QueryMsg::Balance { address } => {
            let address_key = deps.api.canonical_address(&address)?;
            let balance = read_balance(&deps.storage, &address_key)?;
            let out = to_vec(&BalanceResponse {
                balance: balance.to_string(),
            })
            .context(SerializeErr {
                kind: "BalanceResponse",
            })?;
            Ok(out)
        }
        QueryMsg::Allowance { owner, spender } => {
            let owner_key = deps.api.canonical_address(&owner)?;
            let spender_key = deps.api.canonical_address(&spender)?;
            let allowance = read_allowance(&deps.storage, &owner_key, &spender_key)?;
            let out = to_vec(&AllowanceResponse {
                allowance: allowance.to_string(),
            })
            .context(SerializeErr {
                kind: "AllowanceResponse",
            })?;
            Ok(out)
        }
    }
}

fn try_transfer<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    params: Params,
    recipient: &HumanAddr,
    amount: &str,
) -> Result<Response> {
    let sender_address_raw = &params.message.signer;
    let recipient_address_raw = deps.api.canonical_address(recipient)?;
    let amount_raw = parse_u128(amount)?;

    perform_transfer(
        &mut deps.storage,
        &sender_address_raw,
        &recipient_address_raw,
        amount_raw,
    )?;

    let res = Response {
        messages: vec![],
        log: Some("transfer successful".to_string()),
        data: None,
    };
    Ok(res)
}

fn try_transfer_from<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    params: Params,
    owner: &HumanAddr,
    recipient: &HumanAddr,
    amount: &str,
) -> Result<Response> {
    let spender_address_raw = &params.message.signer;
    let owner_address_raw = deps.api.canonical_address(owner)?;
    let recipient_address_raw = deps.api.canonical_address(recipient)?;
    let amount_raw = parse_u128(amount)?;

    let mut allowance = read_allowance(&deps.storage, &owner_address_raw, &spender_address_raw)?;
    if allowance < amount_raw {
        return DynContractErr {
            msg: format!(
                "Insufficient allowance: allowance={}, required={}",
                allowance, amount_raw
            ),
        }
        .fail();
    }
    allowance -= amount_raw;
    write_allowance(
        &mut deps.storage,
        &owner_address_raw,
        &spender_address_raw,
        allowance,
    );
    perform_transfer(
        &mut deps.storage,
        &owner_address_raw,
        &recipient_address_raw,
        amount_raw,
    )?;

    let res = Response {
        messages: vec![],
        log: Some("transfer from successful".to_string()),
        data: None,
    };
    Ok(res)
}

fn try_approve<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    params: Params,
    spender: &HumanAddr,
    amount: &str,
) -> Result<Response> {
    let owner_address_raw = &params.message.signer;
    let spender_address_raw = deps.api.canonical_address(spender)?;
    let amount_raw = parse_u128(amount)?;
    write_allowance(
        &mut deps.storage,
        &owner_address_raw,
        &spender_address_raw,
        amount_raw,
    );
    let res = Response {
        messages: vec![],
        log: Some("approve successful".to_string()),
        data: None,
    };
    Ok(res)
}

fn perform_transfer<T: Storage>(
    store: &mut T,
    from: &CanonicalAddr,
    to: &CanonicalAddr,
    amount: u128,
) -> Result<()> {
    let mut balances_store = PrefixedStorage::new(PREFIX_BALANCES, store);

    let mut from_balance = read_u128(&balances_store, from.as_bytes())?;
    if from_balance < amount {
        return DynContractErr {
            msg: format!(
                "Insufficient funds: balance={}, required={}",
                from_balance, amount
            ),
        }
        .fail();
    }
    from_balance -= amount;
    balances_store.set(from.as_bytes(), &from_balance.to_be_bytes());

    let mut to_balance = read_u128(&balances_store, to.as_bytes())?;
    to_balance += amount;
    balances_store.set(to.as_bytes(), &to_balance.to_be_bytes());

    Ok(())
}

// Converts 16 bytes value into u128
// Errors if data found that is not 16 bytes
pub fn bytes_to_u128(data: &[u8]) -> Result<u128> {
    match data[0..16].try_into() {
        Ok(bytes) => Ok(u128::from_be_bytes(bytes)),
        Err(_) => ContractErr {
            msg: "Corrupted data found. 16 byte expected.",
        }
        .fail(),
    }
}

// Reads 16 byte storage value into u128
// Returns zero if key does not exist. Errors if data found that is not 16 bytes
pub fn read_u128<S: ReadonlyStorage>(store: &S, key: &[u8]) -> Result<u128> {
    return match store.get(key) {
        Some(data) => bytes_to_u128(&data),
        None => Ok(0u128),
    };
}

// Source must be a decadic integer >= 0
pub fn parse_u128(source: &str) -> Result<u128> {
    match source.parse::<u128>() {
        Ok(value) => Ok(value),
        Err(_) => ContractErr {
            msg: "Error while parsing string to u128",
        }
        .fail(),
    }
}

fn read_balance<S: Storage>(store: &S, owner: &CanonicalAddr) -> Result<u128> {
    let balance_store = ReadonlyPrefixedStorage::new(PREFIX_BALANCES, store);
    return read_u128(&balance_store, owner.as_bytes());
}

fn read_allowance<S: Storage>(
    store: &S,
    owner: &CanonicalAddr,
    spender: &CanonicalAddr,
) -> Result<u128> {
    let allowances_store = ReadonlyPrefixedStorage::new(PREFIX_ALLOWANCES, store);
    let owner_store = ReadonlyPrefixedStorage::new(owner.as_bytes(), &allowances_store);
    return read_u128(&owner_store, spender.as_bytes());
}

fn write_allowance<S: Storage>(
    store: &mut S,
    owner: &CanonicalAddr,
    spender: &CanonicalAddr,
    amount: u128,
) -> () {
    let mut allowances_store = PrefixedStorage::new(PREFIX_ALLOWANCES, store);
    let mut owner_store = PrefixedStorage::new(owner.as_bytes(), &mut allowances_store);
    owner_store.set(spender.as_bytes(), &amount.to_be_bytes());
}

fn is_valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() < 3 || bytes.len() > 30 {
        return false;
    }
    return true;
}

fn is_valid_symbol(symbol: &str) -> bool {
    let bytes = symbol.as_bytes();
    if bytes.len() < 3 || bytes.len() > 6 {
        return false;
    }

    for byte in bytes.iter() {
        if *byte < 65 || *byte > 90 {
            return false;
        }
    }

    return true;
}

#[cfg(test)]
mod tests;
