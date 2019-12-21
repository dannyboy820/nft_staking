use cosmwasm::mock::mock_params;
use cosmwasm::serde::to_vec;
use cosmwasm::traits::{Api, Storage};
use cosmwasm::types::Params;
use cosmwasm_vm::testing::{init, mock_instance};
use std::convert::TryInto;

use erc20::contract::{
    bytes_to_u128, read_u128, InitMsg, InitialBalance, KEY_DECIMALS, KEY_NAME, KEY_SYMBOL,
    KEY_TOTAL_SUPPLY, PREFIX_BALANCES, PREFIX_CONFIG,
};

static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/erc20.wasm");

fn init_msg() -> Vec<u8> {
    to_vec(&InitMsg {
        decimals: 5,
        name: "Ash token".to_string(),
        symbol: "ASH".to_string(),
        initial_balances: [
            InitialBalance {
                address: "addr0000".to_string(),
                amount: "11".to_string(),
            },
            InitialBalance {
                address: "addr1111".to_string(),
                amount: "22".to_string(),
            },
            InitialBalance {
                address: "addr4321".to_string(),
                amount: "33".to_string(),
            },
        ]
        .to_vec(),
    })
    .unwrap()
}

fn mock_params_height<A: Api>(api: &A, signer: &str, height: i64, time: i64) -> Params {
    let mut params = mock_params(api, signer, &[], &[]);
    params.block.height = height;
    params.block.time = time;
    params
}

fn get_name<S: Storage>(store: &S) -> String {
    let key = [
        &[PREFIX_CONFIG.len() as u8] as &[u8],
        PREFIX_CONFIG,
        KEY_NAME,
    ]
    .concat();
    let data = store.get(&key).expect("no name data stored");
    return String::from_utf8(data).unwrap();
}

fn get_symbol<S: Storage>(store: &S) -> String {
    let key = [
        &[PREFIX_CONFIG.len() as u8] as &[u8],
        PREFIX_CONFIG,
        KEY_SYMBOL,
    ]
    .concat();
    let data = store.get(&key).expect("no symbol data stored");
    return String::from_utf8(data).unwrap();
}

fn get_decimals<S: Storage>(store: &S) -> u8 {
    let key = [
        &[PREFIX_CONFIG.len() as u8] as &[u8],
        PREFIX_CONFIG,
        KEY_DECIMALS,
    ]
    .concat();
    let data = store.get(&key).expect("no decimals data stored");
    return u8::from_be_bytes(data[0..1].try_into().unwrap());
}

fn get_total_supply<S: Storage>(store: &S) -> u128 {
    let key = [
        &[PREFIX_CONFIG.len() as u8] as &[u8],
        PREFIX_CONFIG,
        KEY_TOTAL_SUPPLY,
    ]
    .concat();
    let data = store.get(&key).expect("no decimals data stored");
    return bytes_to_u128(&data).unwrap();
}

fn get_balance<S: Storage, A: Api>(api: &A, storage: &S, address: &str) -> u128 {
    let address_key = api
        .canonical_address(address)
        .expect("canonical_address failed");
    let key = [
        &[PREFIX_BALANCES.len() as u8] as &[u8],
        PREFIX_BALANCES,
        &address_key[..],
    ]
    .concat();
    return read_u128(storage, &key).unwrap();
}

#[test]
fn proper_initialization() {
    let mut deps = mock_instance(WASM);
    let msg = init_msg();
    let params = mock_params_height(&deps.api, "creator", 876, 0);
    let res = init(&mut deps, params, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // query the store directly
    deps.with_storage(|storage| {
        assert_eq!(get_name(storage), "Ash token");
        assert_eq!(get_symbol(storage), "ASH");
        assert_eq!(get_decimals(storage), 5);
        assert_eq!(get_total_supply(storage), 66);
        assert_eq!(get_balance(&deps.api, storage, "addr0000"), 11);
        assert_eq!(get_balance(&deps.api, storage, "addr1111"), 22);
        assert_eq!(get_balance(&deps.api, storage, "addr4321"), 33);
    });
}
