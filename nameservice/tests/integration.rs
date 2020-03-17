use cosmwasm::mock::{mock_env, MockApi, MockStorage};
use cosmwasm::types::{Coin, ContractResult, HumanAddr};

use cosmwasm_vm::testing::{handle, init, mock_instance, query};
use cosmwasm_vm::Instance;

use cw_storage::deserialize;

use cw_nameservice::coin_helpers::{coin, coin_vec};
use cw_nameservice::msg::{HandleMsg, InitMsg, QueryMsg, ResolveRecordResponse};
use cw_nameservice::state::{config, Config};

/**
This integration test tries to run and call the generated wasm.
It depends on a release build being available already. You can create that with:

cargo wasm && wasm-gc ./target/wasm32-unknown-unknown/release/hackatom.wasm

Then running `cargo test` will validate we can properly call into that generated data.

You can easily convert unit tests to integration tests.
1. First copy them over verbatum,
2. Then change
    let mut deps = mock_instance(WASM);
To
    let mut deps = mock_instance(WASM);
3. If you access raw storage, where ever you see something like:
    deps.storage.get(CONFIG_KEY).expect("no data stored");
 replace it with:
    deps.with_storage(|store| {
        let data = store.get(CONFIG_KEY).expect("no data stored");
        //...
    });
4. Anywhere you see query(&deps, ...) you must replace it with query(&mut deps, ...)
5. When matching on error codes, you can not use Error types, but rather must use strings:
     match res {
         Err(Error::Unauthorized{..}) => {},
         _ => panic!("Must return unauthorized error"),
     }
     becomes:
     match res {
        ContractResult::Err(msg) => assert_eq!(msg, "Unauthorized"),
        _ => panic!("Expected error"),
     }

**/

// This line will test the output of cargo wasm
static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/cw_nameservice.wasm");
// You can uncomment this line instead to test productionified build from cosmwasm-opt
// static WASM: &[u8] = include_bytes!("../contract.wasm");

fn assert_name_owner(mut deps: &mut Instance<MockStorage, MockApi>, name: &str, owner: &str) {
    let res = query(
        &mut deps,
        QueryMsg::ResolveRecord {
            name: name.to_string(),
        },
    )
    .unwrap();

    let value: ResolveRecordResponse = deserialize(res.as_slice()).unwrap();
    assert_eq!(HumanAddr::from(owner), value.address);
}

fn mock_init_with_fees(
    mut deps: &mut Instance<MockStorage, MockApi>,
    purchase_price: Coin,
    transfer_price: Coin,
) {
    let msg = InitMsg {
        purchase_price: Some(purchase_price),
        transfer_price: Some(transfer_price),
    };

    let params = mock_env(&deps.api, "creator", &coin_vec("2", "token"), &[]);
    // unwrap: contract successfully handles InitMsg
    let _res = init(&mut deps, params, msg).unwrap();
}

fn mock_init_no_fees(mut deps: &mut Instance<MockStorage, MockApi>) {
    let msg = InitMsg {
        purchase_price: None,
        transfer_price: None,
    };

    let params = mock_env(&deps.api, "creator", &coin_vec("2", "token"), &[]);
    // unwrap: contract successfully handles InitMsg
    let _res = init(&mut deps, params, msg).unwrap();
}

fn mock_alice_registers_name(mut deps: &mut Instance<MockStorage, MockApi>, sent: &[Coin]) {
    // alice can register an available name
    let params = mock_env(&deps.api, "alice_key", sent, &[]);
    let msg = HandleMsg::Register {
        name: "alice".to_string(),
    };
    // unwrap: contract successfully handles Register message
    let _res = handle(&mut deps, params, msg).unwrap();
}

#[test]
fn proper_init_no_fees() {
    let mut deps = mock_instance(WASM);

    mock_init_no_fees(&mut deps);

    deps.with_storage(|storage| {
        let config_state = config(storage)
            .load()
            .expect("can load config from storage");

        assert_eq!(
            config_state,
            Config {
                purchase_price: None,
                transfer_price: None
            }
        );
    });
}

