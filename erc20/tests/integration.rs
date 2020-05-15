//! This integration test tries to run and call the generated wasm.
//! It depends on a Wasm build being available, which you can create with `cargo wasm`.
//! Then running `cargo integration-test` will validate we can properly call into that generated Wasm.
//!
//! You can easily convert unit tests to integration tests.
//! 1. First copy them over verbatum,
//! 2. Then change
//!      let mut deps = mock_dependencies(20);
//!    to
//!      let mut deps = mock_instance(WASM);
//! 3. If you access raw storage, where ever you see something like:
//!      deps.storage.get(CONFIG_KEY).expect("no data stored");
//!    replace it with:
//!      deps.with_storage(|store| {
//!          let data = store.get(CONFIG_KEY).expect("no data stored");
//!          //...
//!      });
//! 4. Anywhere you see query(&deps, ...) you must replace it with query(&mut deps, ...)
//! 5. When matching on error codes, you can not use Error types, but rather must use strings:
//!      match res {
//!          Err(Error::Unauthorized{..}) => {},
//!          _ => panic!("Must return unauthorized error"),
//!      }
//!    becomes:
//!      match res {
//!         ContractResult::Err(msg) => assert_eq!(msg, "Unauthorized"),
//!         _ => panic!("Expected error"),
//!      }

use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{
    from_slice, log, Api, Env, HandleResponse, HumanAddr, InitResponse, ReadonlyStorage, Storage,
};
use cosmwasm_storage::ReadonlyPrefixedStorage;
use cosmwasm_vm::testing::{handle, init, mock_instance, query};

use cw_erc20::contract::{
    bytes_to_u128, read_u128, Constants, KEY_CONSTANTS, KEY_TOTAL_SUPPLY, PREFIX_ALLOWANCES,
    PREFIX_BALANCES, PREFIX_CONFIG,
};
use cw_erc20::msg::{HandleMsg, InitMsg, InitialBalance, QueryMsg};

static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/cw_erc20.wasm");

fn mock_env_height<A: Api>(api: &A, signer: &HumanAddr, height: u64, time: u64) -> Env {
    let mut env = mock_env(api, signer, &[]);
    env.block.height = height;
    env.block.time = time;
    env
}

fn get_constants<S: Storage>(storage: &S) -> Constants {
    let config_storage = ReadonlyPrefixedStorage::new(PREFIX_CONFIG, storage);
    let data = config_storage
        .get(KEY_CONSTANTS)
        .expect("error getting data")
        .expect("no config data stored");
    from_slice(&data).expect("invalid data")
}

fn get_total_supply<S: Storage>(storage: &S) -> u128 {
    let config_storage = ReadonlyPrefixedStorage::new(PREFIX_CONFIG, storage);
    let data = config_storage
        .get(KEY_TOTAL_SUPPLY)
        .expect("error getting data")
        .expect("no decimals data stored");
    return bytes_to_u128(&data).unwrap();
}

fn get_balance<S: ReadonlyStorage, A: Api>(api: &A, storage: &S, address: &HumanAddr) -> u128 {
    let address_key = api
        .canonical_address(address)
        .expect("canonical_address failed");
    let balances_storage = ReadonlyPrefixedStorage::new(PREFIX_BALANCES, storage);
    return read_u128(&balances_storage, address_key.as_slice()).unwrap();
}

fn get_allowance<S: ReadonlyStorage, A: Api>(
    api: &A,
    storage: &S,
    owner: &HumanAddr,
    spender: &HumanAddr,
) -> u128 {
    let owner_raw_address = api
        .canonical_address(owner)
        .expect("canonical_address failed");
    let spender_raw_address = api
        .canonical_address(spender)
        .expect("canonical_address failed");
    let allowances_storage = ReadonlyPrefixedStorage::new(PREFIX_ALLOWANCES, storage);
    let owner_storage =
        ReadonlyPrefixedStorage::new(owner_raw_address.as_slice(), &allowances_storage);
    return read_u128(&owner_storage, spender_raw_address.as_slice()).unwrap();
}

