use serde::{Deserialize, Serialize};
use snafu::{OptionExt, ResultExt};

use cosmwasm::errors::{ContractErr, DynContractErr, ParseErr, Result, SerializeErr};
use cosmwasm::serde::{from_slice, to_vec};
use cosmwasm::storage::Storage;
use cosmwasm::types::{Params, Response};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct AddressState {
    balance: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct InitialBalance {
    address: String,
    amount: u64,
}

#[derive(Serialize, Deserialize)]
pub struct InitMsg {
    initial_balances: Vec<InitialBalance>,
    name: String,
    symbol: String,
    decimals: u8,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HandleMsg {
    Approve { spender: String, amount: u64 },
    Transfer { recipient: String, amount: u64 },
}

/**
 * We use well defined storage keys to allow for easy access of this contract's state
 * without the need to execute getter logic.
 *
 * - ascii("name") stores human readable name of the token (3-30 bytes as UTF-8)
 * - ascii("symbol") stores the ticker symbol ([A-Z]{3,6} as ASCII)
 * - ascii("decimals") stores the fractional digits (unsigned int8)
 * - ascii("total_supply") stores the total supply (big endian encoded unsigned int64)
 * - `address` stores balance data (as JSON) for a single address. `address`
 *   is always 20 bytes long and thus can not conflict with other keys.
 * - `owner` + `spender` stores allowance data (big endian encoded unsigned int64)
 *   for an owner spender pair address. `owner` + `spender` is always 40 bytes long
 *   and thus can not conflict with other keys.
 */
pub const KEY_TOTAL_SUPPLY: &[u8] = b"total_supply";
pub const KEY_NAME: &[u8] = b"name";
pub const KEY_SYMBOL: &[u8] = b"symbol";
pub const KEY_DECIMALS: &[u8] = b"decimals";

pub fn init<T: Storage>(store: &mut T, _params: Params, msg: Vec<u8>) -> Result<Response> {
    let msg: InitMsg = from_slice(&msg).context(ParseErr { kind: "InitMsg" })?;

    // Name
    if !is_valid_name(&msg.name) {
        return ContractErr {
            msg: "Name is not in the expected format (3-30 UTF-8 bytes)",
        }
        .fail();
    }
    store.set(KEY_NAME, msg.name.as_bytes());

    // Symbol
    if !is_valid_symbol(&msg.symbol) {
        return ContractErr {
            msg: "Ticker symbol is not in expected format [A-Z]{3,6}",
        }
        .fail();
    }
    store.set(KEY_SYMBOL, msg.symbol.as_bytes());

    // Decimals
    if msg.decimals > 18 {
        return ContractErr {
            msg: "Decimals must not exceed 18",
        }
        .fail();
    }
    store.set(KEY_DECIMALS, &msg.decimals.to_be_bytes());

    // Initial balances
    let mut total: u64 = 0;
    for row in msg.initial_balances {
        let raw_address = parse_20bytes_from_hex(&row.address)?;
        store.set(
            &raw_address,
            &to_vec(&AddressState {
                balance: row.amount,
            })
            .context(SerializeErr {
                kind: "AddressState",
            })?,
        );
        total += row.amount;
    }
    store.set(KEY_TOTAL_SUPPLY, &total.to_be_bytes());

    Ok(Response::default())
}

pub fn handle<T: Storage>(store: &mut T, params: Params, msg: Vec<u8>) -> Result<Response> {
    let msg: HandleMsg = from_slice(&msg).context(ParseErr { kind: "HandleMsg" })?;

    match msg {
        HandleMsg::Approve { spender, amount } => try_approve(store, params, &spender, amount),
        HandleMsg::Transfer { recipient, amount } => {
            try_transfer(store, params, &recipient, amount)
        }
    }
}

fn try_transfer<T: Storage>(
    store: &mut T,
    params: Params,
    recipient: &str,
    amount: u64,
) -> Result<Response> {
    let sender_address_raw = parse_20bytes_from_hex(&params.message.signer)?;
    let recipient_address_raw = parse_20bytes_from_hex(&recipient)?;

    perform_transfer(store, &sender_address_raw, &recipient_address_raw, amount)?;

    let res = Response {
        messages: vec![],
        log: Some("transfer successfull".to_string()),
        data: None,
    };
    Ok(res)
}

fn try_approve<T: Storage>(
    store: &mut T,
    params: Params,
    spender: &str,
    amount: u64,
) -> Result<Response> {
    let owner_address_raw = parse_20bytes_from_hex(&params.message.signer)?;
    let spender_address_raw = parse_20bytes_from_hex(&spender)?;
    let key = [&owner_address_raw[..], &spender_address_raw[..]].concat();
    store.set(&key, &amount.to_be_bytes());
    let res = Response {
        messages: vec![],
        log: Some("approve successfull".to_string()),
        data: None,
    };
    Ok(res)
}

fn perform_transfer<T: Storage>(
    store: &mut T,
    from: &[u8; 20],
    to: &[u8; 20],
    amount: u64,
) -> Result<()> {
    let account_data = store.get(from).context(ContractErr {
        msg: "Account not found for this message sender",
    })?;
    let mut from_account: AddressState = from_slice(&account_data).context(ParseErr {
        kind: "AddressState",
    })?;

    if from_account.balance < amount {
        return DynContractErr {
            msg: format!(
                "Insufficient funds: balance={}, required={}",
                from_account.balance, amount
            ),
        }
        .fail();
    }

    let mut to_account = match store.get(to) {
        Some(data) => from_slice(&data).context(ParseErr {
            kind: "AddressState",
        })?,
        None => AddressState { balance: 0 },
    };

    from_account.balance -= amount;
    to_account.balance += amount;

    store.set(
        from,
        &to_vec(&from_account).context(SerializeErr {
            kind: "AddressState",
        })?,
    );
    store.set(
        to,
        &to_vec(&to_account).context(SerializeErr {
            kind: "AddressState",
        })?,
    );

    Ok(())
}

fn parse_20bytes_from_hex(data: &str) -> Result<[u8; 20]> {
    use std::error::Error as StdError;
    let mut out = [0u8; 20];
    match hex::decode_to_slice(data, &mut out as &mut [u8]) {
        Ok(_) => Ok(out),
        Err(e) => DynContractErr {
            msg: format!("parsing hash: {}", e.description()),
        }
        .fail(),
    }
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
