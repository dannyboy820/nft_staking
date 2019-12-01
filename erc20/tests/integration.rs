use cosmwasm::errors::ParseErr;
use cosmwasm::serde::{from_slice, to_vec};
use cosmwasm::storage::Storage;
use cosmwasm::types::{mock_params, Coin, Params};
use cosmwasm_vm::testing::{init, mock_instance};
use std::convert::TryInto;

use snafu::ResultExt;

use erc20::contract::{
    parse_20bytes_from_hex, AddressState, InitMsg, InitialBalance, KEY_DECIMALS, KEY_NAME,
    KEY_SYMBOL, KEY_TOTAL_SUPPLY,
};

static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/erc20.wasm");

fn init_msg() -> Vec<u8> {
    to_vec(&InitMsg {
        decimals: 5,
        name: "Ash token".to_string(),
        symbol: "ASH".to_string(),
        initial_balances: [
            InitialBalance {
                address: "0000000000000000000000000000000000000000".to_string(),
                amount: 11,
            },
            InitialBalance {
                address: "1111111111111111111111111111111111111111".to_string(),
                amount: 22,
            },
            InitialBalance {
                address: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                amount: 33,
            },
        ]
        .to_vec(),
    })
    .unwrap()
}

fn mock_params_height(
    signer: &str,
    sent: &[Coin],
    balance: &[Coin],
    height: i64,
    time: i64,
) -> Params {
    let mut params = mock_params(signer, sent, balance);
    params.block.height = height;
    params.block.time = time;
    params
}

fn get_name<T: Storage>(store: &T) -> String {
    let data = store.get(KEY_NAME).expect("no name data stored");
    let value = String::from_utf8(data).unwrap();
    return value;
}

fn get_symbol<T: Storage>(store: &T) -> String {
    let data = store.get(KEY_SYMBOL).expect("no symbol data stored");
    let value = String::from_utf8(data).unwrap();
    return value;
}

fn get_decimals<T: Storage>(store: &T) -> u8 {
    let data = store.get(KEY_DECIMALS).expect("no decimals data stored");
    let value = u8::from_be_bytes(data[0..1].try_into().unwrap());
    return value;
}

fn get_total_supply<T: Storage>(store: &T) -> u64 {
    let data = store
        .get(KEY_TOTAL_SUPPLY)
        .expect("no total_supply data stored");
    let value = u64::from_be_bytes(data[0..8].try_into().unwrap());
    return value;
}

fn get_balance<T: Storage>(store: &T, address: &str) -> u64 {
    let raw_address = parse_20bytes_from_hex(&address).unwrap();
    let data = store
        .get(&raw_address)
        .expect("no data stored for this address");
    let state: AddressState = from_slice(&data)
        .context(ParseErr {
            kind: "AddressState",
        })
        .unwrap();
    return state.balance;
}

#[test]
fn proper_initialization() {
    let mut instance = mock_instance(WASM);
    let msg = init_msg();
    let params = mock_params_height("creator", &[], &[], 876, 0);
    let res = init(&mut instance, params, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the store
    instance.with_storage(|store| {
        assert_eq!(get_name(store), "Ash token");
        assert_eq!(get_symbol(store), "ASH");
        assert_eq!(get_decimals(store), 5);
        assert_eq!(get_total_supply(store), 66);
        assert_eq!(
            get_balance(store, "0000000000000000000000000000000000000000"),
            11
        );
        assert_eq!(
            get_balance(store, "1111111111111111111111111111111111111111"),
            22
        );
        assert_eq!(
            get_balance(store, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            33
        );
    });
}