fn address(index: u8) -> HumanAddr {
    match index {
        0 => HumanAddr("addr0000".to_string()), // contract initializer
        1 => HumanAddr("addr1111".to_string()),
        2 => HumanAddr("addr4321".to_string()),
        3 => HumanAddr("addr5432".to_string()),
        _ => panic!("Unsupported address index"),
    }
}

fn init_msg() -> InitMsg {
    InitMsg {
        decimals: 5,
        name: "Ash token".to_string(),
        symbol: "ASH".to_string(),
        initial_balances: [
            InitialBalance {
                address: address(1),
                amount: "11".to_string(),
            },
            InitialBalance {
                address: address(2),
                amount: "22".to_string(),
            },
            InitialBalance {
                address: address(3),
                amount: "33".to_string(),
            },
        ]
        .to_vec(),
    }
}

#[test]
fn init_works() {
    let mut deps = mock_instance(WASM, &[]);
    let init_msg = init_msg();
    let params = mock_env_height(&deps.api, &address(0), 876, 0);
    let res: InitResponse = init(&mut deps, params, init_msg).unwrap();
    assert_eq!(0, res.messages.len());

    // query the store directly
    let api = deps.api;
    deps.with_storage(|storage| {
        assert_eq!(
            get_constants(storage),
            Constants {
                name: "Ash token".to_string(),
                symbol: "ASH".to_string(),
                decimals: 5
            }
        );
        assert_eq!(get_total_supply(storage), 66);
        assert_eq!(get_balance(&api, storage, &address(1)), 11);
        assert_eq!(get_balance(&api, storage, &address(2)), 22);
        assert_eq!(get_balance(&api, storage, &address(3)), 33);
        Ok(())
    })
    .unwrap();
}

#[test]
fn transfer_works() {
    let mut deps = mock_instance(WASM, &[]);
    let init_msg = init_msg();
    let env1 = mock_env_height(&deps.api, &address(0), 876, 0);
    let res: InitResponse = init(&mut deps, env1, init_msg).unwrap();
    assert_eq!(0, res.messages.len());

    let sender = address(1);
    let recipient = address(2);

    // Before
    let api = deps.api;
    deps.with_storage(|storage| {
        assert_eq!(get_balance(&api, storage, &sender), 11);
        assert_eq!(get_balance(&api, storage, &recipient), 22);
        Ok(())
    })
    .unwrap();

    // Transfer
    let transfer_msg = HandleMsg::Transfer {
        recipient: recipient.clone(),
        amount: "1".to_string(),
    };
    let env2 = mock_env_height(&deps.api, &sender, 877, 0);
    let transfer_response: HandleResponse = handle(&mut deps, env2, transfer_msg).unwrap();
    assert_eq!(transfer_response.messages.len(), 0);
    assert_eq!(
        transfer_response.log,
        vec![
            log("action", "transfer"),
            log("sender", sender.as_str()),
            log("recipient", recipient.as_str()),
        ]
    );

    // After
    deps.with_storage(|storage| {
        assert_eq!(get_balance(&api, storage, &sender), 10);
        assert_eq!(get_balance(&api, storage, &recipient), 23);
        Ok(())
    })
    .unwrap();
}

#[test]
fn approve_works() {
    let mut deps = mock_instance(WASM, &[]);
    let init_msg = init_msg();
    let env1 = mock_env_height(&deps.api, &address(0), 876, 0);
    let res: InitResponse = init(&mut deps, env1, init_msg).unwrap();
    assert_eq!(0, res.messages.len());

    let owner = address(1);
    let spender = address(2);

    // Before
    let api = deps.api;
    deps.with_storage(|storage| {
        assert_eq!(get_allowance(&api, storage, &owner, &spender), 0);
        Ok(())
    })
    .unwrap();

    // Approve
    let approve_msg = HandleMsg::Approve {
        spender: spender.clone(),
        amount: "42".to_string(),
    };
    let env2 = mock_env_height(&deps.api, &owner, 877, 0);
    let approve_response: HandleResponse = handle(&mut deps, env2, approve_msg).unwrap();
    assert_eq!(approve_response.messages.len(), 0);
    assert_eq!(
        approve_response.log,
        vec![
            log("action", "approve"),
            log("owner", owner.as_str()),
            log("spender", spender.as_str()),
        ]
    );

    // After
    deps.with_storage(|storage| {
        assert_eq!(get_allowance(&api, storage, &owner, &spender), 42);
        Ok(())
    })
    .unwrap();
}

