use cosmwasm::errors::Error;
use cosmwasm::mock::{dependencies, mock_env, MockApi, MockStorage};
use cosmwasm::traits::Extern;
use cosmwasm::types::{Coin, HumanAddr};

use cw_storage::deserialize;

use crate::coin_helpers::{coin, coin_vec};
use crate::contract::{handle, init, query};
use crate::msg::{HandleMsg, InitMsg, QueryMsg, ResolveRecordResponse};
use crate::state::Config;

fn assert_name_owner(deps: &mut Extern<MockStorage, MockApi>, name: &str, owner: &str) {
    let res = query(
        &deps,
        QueryMsg::ResolveRecord {
            name: name.to_string(),
        },
    )
    .unwrap();

    let value: ResolveRecordResponse = deserialize(&res).unwrap();
    assert_eq!(HumanAddr::from(owner), value.address);
}

fn assert_config_state(deps: &mut Extern<MockStorage, MockApi>, expected: Config) {
    let res = query(&deps, QueryMsg::Config {}).unwrap();
    let value: Config = deserialize(&res).unwrap();
    assert_eq!(value, expected);
}

fn mock_init_with_fees(
    mut deps: &mut Extern<MockStorage, MockApi>,
    purchase_price: Coin,
    transfer_price: Coin,
) {
    let msg = InitMsg {
        name: "costly".to_string(),
        purchase_price: Some(purchase_price),
        transfer_price: Some(transfer_price),
    };

    let env = mock_env(&deps.api, "creator", &coin_vec("2", "token"), &[]);
    let _res = init(&mut deps, env, msg).expect("contract successfully handles InitMsg");
}

fn mock_init_no_fees(mut deps: &mut Extern<MockStorage, MockApi>) {
    let msg = InitMsg {
        name: "cheap".to_string(),
        purchase_price: None,
        transfer_price: None,
    };

    let env = mock_env(&deps.api, "creator", &coin_vec("2", "token"), &[]);
    let _res = init(&mut deps, env, msg).expect("contract successfully handles InitMsg");
}

fn mock_alice_registers_name(mut deps: &mut Extern<MockStorage, MockApi>, sent: &[Coin]) {
    // alice can register an available name
    let env = mock_env(&deps.api, "alice_key", sent, &[]);
    let msg = HandleMsg::Register {
        name: "alice".to_string(),
    };
    let _res = handle(&mut deps, env, msg).expect("contract successfully handles Register message");
}

#[test]
fn proper_init_no_fees() {
    let mut deps = dependencies(20);

    mock_init_no_fees(&mut deps);

    assert_config_state(
        &mut deps,
        Config {
            name: "cheap".to_string(),
            purchase_price: None,
            transfer_price: None,
        },
    );
}

#[test]
fn proper_init_with_fees() {
    let mut deps = dependencies(20);

    mock_init_with_fees(&mut deps, coin("2", "token"), coin("2", "token"));

    assert_config_state(
        &mut deps,
        Config {
            name: "costly".to_string(),
            purchase_price: Some(coin("2", "token")),
            transfer_price: Some(coin("2", "token")),
        },
    );
}

#[test]
fn register_available_name_and_query_works() {
    let mut deps = dependencies(20);
    mock_init_no_fees(&mut deps);
    mock_alice_registers_name(&mut deps, &[]);

    // querying for name resolves to correct address
    assert_name_owner(&mut deps, "alice", "alice_key");
}

#[test]
fn register_available_name_and_query_works_with_fees() {
    let mut deps = dependencies(20);
    mock_init_with_fees(&mut deps, coin("2", "token"), coin("2", "token"));
    mock_alice_registers_name(&mut deps, &coin_vec("2", "token"));

    // anyone can register an available name with more fees than needed
    let env = mock_env(&deps.api, "bob_key", &coin_vec("5", "token"), &[]);
    let msg = HandleMsg::Register {
        name: "bob".to_string(),
    };

    let _res = handle(&mut deps, env, msg).expect("contract successfully handles Register message");

    // querying for name resolves to correct address
    assert_name_owner(&mut deps, "alice", "alice_key");
    assert_name_owner(&mut deps, "bob", "bob_key");
}