#[test]
fn proper_init_with_fees() {
    let mut deps = mock_instance(WASM);

    mock_init_with_fees(&mut deps, coin("2", "token"), coin("2", "token"));

    deps.with_storage(|storage| {
        let config_state = config(storage)
            .load()
            .expect("can load config from storage");

        assert_eq!(
            config_state,
            Config {
                purchase_price: Some(coin("2", "token")),
                transfer_price: Some(coin("2", "token")),
            }
        );
    });
}

#[test]
fn register_available_name_and_query_works_with_fees() {
    let mut deps = mock_instance(WASM);
    mock_init_with_fees(&mut deps, coin("2", "token"), coin("2", "token"));
    mock_alice_registers_name(&mut deps, &coin_vec("2", "token"));

    // anyone can register an available name with more fees than needed
    let params = mock_env(&deps.api, "bob_key", &coin_vec("5", "token"), &[]);
    let msg = HandleMsg::Register {
        name: "bob".to_string(),
    };

    // unwrap: contract successfully handles Register message
    let _res = handle(&mut deps, params, msg).unwrap();

    // querying for name resolves to correct address
    assert_name_owner(&mut deps, "alice", "alice_key");
    assert_name_owner(&mut deps, "bob", "bob_key");
}

#[test]
fn register_available_name_and_query_works() {
    let mut deps = mock_instance(WASM);
    mock_init_no_fees(&mut deps);
    mock_alice_registers_name(&mut deps, &[]);

    // querying for name resolves to correct address
    assert_name_owner(&mut deps, "alice", "alice_key");
}

#[test]
fn fails_on_register_already_taken_name() {
    let mut deps = mock_instance(WASM);
    mock_init_no_fees(&mut deps);
    mock_alice_registers_name(&mut deps, &[]);

    // bob can't register the same name
    let params = mock_env(&deps.api, "bob_key", &coin_vec("2", "token"), &[]);
    let msg = HandleMsg::Register {
        name: "alice".to_string(),
    };
    let res = handle(&mut deps, params, msg);

    match res {
        ContractResult::Ok(_) => panic!("Must return error"),
        ContractResult::Err(e) => assert_eq!(e, "Contract error: Name is already taken"),
    }
    // alice can't register the same name again
    let params = mock_env(&deps.api, "alice_key", &coin_vec("2", "token"), &[]);
    let msg = HandleMsg::Register {
        name: "alice".to_string(),
    };
    let res = handle(&mut deps, params, msg);

    match res {
        ContractResult::Ok(_) => panic!("Must return error"),
        ContractResult::Err(e) => assert_eq!(e, "Contract error: Name is already taken"),
    }
}

#[test]
fn fails_on_register_insufficient_fees() {
    let mut deps = mock_instance(WASM);
    mock_init_with_fees(&mut deps, coin("2", "token"), coin("2", "token"));

    // anyone can register an available name with sufficient fees
    let params = mock_env(&deps.api, "alice_key", &[], &[]);
    let msg = HandleMsg::Register {
        name: "alice".to_string(),
    };

    let res = handle(&mut deps, params, msg);

    match res {
        ContractResult::Ok(_) => panic!("Must return error"),
        ContractResult::Err(e) => assert_eq!(e, "Contract error: Insufficient funds sent"),
    }
}

#[test]
fn fails_on_register_wrong_fee_denom() {
    let mut deps = mock_instance(WASM);
    mock_init_with_fees(&mut deps, coin("2", "token"), coin("2", "token"));

    // anyone can register an available name with sufficient fees
    let params = mock_env(&deps.api, "alice_key", &coin_vec("2", "earth"), &[]);
    let msg = HandleMsg::Register {
        name: "alice".to_string(),
    };

    let res = handle(&mut deps, params, msg);

    match res {
        ContractResult::Ok(_) => panic!("Must return error"),
        ContractResult::Err(e) => assert_eq!(e, "Contract error: Insufficient funds sent"),
    }
}

