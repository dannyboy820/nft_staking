use cosmwasm::serde::{from_slice, to_vec};
use cosmwasm::types::{coin, mock_params, Coin, ContractResult, CosmosMsg, Params};
use cosmwasm_vm::testing::{handle, init, mock_instance, query};

use escrow::contract::{raw_query, HandleMsg, InitMsg, State, CONFIG_KEY};

/**
This integration test tries to run and call the generated wasm.
It depends on a release build being available already. You can create that with: `cargo wasm`
Then running `cargo test` will validate we can properly call into that generated data.

You can copy the code from unit tests here verbatim, then make a few changes:

Replace `let mut store = MockStorage::new();` with `let mut store = mock_instance(WASM);`.

Replace `query(&store...` with `query(&mut store..` (we need mutability to pass args into wasm).

Any switches on error results, using types will have to use raw strings from formatted errors.
You can use a pattern like this to assert specific errors:

```
match res {
    ContractResult::Err(msg) => assert_eq!(msg, "Contract error: creating expired escrow"),
    _=> panic!("expected error"),
}
```
**/
static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/escrow.wasm");

fn init_msg(height: i64, time: i64) -> Vec<u8> {
    to_vec(&InitMsg {
        arbiter: String::from("verifies"),
        recipient: String::from("benefits"),
        end_height: height,
        end_time: time,
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

#[test]
fn proper_initialization() {
    let mut store = mock_instance(WASM);
    let msg = init_msg(1000, 0);
    let params = mock_params_height("creator", &coin("1000", "earth"), &[], 876, 0);
    let res = init(&mut store, params, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let q_res = query(&mut store, raw_query(CONFIG_KEY).unwrap()).unwrap();
    let model = q_res.results.first().expect("no data stored");
    let state: State = from_slice(&model.val).unwrap();
    assert_eq!(
        state,
        State {
            arbiter: String::from("verifies"),
            recipient: String::from("benefits"),
            source: String::from("creator"),
            end_height: 1000,
            end_time: 0,
        }
    );
}

#[test]
fn cannot_initialize_expired() {
    let mut store = mock_instance(WASM);
    let msg = init_msg(1000, 0);
    let params = mock_params_height("creator", &coin("1000", "earth"), &[], 1001, 0);
    let res = init(&mut store, params, msg);
    match res {
        ContractResult::Err(msg) => assert_eq!(msg, "Contract error: creating expired escrow"),
        _ => panic!("expected error"),
    }
}

#[test]
fn fails_on_bad_init_data() {
    let mut store = mock_instance(WASM);
    let bad_msg = b"{}".to_vec();
    let params = mock_params_height("creator", &coin("1000", "earth"), &[], 876, 0);
    let res = init(&mut store, params, bad_msg);
    match res {
        ContractResult::Err(msg) => {
            assert_eq!(msg, "Error parsing InitMsg: missing field `arbiter`")
        }
        _ => panic!("expected error"),
    }
}

#[test]
fn handle_approve() {
    let mut store = mock_instance(WASM);

    // initialize the store
    let msg = init_msg(1000, 0);
    let params = mock_params_height("creator", &coin("1000", "earth"), &[], 876, 0);
    let init_res = init(&mut store, params, msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    // beneficiary cannot release it
    let msg = to_vec(&HandleMsg::Approve { quantity: None }).unwrap();
    let params = mock_params_height(
        "beneficiary",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        900,
        0,
    );
    let handle_res = handle(&mut store, params, msg.clone());
    match handle_res {
        ContractResult::Err(msg) => assert_eq!(msg, "Unauthorized"),
        _ => panic!("expected error"),
    }

    // verifier cannot release it when expired
    let params = mock_params_height(
        "verifies",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        1100,
        0,
    );
    let handle_res = handle(&mut store, params, msg.clone());
    match handle_res {
        ContractResult::Err(msg) => assert_eq!(msg, "Contract error: escrow expired"),
        _ => panic!("expected error"),
    }

    // complete release by verfier, before expiration
    let params = mock_params_height(
        "verifies",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        999,
        0,
    );
    let handle_res = handle(&mut store, params, msg.clone()).unwrap();
    assert_eq!(1, handle_res.messages.len());
    let msg = handle_res.messages.get(0).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Send {
            from_address: "cosmos2contract".to_string(),
            to_address: "benefits".to_string(),
            amount: coin("1000", "earth"),
        }
    );

    // partial release by verfier, before expiration
    let partial_msg = to_vec(&HandleMsg::Approve {
        quantity: Some(coin("500", "earth")),
    })
    .unwrap();
    let params = mock_params_height(
        "verifies",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        999,
        0,
    );
    let handle_res = handle(&mut store, params, partial_msg).unwrap();
    assert_eq!(1, handle_res.messages.len());
    let msg = handle_res.messages.get(0).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Send {
            from_address: "cosmos2contract".to_string(),
            to_address: "benefits".to_string(),
            amount: coin("500", "earth"),
        }
    );
}

#[test]
fn handle_refund() {
    let mut store = mock_instance(WASM);

    // initialize the store
    let msg = init_msg(1000, 0);
    let params = mock_params_height("creator", &coin("1000", "earth"), &[], 876, 0);
    let init_res = init(&mut store, params, msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    // cannot release when unexpired
    let msg = to_vec(&HandleMsg::Refund {}).unwrap();
    let params = mock_params_height(
        "anybody",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        800,
        0,
    );
    let handle_res = handle(&mut store, params, msg.clone());
    match handle_res {
        ContractResult::Err(msg) => assert_eq!(msg, "Contract error: escrow not yet expired"),
        _ => panic!("expected error"),
    }

    // anyone can release after expiration
    let params = mock_params_height(
        "anybody",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        1001,
        0,
    );
    let handle_res = handle(&mut store, params, msg.clone()).unwrap();
    assert_eq!(1, handle_res.messages.len());
    let msg = handle_res.messages.get(0).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Send {
            from_address: "cosmos2contract".to_string(),
            to_address: "creator".to_string(),
            amount: coin("1000", "earth"),
        }
    );
}
