use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, HandleResponse, HumanAddr, InitResponse, InitResult,
    MessageInfo, StdResult,
};

use crate::coin_helpers::assert_sent_sufficient_coin;
use crate::error::ContractError;
use crate::msg::{HandleMsg, InitMsg, QueryMsg, ResolveRecordResponse};
use crate::state::{config, config_read, resolver, resolver_read, Config, NameRecord};

const MIN_NAME_LENGTH: u64 = 3;
const MAX_NAME_LENGTH: u64 = 64;

pub fn init(deps: DepsMut, _env: Env, _info: MessageInfo, msg: InitMsg) -> InitResult {
    let config_state = Config {
        purchase_price: msg.purchase_price,
        transfer_price: msg.transfer_price,
    };

    config(deps.storage).save(&config_state)?;

    Ok(InitResponse::default())
}

pub fn handle(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: HandleMsg,
) -> Result<HandleResponse, ContractError> {
    match msg {
        HandleMsg::Register { name } => try_register(deps, env, info, name),
        HandleMsg::Transfer { name, to } => try_transfer(deps, env, info, name, to),
    }
}

pub fn try_register(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    name: String,
) -> Result<HandleResponse, ContractError> {
    // we only need to check here - at point of registration
    validate_name(&name)?;
    let config_state = config(deps.storage).load()?;
    assert_sent_sufficient_coin(&info.sent_funds, config_state.purchase_price)?;

    let key = name.as_bytes();
    let record = NameRecord {
        owner: deps.api.canonical_address(&info.sender)?,
    };

    if (resolver(deps.storage).may_load(key)?).is_some() {
        // name is already taken
        return Err(ContractError::NameTaken { name });
    }

    // name is available
    resolver(deps.storage).save(key, &record)?;

    Ok(HandleResponse::default())
}

pub fn try_transfer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    name: String,
    to: HumanAddr,
) -> Result<HandleResponse, ContractError> {
    let api = deps.api;
    let config_state = config(deps.storage).load()?;
    assert_sent_sufficient_coin(&info.sent_funds, config_state.transfer_price)?;

    let new_owner = deps.api.canonical_address(&to)?;
    let key = name.as_bytes();
    resolver(deps.storage).update(key, |record| {
        if let Some(mut record) = record {
            if api.canonical_address(&info.sender)? != record.owner {
                return Err(ContractError::Unauthorized {});
            }

            record.owner = new_owner.clone();
            Ok(record)
        } else {
            Err(ContractError::NameNotExists { name: name.clone() })
        }
    })?;
    Ok(HandleResponse::default())
}

pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ResolveRecord { name } => query_resolver(deps, env, name),
        QueryMsg::Config {} => to_binary(&config_read(deps.storage).load()?),
    }
}

fn query_resolver(deps: Deps, _env: Env, name: String) -> StdResult<Binary> {
    let key = name.as_bytes();

    let address = match resolver_read(deps.storage).may_load(key)? {
        Some(record) => Some(deps.api.human_address(&record.owner)?),
        None => None,
    };
    let resp = ResolveRecordResponse { address };

    to_binary(&resp)
}

// let's not import a regexp library and just do these checks by hand
fn invalid_char(c: char) -> bool {
    let is_valid =
        (c >= '0' && c <= '9') || (c >= 'a' && c <= 'z') || (c == '.' || c == '-' || c == '_');
    !is_valid
}

/// validate_name returns an error if the name is invalid
/// (we require 3-64 lowercase ascii letters, numbers, or . - _)
fn validate_name(name: &str) -> Result<(), ContractError> {
    let length = name.len() as u64;
    if (name.len() as u64) < MIN_NAME_LENGTH {
        Err(ContractError::NameTooShort {
            length,
            min_length: MIN_NAME_LENGTH,
        })
    } else if (name.len() as u64) > MAX_NAME_LENGTH {
        Err(ContractError::NameTooLong {
            length,
            max_length: MAX_NAME_LENGTH,
        })
    } else {
        match name.find(invalid_char) {
            None => Ok(()),
            Some(bytepos_invalid_char_start) => {
                let c = name[bytepos_invalid_char_start..].chars().next().unwrap();
                Err(ContractError::InvalidCharacter { c })
            }
        }
    }
}