#[test]
fn transfer_works() {
    let mut deps = mock_instance(WASM);
    mock_init_no_fees(&mut deps);
    mock_alice_registers_name(&mut deps, &[]);

    // alice can transfer her name successfully to bob
    let params = mock_env(&deps.api, "alice_key", &[], &[]);
    let msg = HandleMsg::Transfer {
        name: "alice".to_string(),
        to: HumanAddr::from("bob_key"),
    };

    let _res = handle(&mut deps, params, msg).unwrap();
    // querying for name resolves to correct address (bob_key)
    assert_name_owner(&mut deps, "alice", "bob_key");
}

#[test]
fn transfer_works_with_fees() {
    let mut deps = mock_instance(WASM);
    mock_init_with_fees(&mut deps, coin("2", "token"), coin("2", "token"));
    mock_alice_registers_name(&mut deps, &coin_vec("2", "token"));

    // alice can transfer her name successfully to bob
    let params = mock_env(
        &deps.api,
        "alice_key",
        &vec![coin("1", "earth"), coin("2", "token")],
        &[],
    );
    let msg = HandleMsg::Transfer {
        name: "alice".to_string(),
        to: HumanAddr::from("bob_key"),
    };

    let _res = handle(&mut deps, params, msg).unwrap();
    // querying for name resolves to correct address (bob_key)
    assert_name_owner(&mut deps, "alice", "bob_key");
}

#[test]
fn fails_on_transfer_from_nonowner() {
    let mut deps = mock_instance(WASM);
    mock_init_no_fees(&mut deps);
    mock_alice_registers_name(&mut deps, &[]);

    // alice can transfer her name successfully to bob
    let params = mock_env(&deps.api, "frank_key", &coin_vec("2", "token"), &[]);
    let msg = HandleMsg::Transfer {
        name: "alice".to_string(),
        to: HumanAddr::from("bob_key"),
    };

    let res = handle(&mut deps, params, msg);

    match res {
        ContractResult::Ok(_) => panic!("Must return error"),
        ContractResult::Err(e) => assert_eq!(e, "Unauthorized"),
    }

    // querying for name resolves to correct address (alice_key)
    assert_name_owner(&mut deps, "alice", "alice_key");
}

#[test]
fn fails_on_transfer_insufficient_fees() {
    let mut deps = mock_instance(WASM);
    mock_init_with_fees(&mut deps, coin("2", "token"), coin("5", "token"));
    mock_alice_registers_name(&mut deps, &coin_vec("2", "token"));

    // alice can transfer her name successfully to bob
    let params = mock_env(
        &deps.api,
        "alice_key",
        &vec![coin("1", "earth"), coin("2", "token")],
        &[],
    );
    let msg = HandleMsg::Transfer {
        name: "alice".to_string(),
        to: HumanAddr::from("bob_key"),
    };

    let res = handle(&mut deps, params, msg);

    match res {
        ContractResult::Ok(_) => panic!("Must return error"),
        ContractResult::Err(e) => assert_eq!(e, "Contract error: Insufficient funds sent"),
    }

    // querying for name resolves to correct address (bob_key)
    assert_name_owner(&mut deps, "alice", "alice_key");
}

#[test]
fn returns_empty_on_query_unregistered_name() {
    let mut deps = mock_instance(WASM);

    mock_init_no_fees(&mut deps);

    // querying for unregistered name results in NotFound error
    let res = query(
        &mut deps,
        QueryMsg::ResolveRecord {
            name: "alice".to_string(),
        },
    )
    .unwrap();
    let value: ResolveRecordResponse = deserialize(res.as_slice()).unwrap();
    assert_eq!(value.address.as_str(), "");
}