#[test]
fn fails_on_register_already_taken_name() {
    let mut deps = dependencies(20);
    mock_init_no_fees(&mut deps);
    mock_alice_registers_name(&mut deps, &[]);

    // bob can't register the same name
    let env = mock_env(&deps.api, "bob_key", &coin_vec("2", "token"), &[]);
    let msg = HandleMsg::Register {
        name: "alice".to_string(),
    };
    let res = handle(&mut deps, env, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "Name is already taken"),
        Err(_) => panic!("Unknown error"),
    }
    // alice can't register the same name again
    let env = mock_env(&deps.api, "alice_key", &coin_vec("2", "token"), &[]);
    let msg = HandleMsg::Register {
        name: "alice".to_string(),
    };
    let res = handle(&mut deps, env, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "Name is already taken"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_on_register_insufficient_fees() {
    let mut deps = dependencies(20);
    mock_init_with_fees(&mut deps, coin("2", "token"), coin("2", "token"));

    // anyone can register an available name with sufficient fees
    let env = mock_env(&deps.api, "alice_key", &[], &[]);
    let msg = HandleMsg::Register {
        name: "alice".to_string(),
    };

    let res = handle(&mut deps, env, msg);

    match res {
        Ok(_) => panic!("register call should fail with insufficient fees"),
        Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "Insufficient funds sent"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_on_register_wrong_fee_denom() {
    let mut deps = dependencies(20);
    mock_init_with_fees(&mut deps, coin("2", "token"), coin("2", "token"));

    // anyone can register an available name with sufficient fees
    let env = mock_env(&deps.api, "alice_key", &coin_vec("2", "earth"), &[]);
    let msg = HandleMsg::Register {
        name: "alice".to_string(),
    };

    let res = handle(&mut deps, env, msg);

    match res {
        Ok(_) => panic!("register call should fail with insufficient fees"),
        Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "Insufficient funds sent"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn transfer_works() {
    let mut deps = dependencies(20);
    mock_init_no_fees(&mut deps);
    mock_alice_registers_name(&mut deps, &[]);

    // alice can transfer her name successfully to bob
    let env = mock_env(&deps.api, "alice_key", &[], &[]);
    let msg = HandleMsg::Transfer {
        name: "alice".to_string(),
        to: HumanAddr::from("bob_key"),
    };

    let _res = handle(&mut deps, env, msg).expect("contract successfully handles Transfer message");
    // querying for name resolves to correct address (bob_key)
    assert_name_owner(&mut deps, "alice", "bob_key");
}

#[test]
fn transfer_works_with_fees() {
    let mut deps = dependencies(20);
    mock_init_with_fees(&mut deps, coin("2", "token"), coin("2", "token"));
    mock_alice_registers_name(&mut deps, &coin_vec("2", "token"));

    // alice can transfer her name successfully to bob
    let env = mock_env(
        &deps.api,
        "alice_key",
        &vec![coin("1", "earth"), coin("2", "token")],
        &[],
    );
    let msg = HandleMsg::Transfer {
        name: "alice".to_string(),
        to: HumanAddr::from("bob_key"),
    };

    let _res = handle(&mut deps, env, msg).expect("contract successfully handles Transfer message");
    // querying for name resolves to correct address (bob_key)
    assert_name_owner(&mut deps, "alice", "bob_key");
}

#[test]
fn fails_on_transfer_from_nonowner() {
    let mut deps = dependencies(20);
    mock_init_no_fees(&mut deps);
    mock_alice_registers_name(&mut deps, &[]);

    // alice can transfer her name successfully to bob
    let env = mock_env(&deps.api, "frank_key", &coin_vec("2", "token"), &[]);
    let msg = HandleMsg::Transfer {
        name: "alice".to_string(),
        to: HumanAddr::from("bob_key"),
    };

    let res = handle(&mut deps, env, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(Error::Unauthorized { .. }) => {}
        Err(e) => panic!("Unexpected error: {:?}", e),
    }

    // querying for name resolves to correct address (alice_key)
    assert_name_owner(&mut deps, "alice", "alice_key");
}

#[test]
fn fails_on_transfer_insufficient_fees() {
    let mut deps = dependencies(20);
    mock_init_with_fees(&mut deps, coin("2", "token"), coin("5", "token"));
    mock_alice_registers_name(&mut deps, &coin_vec("2", "token"));

    // alice can transfer her name successfully to bob
    let env = mock_env(
        &deps.api,
        "alice_key",
        &vec![coin("1", "earth"), coin("2", "token")],
        &[],
    );
    let msg = HandleMsg::Transfer {
        name: "alice".to_string(),
        to: HumanAddr::from("bob_key"),
    };

    let res = handle(&mut deps, env, msg);

    match res {
        Ok(_) => panic!("register call should fail with insufficient fees"),
        Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "Insufficient funds sent"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }

    // querying for name resolves to correct address (bob_key)
    assert_name_owner(&mut deps, "alice", "alice_key");
}

#[test]
fn fails_on_query_unregistered_name() {
    let mut deps = dependencies(20);

    mock_init_no_fees(&mut deps);

    // querying for unregistered name results in NotFound error
    let res = query(
        &deps,
        QueryMsg::ResolveRecord {
            name: "alice".to_string(),
        },
    );

    match res {
        Ok(_) => panic!("Must return error"),
        Err(Error::NotFound { kind, .. }) => assert_eq!(kind, "cw_nameservice::state::NameRecord"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}
