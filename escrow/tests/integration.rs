use cosmwasm::mock::MockStorage;
use cosmwasm::serde::to_vec;
use cosmwasm::types::{coin, mock_params, Coin, ContractResult, CosmosMsg, Params};
use cosmwasm_vm::{call_handle, call_init, Instance};

use escrow::contract::{HandleMsg, InitMsg};

/**
This integration test tries to run and call the generated wasm.
It depends on a release build being available already. You can create that with:

cargo wasm && wasm-gc ./target/wasm32-unknown-unknown/release/escrow.wasm

Then running `cargo test` will validate we can properly call into that generated data.
**/
static WASM: &[u8] = include_bytes!("../../target/wasm32-unknown-unknown/release/escrow.wasm");

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
    let storage = MockStorage::new();
    let mut instance = Instance::from_code(&WASM, storage).unwrap();

    let msg = init_msg(1000, 0);
    let params = mock_params_height("creator", &coin("1000", "earth"), &[], 876, 0);
    let res = call_init(&mut instance, &params, &msg).unwrap().unwrap();
    assert_eq!(0, res.messages.len());
}

#[test]
fn cannot_initialize_expired() {
    let storage = MockStorage::new();
    let mut instance = Instance::from_code(&WASM, storage).unwrap();

    let msg = init_msg(1000, 0);
    let params = mock_params_height("creator", &coin("1000", "earth"), &[], 1001, 0);
    let res = call_init(&mut instance, &params, &msg).unwrap();
    match res {
        ContractResult::Ok(_) => panic!("expected error"),
        ContractResult::Err(msg) => {
            assert_eq!(msg, "Contract error: creating expired escrow".to_string())
        }
    }
}

#[test]
fn fails_on_bad_init_data() {
    let storage = MockStorage::new();
    let mut instance = Instance::from_code(&WASM, storage).unwrap();

    let bad_msg = b"{}".to_vec();
    let params = mock_params_height("creator", &coin("1000", "earth"), &[], 876, 0);
    let res = call_init(&mut instance, &params, &bad_msg).unwrap();
    match res {
        ContractResult::Ok(_) => panic!("expected error"),
        ContractResult::Err(msg) => {
            assert_eq!(msg, "Parse error: missing field `arbiter`".to_string())
        }
    }
}

#[test]
fn handle_approve() {
    let storage = MockStorage::new();
    let mut instance = Instance::from_code(&WASM, storage).unwrap();

    // initialize the store
    let msg = init_msg(1000, 0);
    let params = mock_params_height("creator", &coin("1000", "earth"), &[], 876, 0);
    let init_res = call_init(&mut instance, &params, &msg).unwrap().unwrap();
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
    let handle_res = call_handle(&mut instance, &params, &msg).unwrap();
    match handle_res {
        ContractResult::Ok(_) => panic!("expected error"),
        ContractResult::Err(msg) => assert_eq!(msg, "Unauthorized".to_string()),
    }

    // verifier cannot release it when expired
    let params = mock_params_height(
        "verifies",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        1100,
        0,
    );
    let handle_res = call_handle(&mut instance, &params, &msg).unwrap();
    match handle_res {
        ContractResult::Ok(_) => panic!("expected error"),
        ContractResult::Err(msg) => assert_eq!(msg, "Contract error: escrow expired".to_string()),
    }

    // complete release by verfier, before expiration
    let params = mock_params_height(
        "verifies",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        999,
        0,
    );
    let handle_res = call_handle(&mut instance, &params, &msg).unwrap().unwrap();
    assert_eq!(1, handle_res.messages.len());
    let msg = handle_res.messages.get(0).expect("no message");
    match &msg {
        CosmosMsg::Send {
            from_address,
            to_address,
            amount,
        } => {
            assert_eq!("cosmos2contract", from_address);
            assert_eq!("benefits", to_address);
            assert_eq!(1, amount.len());
            let coin = amount.get(0).expect("No coin");
            assert_eq!(coin.denom, "earth");
            assert_eq!(coin.amount, "1000");
        }
        _ => panic!("Unexpected message type"),
    }

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
    let handle_res = call_handle(&mut instance, &params, &partial_msg)
        .unwrap()
        .unwrap();
    assert_eq!(1, handle_res.messages.len());
    let msg = handle_res.messages.get(0).expect("no message");
    match &msg {
        CosmosMsg::Send {
            from_address,
            to_address,
            amount,
        } => {
            assert_eq!("cosmos2contract", from_address);
            assert_eq!("benefits", to_address);
            assert_eq!(1, amount.len());
            let coin = amount.get(0).expect("No coin");
            assert_eq!(coin.denom, "earth");
            assert_eq!(coin.amount, "500");
        }
        _ => panic!("Unexpected message type"),
    }
}

#[test]
fn handle_refund() {
    let storage = MockStorage::new();
    let mut instance = Instance::from_code(&WASM, storage).unwrap();

    // initialize the store
    let msg = init_msg(1000, 0);
    let params = mock_params_height("creator", &coin("1000", "earth"), &[], 876, 0);
    let init_res = call_init(&mut instance, &params, &msg).unwrap().unwrap();
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
    let handle_res = call_handle(&mut instance, &params, &msg).unwrap();
    match handle_res {
        ContractResult::Ok(_) => panic!("expected error"),
        ContractResult::Err(msg) => {
            assert_eq!(msg, "Contract error: escrow not yet expired".to_string())
        }
    }

    // anyone can release after expiration
    let params = mock_params_height(
        "anybody",
        &coin("0", "earth"),
        &coin("1000", "earth"),
        1001,
        0,
    );
    let handle_res = call_handle(&mut instance, &params, &msg).unwrap().unwrap();
    assert_eq!(1, handle_res.messages.len());
    let msg = handle_res.messages.get(0).expect("no message");
    match &msg {
        CosmosMsg::Send {
            from_address,
            to_address,
            amount,
        } => {
            assert_eq!("cosmos2contract", from_address);
            assert_eq!("creator", to_address);
            assert_eq!(1, amount.len());
            let coin = amount.get(0).expect("No coin");
            assert_eq!(coin.denom, "earth");
            assert_eq!(coin.amount, "1000");
        }
        _ => panic!("Unexpected message type"),
    }
}
