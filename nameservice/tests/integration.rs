use cosmwasm::mock::{mock_params, MockApi, MockStorage};
use cosmwasm::serde::from_slice;
use cosmwasm::types::{HumanAddr, coin, ContractResult, QueryResult};

use cosmwasm_vm::testing::{handle, init, mock_instance, query};
use cosmwasm_vm::Instance;

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

fn mock_init_and_alice_registers_name(mut deps: &mut Instance<MockStorage, MockApi>) {
    let msg = InitMsg {
        name: "Cool Name Service".to_string(),
    };
    let params = mock_params(&deps.api, "creator", &coin("2", "token"), &[]);
    let _res = init(&mut deps, params, msg).unwrap();

    // anyone can register an available name
    let params = mock_params(&deps.api, "alice_key", &coin("2", "token"), &[]);
    let msg = HandleMsg::Register {
        name: "alice".to_string(),
    };
    let _res = handle(&mut deps, params, msg).unwrap();
}

#[test]
fn proper_initialization() {
    let mut deps = mock_instance(WASM);

    let msg = InitMsg {
        name: "Cool Name Service".to_string(),
    };
    let params = mock_params(&deps.api, "creator", &coin("1000", "earth"), &[]);

    // we can just call .unwrap() to assert this was a success
    let res = init(&mut deps, params, msg).unwrap();
    assert_eq!(0, res.messages.len());

    deps.with_storage(|storage| {
        // assert the name was set correctly
        let config_state = config(storage)
            .load()
            .expect("Config loads successfully from storage");

        assert_eq!(
            config_state,
            Config {
                name: "Cool Name Service".to_string()
            }
        );
    });
}

#[test]
fn register_available_name_and_query_works() {
    let mut deps = mock_instance(WASM);
    mock_init_and_alice_registers_name(&mut deps);

    // querying for name resolves to correct address
    let res = query(
        &mut deps,
        QueryMsg::ResolveRecord {
            name: "alice".to_string(),
        },
    )
    .unwrap();

    let value: ResolveRecordResponse = from_slice(&res).unwrap();
    assert_eq!(HumanAddr::from("alice_key"), value.address);
}

#[test]
fn fails_on_register_already_taken_name() {
    let mut deps = mock_instance(WASM);
    mock_init_and_alice_registers_name(&mut deps);

    // bob can't register the same name
    let params = mock_params(&deps.api, "bob_key", &coin("2", "token"), &[]);
    let msg = HandleMsg::Register {
        name: "alice".to_string(),
    };
    let res = handle(&mut deps, params, msg);

    match res {
        ContractResult::Ok(_) => panic!("Must return error"),
        ContractResult::Err(e) => assert_eq!(e, "Contract error: Name is already taken"),
    }
    // alice can't register the same name again
    let params = mock_params(&deps.api, "alice_key", &coin("2", "token"), &[]);
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
fn transfer_works() {
    let mut deps = mock_instance(WASM);
    mock_init_and_alice_registers_name(&mut deps);

    // alice can transfer her name successfully to bob
    let params = mock_params(&deps.api, "alice_key", &coin("2", "token"), &[]);
    let msg = HandleMsg::Transfer {
        name: "alice".to_string(),
        to: HumanAddr::from("bob_key"),
    };

    let _res =
        handle(&mut deps, params, msg).unwrap();
    // querying for name resolves to correct address (bob_key)
    let res = query(
        &mut deps,
        QueryMsg::ResolveRecord {
            name: "alice".to_string(),
        },
    )
    .unwrap();

    let value: ResolveRecordResponse = from_slice(&res).unwrap();
    assert_eq!(HumanAddr::from("bob_key"), value.address);
}

#[test]
fn fails_on_transfer_from_nonowner() {
    let mut deps = mock_instance(WASM);
    mock_init_and_alice_registers_name(&mut deps);

    // alice can transfer her name successfully to bob
    let params = mock_params(&deps.api, "frank_key", &coin("2", "token"), &[]);
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
    let res = query(
        &mut deps,
        QueryMsg::ResolveRecord {
            name: "alice".to_string(),
        },
    )
    .unwrap();

    let value: ResolveRecordResponse = from_slice(&res).unwrap();
    assert_eq!(HumanAddr::from("alice_key"), value.address);
}

#[test]
fn fails_on_query_unregistered_name() {
    let mut deps = mock_instance(WASM);

    let msg = InitMsg {
        name: "Cool Name Service".to_string(),
    };
    let params = mock_params(&deps.api, "creator", &coin("2", "token"), &[]);
    let _res = init(&mut deps, params, msg).unwrap();

    // querying for unregistered name results in NotFound error
    let res = query(
        &mut deps,
        QueryMsg::ResolveRecord {
            name: "alice".to_string(),
        },
    );

    match res {
        QueryResult::Ok(_) => panic!("Must return error"),
        QueryResult::Err(e) => assert_eq!(e, "NameRecord not found"),
    }
}