#[test]
fn transfer_from_works() {
    let mut deps = mock_instance(WASM, &[]);
    let init_msg = init_msg();
    let env1 = mock_env_height(&deps.api, &address(0), 876, 0);
    let res: InitResponse = init(&mut deps, env1, init_msg).unwrap();
    assert_eq!(0, res.messages.len());

    let owner = address(1);
    let spender = address(2);
    let recipient = address(3);

    // Before
    let api = deps.api;
    deps.with_storage(|storage| {
        assert_eq!(get_balance(&api, storage, &owner), 11);
        assert_eq!(get_balance(&api, storage, &recipient), 33);
        assert_eq!(get_allowance(&api, storage, &owner, &spender), 0);
        Ok(())
    })
    .unwrap();

    // Approve
    let approve_msg = HandleMsg::Approve {
        spender: spender.clone(),
        amount: "42".to_string(),
    };
    let env2 = mock_env_height(&deps.api, &owner, 877, 0);
    let approve_response: HandleResponse = handle(&mut deps, env2, approve_msg).unwrap();
    assert_eq!(approve_response.messages.len(), 0);
    assert_eq!(
        approve_response.log,
        vec![
            log("action", "approve"),
            log("owner", owner.as_str()),
            log("spender", spender.as_str()),
        ]
    );

    // Transfer from
    let transfer_from_msg = HandleMsg::TransferFrom {
        owner: owner.clone(),
        recipient: recipient.clone(),
        amount: "2".to_string(),
    };
    let env3 = mock_env_height(&deps.api, &spender, 878, 0);
    let transfer_from_response: HandleResponse =
        handle(&mut deps, env3, transfer_from_msg).unwrap();
    assert_eq!(transfer_from_response.messages.len(), 0);
    assert_eq!(
        transfer_from_response.log,
        vec![
            log("action", "transfer_from"),
            log("spender", spender.as_str()),
            log("sender", owner.as_str()),
            log("recipient", recipient.as_str()),
        ]
    );

    // After
    deps.with_storage(|storage| {
        assert_eq!(get_balance(&api, storage, &owner), 9);
        assert_eq!(get_balance(&api, storage, &recipient), 35);
        assert_eq!(get_allowance(&api, storage, &owner, &spender), 40);
        Ok(())
    })
    .unwrap();
}

#[test]
fn burn_works() {
    let mut deps = mock_instance(WASM, &[]);
    let init_msg = init_msg();
    let env1 = mock_env_height(&deps.api, &address(0), 876, 0);
    let res: InitResponse = init(&mut deps, env1, init_msg).unwrap();
    assert_eq!(0, res.messages.len());

    let owner = address(1);

    // Before
    let api = deps.api;
    deps.with_storage(|storage| {
        assert_eq!(get_balance(&api, storage, &owner), 11);
        Ok(())
    })
    .unwrap();

    // Burn
    let burn_msg = HandleMsg::Burn {
        amount: "1".to_string(),
    };
    let env2 = mock_env_height(&deps.api, &owner, 877, 0);
    let burn_response: HandleResponse = handle(&mut deps, env2, burn_msg).unwrap();
    assert_eq!(burn_response.messages.len(), 0);
    assert_eq!(
        burn_response.log,
        vec![
            log("action", "burn"),
            log("account", owner.as_str()),
            log("amount", "1")
        ]
    );

    // After
    deps.with_storage(|storage| {
        assert_eq!(get_balance(&api, storage, &owner), 10);
        Ok(())
    })
    .unwrap();
}

#[test]
fn can_query_balance_of_existing_address() {
    let mut deps = mock_instance(WASM, &[]);
    let init_msg = init_msg();
    let env1 = mock_env_height(&deps.api, &address(0), 450, 550);
    let res: InitResponse = init(&mut deps, env1, init_msg).unwrap();
    assert_eq!(0, res.messages.len());

    let query_msg = QueryMsg::Balance {
        address: address(2),
    };
    let query_result = query(&mut deps, query_msg).unwrap();
    assert_eq!(query_result.as_slice(), b"{\"balance\":\"22\"}");
}
