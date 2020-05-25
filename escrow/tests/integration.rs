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
    coins, Api, BankMsg, Coin, CosmosMsg, Env, HandleResponse, HandleResult, HumanAddr,
    InitResponse, InitResult, StdError,
};

use cosmwasm_vm::testing::{handle, init, mock_instance};

use cw_escrow::contract::{config, State};
use cw_escrow::msg::{HandleMsg, InitMsg};

// This line will test the output of cargo wasm
static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/cw_escrow.wasm");
// You can uncomment this line instead to test productionified build from cosmwasm-opt
// static WASM: &[u8] = include_bytes!("../contract.wasm");

fn init_msg_expire_by_height(height: u64) -> InitMsg {
    InitMsg {
        arbiter: HumanAddr::from("verifies"),
        recipient: HumanAddr::from("benefits"),
        end_height: Some(height),
        end_time: None,
    }
}

fn mock_env_height<A: Api>(api: &A, signer: &str, sent: &[Coin], height: u64, time: u64) -> Env {
    let mut env = mock_env(api, signer, sent);
    env.block.height = height;
    env.block.time = time;
    env
}

#[test]
fn proper_initialization() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = init_msg_expire_by_height(1000);
    let env = mock_env_height(&deps.api, "creator", &coins(1000, "earth"), 876, 0);
    let res: InitResponse = init(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let api = deps.api;
    deps.with_storage(|store| {
        let state = config(store).load().unwrap();
        assert_eq!(
            state,
            State {
                arbiter: api.canonical_address(&HumanAddr::from("verifies")).unwrap(),
                recipient: api.canonical_address(&HumanAddr::from("benefits")).unwrap(),
                source: api.canonical_address(&HumanAddr::from("creator")).unwrap(),
                end_height: Some(1000),
                end_time: None,
            }
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn cannot_initialize_expired() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = init_msg_expire_by_height(1000);
    let env = mock_env_height(&deps.api, "creator", &coins(1000, "earth"), 1001, 0);
    let res: InitResult = init(&mut deps, env, msg);
    match res.unwrap_err() {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "creating expired escrow"),
        e => panic!("unexpected error: {:?}", e),
    }
}

#[test]
fn handle_approve() {
    let mut deps = mock_instance(WASM, &[]);

    // initialize the store
    let init_amount = coins(1000, "earth");
    let init_env = mock_env_height(&deps.api, "creator", &init_amount, 876, 0);
    let msg = init_msg_expire_by_height(1000);
    let init_res: InitResponse = init(&mut deps, init_env, msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    // TODO: update balance to init_amount here

    // beneficiary cannot release it
    let msg = HandleMsg::Approve { quantity: None };
    let env = mock_env_height(&deps.api, "beneficiary", &[], 900, 0);
    let handle_res: HandleResult = handle(&mut deps, env, msg.clone());
    match handle_res.unwrap_err() {
        StdError::Unauthorized { .. } => {}
        e => panic!("unexpected error: {:?}", e),
    }

    // verifier cannot release it when expired
    let env = mock_env_height(&deps.api, "verifies", &[], 1100, 0);
    let handle_res: HandleResult = handle(&mut deps, env, msg.clone());
    match handle_res.unwrap_err() {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "escrow expired"),
        e => panic!("unexpected error: {:?}", e),
    }

    // complete release by verfier, before expiration
    let env = mock_env_height(&deps.api, "verifies", &[], 999, 0);
    let handle_res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();
    assert_eq!(1, handle_res.messages.len());
    let msg = handle_res.messages.get(0).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from("cosmos2contract"),
            to_address: HumanAddr::from("benefits"),
            amount: coins(1000, "earth"),
        })
    );

    // partial release by verfier, before expiration
    let partial_msg = HandleMsg::Approve {
        quantity: Some(coins(500, "earth")),
    };
    let env = mock_env_height(&deps.api, "verifies", &[], 999, 0);
    let handle_res: HandleResponse = handle(&mut deps, env, partial_msg).unwrap();
    assert_eq!(1, handle_res.messages.len());
    let msg = handle_res.messages.get(0).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from("cosmos2contract"),
            to_address: HumanAddr::from("benefits"),
            amount: coins(500, "earth"),
        })
    );
}

#[test]
fn handle_refund() {
    let mut deps = mock_instance(WASM, &[]);

    // initialize the store
    let init_amount = coins(1000, "earth");
    let init_env = mock_env_height(&deps.api, "creator", &init_amount, 876, 0);
    let msg = init_msg_expire_by_height(1000);
    let init_res: InitResponse = init(&mut deps, init_env, msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    // TODO: update balance to init_amount here

    // cannot release when unexpired
    let msg = HandleMsg::Refund {};
    let env = mock_env_height(&deps.api, "anybody", &[], 800, 0);
    let handle_res: HandleResult = handle(&mut deps, env, msg.clone());
    match handle_res.unwrap_err() {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "escrow not yet expired"),
        e => panic!("unexpected error: {:?}", e),
    }

    // anyone can release after expiration
    let env = mock_env_height(&deps.api, "anybody", &[], 1001, 0);
    let handle_res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();
    assert_eq!(1, handle_res.messages.len());
    let msg = handle_res.messages.get(0).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from("cosmos2contract"),
            to_address: HumanAddr::from("creator"),
            amount: coins(1000, "earth"),
        })
    );
}
