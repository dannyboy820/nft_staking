use cosmwasm::mock::mock_params;
use cosmwasm::traits::Api;
use cosmwasm::types::{coin, Coin, ContractResult, CosmosMsg, HumanAddr, Params};

use cosmwasm_vm::testing::{handle, init, mock_instance};

use cw_escrow::contract::{config, State};
use cw_escrow::msg::{HandleMsg, InitMsg};

/**
This integration test tries to run and call the generated wasm.
It depends on a release build being available already. You can create that with:

cargo wasm && wasm-gc ./target/wasm32-unknown-unknown/release/hackatom.wasm

Then running `cargo test` will validate we can properly call into that generated data.

You can easily convert unit tests to integration tests.
1. First copy them over verbatum,
2. Then change
    let mut deps = dependencies(20);
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
static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/cw_escrow.wasm");
// You can uncomment this line instead to test productionified build from cosmwasm-opt
// static WASM: &[u8] = include_bytes!("../contract.wasm");

fn init_msg(height: i64, time: i64) -> InitMsg {
    InitMsg {
        arbiter: HumanAddr::from("verifies"),
        recipient: HumanAddr::from("benefits"),
        end_height: height,
        end_time: time,
    }
}

fn mock_params_height<A: Api>(
    api: &A,
    signer: &str,
    sent: &[Coin],
    balance: &[Coin],
    height: i64,
    time: i64,
) -> Params {
    let mut params = mock_params(api, signer, sent, balance);
    params.block.height = height;
    params.block.time = time;
    params
}

#[test]
fn proper_initialization() {
    let mut deps = mock_instance(WASM);

    let msg = init_msg(1000, 0);
    let params = mock_params_height(&deps.api, "creator", &coin("1000", "earth"), &[], 876, 0);
    let res = init(&mut deps, params, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    deps.with_storage(|store| {
        let state = config(store).load().unwrap();
        assert_eq!(
            state,
            State {
                arbiter: deps
                    .api
                    .canonical_address(&HumanAddr::from("verifies"))
                    .unwrap(),
                recipient: deps
                    .api
                    .canonical_address(&HumanAddr::from("benefits"))
                    .unwrap(),
                source: deps
                    .api
                    .canonical_address(&HumanAddr::from("creator"))
                    .unwrap(),
                end_height: 1000,
                end_time: 0,
            }
        );
    });
}

#[test]
fn cannot_initialize_expired() {
    let mut deps = mock_instance(WASM);

    let msg = init_msg(1000, 0);
    let params = mock_params_height(&deps.api, "creator", &coin("1000", "earth"), &[], 1001, 0);
    let res = init(&mut deps, params, msg);
    if let ContractResult::Err(msg) = res {
        assert_eq!(msg, "Contract error: creating expired escrow".to_string());
    } else {
        panic!("expected error");
    }
}

#[test]
fn handle_approve() {
    let mut deps = mock_instance(WASM);

    // initialize the store
    let msg = init_msg(1000, 0);
    let params = mock_params_height(&deps.api, "creator", &coin("1000", "earth"), &[], 876, 0);
    let init_res = init(&mut deps, params, msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    // beneficiary cannot release it
    let msg = HandleMsg::Approve { quantity: None };
    let params = mock_params_height(
        &deps.api,
        "beneficiary",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        900,
        0,
    );
    let handle_res = handle(&mut deps, params, msg.clone());
    match handle_res {
        ContractResult::Err(msg) => assert_eq!(msg, "Unauthorized".to_string()),
        _ => panic!("expected error"),
    }

    // verifier cannot release it when expired
    let params = mock_params_height(
        &deps.api,
        "verifies",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        1100,
        0,
    );
    let handle_res = handle(&mut deps, params, msg.clone());
    match handle_res {
        ContractResult::Err(msg) => assert_eq!(msg, "Contract error: escrow expired".to_string()),
        _ => panic!("expected error"),
    }

    // complete release by verfier, before expiration
    let params = mock_params_height(
        &deps.api,
        "verifies",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        999,
        0,
    );
    let handle_res = handle(&mut deps, params, msg.clone()).unwrap();
    assert_eq!(1, handle_res.messages.len());
    let msg = handle_res.messages.get(0).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Send {
            from_address: HumanAddr::from("cosmos2contract"),
            to_address: HumanAddr::from("benefits"),
            amount: coin("1000", "earth"),
        }
    );

    // partial release by verfier, before expiration
    let partial_msg = HandleMsg::Approve {
        quantity: Some(coin("500", "earth")),
    };
    let params = mock_params_height(
        &deps.api,
        "verifies",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        999,
        0,
    );
    let handle_res = handle(&mut deps, params, partial_msg).unwrap();
    assert_eq!(1, handle_res.messages.len());
    let msg = handle_res.messages.get(0).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Send {
            from_address: HumanAddr::from("cosmos2contract"),
            to_address: HumanAddr::from("benefits"),
            amount: coin("500", "earth"),
        }
    );
}

#[test]
fn handle_refund() {
    let mut deps = mock_instance(WASM);

    // initialize the store
    let msg = init_msg(1000, 0);
    let params = mock_params_height(&deps.api, "creator", &coin("1000", "earth"), &[], 876, 0);
    let init_res = init(&mut deps, params, msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    // cannot release when unexpired
    let msg = HandleMsg::Refund {};
    let params = mock_params_height(
        &deps.api,
        "anybody",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        800,
        0,
    );
    let handle_res = handle(&mut deps, params, msg.clone());
    match handle_res {
        ContractResult::Err(msg) => {
            assert_eq!(msg, "Contract error: escrow not yet expired".to_string())
        }
        _ => panic!("expected error"),
    }

    // anyone can release after expiration
    let params = mock_params_height(
        &deps.api,
        "anybody",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        1001,
        0,
    );
    let handle_res = handle(&mut deps, params, msg.clone()).unwrap();
    assert_eq!(1, handle_res.messages.len());
    let msg = handle_res.messages.get(0).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Send {
            from_address: HumanAddr::from("cosmos2contract"),
            to_address: HumanAddr::from("creator"),
            amount: coin("1000", "earth"),
        }
    );
}
