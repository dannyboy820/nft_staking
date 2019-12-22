use cosmwasm::mock::mock_params;
use cosmwasm::serde::to_vec;
use cosmwasm::traits::{Api, ReadonlyStorage, Storage};
use cosmwasm::types::{HumanAddr, Params};
use cosmwasm_vm::testing::{handle, init, mock_instance};
use std::convert::TryInto;

use erc20::contract::{
    bytes_to_u128, prefixedstorage, read_u128, HandleMsg, InitMsg, InitialBalance, KEY_DECIMALS,
    KEY_NAME, KEY_SYMBOL, KEY_TOTAL_SUPPLY, PREFIX_BALANCES, PREFIX_CONFIG,
};
use prefixedstorage::ReadonlyPrefixedStorage;

static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/erc20.wasm");

fn init_msg() -> Vec<u8> {
    to_vec(&InitMsg {
        decimals: 5,
        name: "Ash token".to_string(),
        symbol: "ASH".to_string(),
        initial_balances: [
            InitialBalance {
                address: HumanAddr("addr0000".to_string()),
                amount: "11".to_string(),
            },
            InitialBalance {
                address: HumanAddr("addr1111".to_string()),
                amount: "22".to_string(),
            },
            InitialBalance {
                address: HumanAddr("addr4321".to_string()),
                amount: "33".to_string(),
            },
        ]
        .to_vec(),
    })
    .unwrap()
}

fn mock_params_height<A: Api>(api: &A, signer: &HumanAddr, height: i64, time: i64) -> Params {
    let mut params = mock_params(api, signer.as_str(), &[], &[]);
    params.block.height = height;
    params.block.time = time;
    params
}

fn get_name<S: Storage>(storage: &S) -> String {
    let config_storage = ReadonlyPrefixedStorage::new(storage, PREFIX_CONFIG);
    let data = config_storage.get(KEY_NAME).expect("no name data stored");
    return String::from_utf8(data).unwrap();
}

fn get_symbol<S: Storage>(storage: &S) -> String {
    let config_storage = ReadonlyPrefixedStorage::new(storage, PREFIX_CONFIG);
    let data = config_storage
        .get(KEY_SYMBOL)
        .expect("no symbol data stored");
    return String::from_utf8(data).unwrap();
}

fn get_decimals<S: Storage>(storage: &S) -> u8 {
    let config_storage = ReadonlyPrefixedStorage::new(storage, PREFIX_CONFIG);
    let data = config_storage
        .get(KEY_DECIMALS)
        .expect("no decimals data stored");
    return u8::from_be_bytes(data[0..1].try_into().unwrap());
}

fn get_total_supply<S: Storage>(storage: &S) -> u128 {
    let config_storage = ReadonlyPrefixedStorage::new(storage, PREFIX_CONFIG);
    let data = config_storage
        .get(KEY_TOTAL_SUPPLY)
        .expect("no decimals data stored");
    return bytes_to_u128(&data).unwrap();
}

fn get_balance<S: ReadonlyStorage, A: Api>(api: &A, storage: &S, address: &HumanAddr) -> u128 {
    let address_key = api
        .canonical_address(address)
        .expect("canonical_address failed");
    let balances_storage = ReadonlyPrefixedStorage::new(storage, PREFIX_BALANCES);
    return read_u128(&balances_storage, address_key.as_bytes()).unwrap();
}

#[test]
fn init_works() {
    let mut deps = mock_instance(WASM);
    let msg = init_msg();
    let params = mock_params_height(&deps.api, &HumanAddr("creator".to_string()), 876, 0);
    let res = init(&mut deps, params, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // query the store directly
    deps.with_storage(|storage| {
        assert_eq!(get_name(storage), "Ash token");
        assert_eq!(get_symbol(storage), "ASH");
        assert_eq!(get_decimals(storage), 5);
        assert_eq!(get_total_supply(storage), 66);
        assert_eq!(
            get_balance(&deps.api, storage, &HumanAddr("addr0000".to_string())),
            11
        );
        assert_eq!(
            get_balance(&deps.api, storage, &HumanAddr("addr1111".to_string())),
            22
        );
        assert_eq!(
            get_balance(&deps.api, storage, &HumanAddr("addr4321".to_string())),
            33
        );
    });
}

#[test]
fn transfer_works() {
    let mut deps = mock_instance(WASM);
    let msg = init_msg();
    let params1 = mock_params_height(&deps.api, &HumanAddr("creator".to_string()), 876, 0);
    let res = init(&mut deps, params1, msg).unwrap();
    assert_eq!(0, res.messages.len());

    let sender = HumanAddr("addr0000".to_string());
    let recipient = HumanAddr("addr1111".to_string());

    // Before
    deps.with_storage(|storage| {
        assert_eq!(get_balance(&deps.api, storage, &sender), 11);
        assert_eq!(get_balance(&deps.api, storage, &recipient), 22);
    });

    // Transfer
    let transfer_msg = to_vec(&HandleMsg::Transfer {
        recipient: recipient.clone(),
        amount: "1".to_string(),
    })
    .unwrap();
    let params2 = mock_params_height(&deps.api, &sender, 877, 0);
    let transfer_result = handle(&mut deps, params2, transfer_msg).unwrap();
    assert_eq!(transfer_result.messages.len(), 0);
    assert_eq!(transfer_result.log, Some("transfer successful".to_string()));

    // After
    deps.with_storage(|storage| {
        assert_eq!(get_balance(&deps.api, storage, &sender), 10);
        assert_eq!(get_balance(&deps.api, storage, &recipient), 23);
    });
}
