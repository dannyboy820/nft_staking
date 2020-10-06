//! This integration test tries to run and call the generated wasm.
//! It depends on a Wasm build being available, which you can create with `cargo wasm`.
//! Then running `cargo integration-test` will validate we can properly call into that generated Wasm.
//!
//! You can easily convert unit tests to integration tests as follows:
//! 1. Copy them over verbatim
//! 2. Then change
//!      let mut deps = mock_dependencies(20, &[]);
//!    to
//!      let mut deps = mock_instance(WASM, &[]);
//! 3. If you access raw storage, where ever you see something like:
//!      deps.storage.get(CONFIG_KEY).expect("no data stored");
//!    replace it with:
//!      deps.with_storage(|store| {
//!          let data = store.get(CONFIG_KEY).expect("no data stored");
//!          //...
//!      });
//! 4. Anywhere you see query(&deps, ...) you must replace it with query(&mut deps, ...)

use cosmwasm_std::{
    coins, from_slice, BankMsg, CosmosMsg, HandleResponse, HumanAddr, InitResponse,
};
use cosmwasm_vm::testing::{handle, init, mock_env, mock_instance, query};

use cw_mask::msg::{HandleMsg, InitMsg, OwnerResponse, QueryMsg};

// This line will test the output of cargo wasm
static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/cw_mask.wasm");
// You can uncomment this line instead to test productionified build from rust-optimizer
// static WASM: &[u8] = include_bytes!("../contract.wasm");

#[test]
fn proper_initialization() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {};
    let env = mock_env("creator", &coins(1000, "earth"));

    // we can just call .unwrap() to assert this was a success
    let res: InitResponse = init(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(&mut deps, QueryMsg::Owner {}).unwrap();
    let value: OwnerResponse = from_slice(res.as_slice()).unwrap();
    assert_eq!("creator", value.owner.as_str());
}

#[test]
fn reflect() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {};
    let env = mock_env("creator", &coins(2, "token"));
    let _res: InitResponse = init(&mut deps, env, msg).unwrap();

    let env = mock_env("creator", &[]);
    let payload = vec![CosmosMsg::Bank(BankMsg::Send {
        from_address: env.contract.address.clone(),
        to_address: HumanAddr::from("friend"),
        amount: coins(1, "token"),
    })];
    let msg = HandleMsg::ReflectMsg {
        msgs: payload.clone(),
    };
    let res = handle(&mut deps, env, msg).unwrap();

    // should return payload
    assert_eq!(payload, res.messages);
}

#[test]
fn transfer() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {};
    let env = mock_env("creator", &coins(2, "token"));
    let _res: InitResponse = init(&mut deps, env, msg).unwrap();

    let env = mock_env("creator", &[]);
    let new_owner = HumanAddr::from("friend");
    let msg = HandleMsg::ChangeOwner {
        owner: new_owner.clone(),
    };
    let res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    // should change state
    assert_eq!(0, res.messages.len());
    let res = query(&mut deps, QueryMsg::Owner {}).unwrap();
    let value: OwnerResponse = from_slice(res.as_slice()).unwrap();
    assert_eq!("friend", value.owner.as_str());
}
