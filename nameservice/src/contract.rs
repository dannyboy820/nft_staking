use cosmwasm::errors::{contract_err, dyn_contract_err, unauthorized, Result};
use cosmwasm::traits::{Api, Extern, Storage};
use cosmwasm::types::{Env, HumanAddr, Response};

use crate::coin_helpers::assert_sent_sufficient_coin;
use crate::msg::{HandleMsg, InitMsg, QueryMsg, ResolveRecordResponse};
use crate::state::{config, config_read, resolver, resolver_read, Config, NameRecord};

use cw_storage::serialize;

const MIN_NAME_LENGTH: usize = 3;
const MAX_NAME_LENGTH: usize = 64;

pub fn init<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    _env: Env,
    msg: InitMsg,
) -> Result<Response> {
    let config_state = Config {
        purchase_price: msg.purchase_price,
        transfer_price: msg.transfer_price,
    };

    config(&mut deps.storage).save(&config_state)?;

    Ok(Response::default())
}

pub fn handle<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    env: Env,
    msg: HandleMsg,
) -> Result<Response> {
    match msg {
        HandleMsg::Register { name } => try_register(deps, env, name),
        HandleMsg::Transfer { name, to } => try_transfer(deps, env, name, to),
    }
}

pub fn try_register<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    env: Env,
    name: String,
) -> Result<Response> {
    // we only need to check here - at point of registration
    validate_name(&name)?;
    let config_state = config(&mut deps.storage).load()?;
    assert_sent_sufficient_coin(&env.message.sent_funds, config_state.purchase_price)?;

    let key = name.as_bytes();
    let record = NameRecord {
        owner: env.message.signer,
    };

    if let None = resolver(&mut deps.storage).may_load(key)? {
        // name is available
        resolver(&mut deps.storage).save(key, &record)?;
    } else {
        // name is already taken
        contract_err("Name is already taken")?;
    }

    Ok(Response::default())
}

pub fn try_transfer<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    env: Env,
    name: String,
    to: HumanAddr,
) -> Result<Response> {
    let config_state = config(&mut deps.storage).load()?;
    assert_sent_sufficient_coin(&env.message.sent_funds, config_state.transfer_price)?;

    let key = name.as_bytes();
    let new_owner = deps.api.canonical_address(&to)?;

    resolver(&mut deps.storage).update(key, &|record| {
        if let Some(mut record) = record {
            if env.message.signer != record.owner {
                unauthorized()?;
            }

            record.owner = new_owner.clone();
            Ok(record)
        } else {
            contract_err("Name does not exist")
        }
    })?;
    Ok(Response::default())
}

pub fn query<S: Storage, A: Api>(deps: &Extern<S, A>, msg: QueryMsg) -> Result<Vec<u8>> {
    match msg {
        QueryMsg::ResolveRecord { name } => query_resolver(deps, name),
        QueryMsg::Config {} => serialize(&config_read(&deps.storage).load()?),
    }
}

fn query_resolver<S: Storage, A: Api>(deps: &Extern<S, A>, name: String) -> Result<Vec<u8>> {
    let key = name.as_bytes();

    let address = match resolver_read(&deps.storage).may_load(key)? {
        Some(record) => Some(deps.api.human_address(&record.owner)?),
        None => None,
    };
    let resp = ResolveRecordResponse { address };

    serialize(&resp)
}

// let's not import a regexp library and just do these checks by hand
fn invalid_char(ch: &char) -> bool {
    let c = *ch;
    if c >= '0' && c <= '9' {
        false
    } else if c >= 'a' && c <= 'z' {
        false
    } else if c == '.' || c == '-' || c == '_' {
        false
    } else {
        true
    }
}

/// validate_name returns an error if the name is invalid
/// (we require 3-64 lowercase ascii letters, numbers, or . - _)
fn validate_name(name: &str) -> Result<()> {
    if name.len() < MIN_NAME_LENGTH {
        contract_err("Name too short")
    } else if name.len() > MAX_NAME_LENGTH {
        contract_err("Name too long")
    } else {
        match name.chars().find(invalid_char) {
            None => Ok(()),
            Some(c) => dyn_contract_err(format!("Invalid character: '{}'", c)),
        }
    }
}
