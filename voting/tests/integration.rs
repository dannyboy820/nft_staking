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
//!      // now you don't mock_init anymore
//! 3. If you access raw storage, where ever you see something like:
//!      deps.storage.get(CONFIG_KEY).expect("no data stored");
//!    replace it with:
//!      deps.with_storage(|store| {
//!          let data = store.get(CONFIG_KEY).expect("no data stored");
//!          //...
//!      });
//! 4. Anywhere you see query(&deps, ...) you must replace it with query(&mut deps, mock_env(), ...)

use cosmwasm_std::{
    attr, coins, from_binary, BankMsg, Coin, CosmosMsg, Env, HandleResponse, HumanAddr,
    InitResponse, MessageInfo, Uint128,
};
use cosmwasm_storage::to_length_prefixed;
use cosmwasm_vm::testing::{handle, init, mock_env, mock_info, mock_instance, query};
use cosmwasm_vm::{from_slice, Api, Storage};
use cw_voting::contract::VOTING_TOKEN;
use cw_voting::msg::{HandleMsg, InitMsg, PollResponse, QueryMsg};
use cw_voting::state::{PollStatus, State};

// This line will test the output of cargo wasm
static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/cw_voting.wasm");
// You can uncomment this line instead to test productionified build from rust-optimizer
// static WASM: &[u8] = include_bytes!("../contract.wasm");

const DEFAULT_END_HEIGHT: u64 = 100800u64;
const TEST_CREATOR: &str = "creator";
const TEST_VOTER: &str = "voter1";
const TEST_VOTER_2: &str = "voter2";

fn mock_info_height(sender: &str, sent: &[Coin], height: u64, time: u64) -> (Env, MessageInfo) {
    let info = mock_info(sender, sent);
    let mut env = mock_env();
    env.block.height = height;
    env.block.time = time;
    (env, info)
}

fn init_msg() -> InitMsg {
    InitMsg {
        denom: String::from(VOTING_TOKEN),
    }
}

fn address(index: u8) -> HumanAddr {
    match index {
        0 => HumanAddr(TEST_CREATOR.to_string()), // contract initializer
        1 => HumanAddr(TEST_VOTER.to_string()),
        2 => HumanAddr(TEST_VOTER_2.to_string()),
        _ => panic!("Unsupported address index"),
    }
}

#[test]
fn proper_initialization() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = init_msg();
    let info = mock_info(
        &HumanAddr(TEST_CREATOR.to_string()),
        &coins(2, VOTING_TOKEN),
    );
    let res: InitResponse = init(&mut deps, mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    let api = deps.api;

    deps.with_storage(|store| {
        let config_key_raw = to_length_prefixed(b"config");
        let state: State = from_slice(&store.get(&config_key_raw).0.unwrap().unwrap()).unwrap();
        assert_eq!(
            state,
            State {
                denom: String::from(VOTING_TOKEN),
                owner: api
                    .canonical_address(&HumanAddr::from(&HumanAddr(TEST_CREATOR.to_string())))
                    .0
                    .unwrap(),
                poll_count: 0,
                staked_tokens: Uint128::zero(),
            }
        );
        Ok(())
    })
    .unwrap();
}

fn create_poll_msg(
    quorum_percentage: u8,
    description: String,
    start_height: Option<u64>,
    end_height: Option<u64>,
) -> HandleMsg {
    let msg = HandleMsg::CreatePoll {
        quorum_percentage: Some(quorum_percentage),
        description,
        start_height,
        end_height,
    };
    msg
}

#[test]
fn happy_days_create_poll() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let info = mock_info(
        &HumanAddr(TEST_CREATOR.to_string()),
        &coins(2, VOTING_TOKEN),
    );
    let res: InitResponse = init(&mut deps, mock_env(), info.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let quorum = 30;
    let msg = create_poll_msg(quorum, "test".to_string(), None, Some(DEFAULT_END_HEIGHT));

    let handle_res: HandleResponse =
        handle(&mut deps, mock_env(), info.clone(), msg.clone()).unwrap();

    assert_create_poll_result(
        1,
        quorum,
        DEFAULT_END_HEIGHT,
        0,
        &HumanAddr(TEST_CREATOR.to_string()),
        handle_res,
    );
}

#[test]
fn create_poll_no_quorum() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let (env, info) = mock_info_height(TEST_CREATOR, &[], 0, 10000);
    let res: InitResponse = init(&mut deps, env.clone(), info.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let quorum = 0;
    let msg = create_poll_msg(quorum, "test".to_string(), None, None);

    let handle_res: HandleResponse = handle(&mut deps, env, info, msg.clone()).unwrap();
    assert_create_poll_result(
        1,
        quorum,
        DEFAULT_END_HEIGHT,
        0,
        &HumanAddr(TEST_CREATOR.to_string()),
        handle_res,
    );
}

#[test]
fn happy_days_end_poll() {
    const POLL_END_HEIGHT: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 1000;
    let mut deps = mock_instance(WASM, &coins(stake_amount, VOTING_TOKEN));

    let msg = init_msg();
    let (mut creator_env, creator_info) =
        mock_info_height(TEST_CREATOR, &[], POLL_END_HEIGHT, 10000);
    let res: InitResponse =
        init(&mut deps, creator_env.clone(), creator_info.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let msg = create_poll_msg(
        0,
        "test".to_string(),
        None,
        Some(creator_env.block.height + 1),
    );

    let handle_res: HandleResponse =
        handle(&mut deps, creator_env.clone(), creator_info.clone(), msg).unwrap();

    assert_create_poll_result(
        POLL_ID,
        0,
        creator_env.block.height + 1,
        0,
        &HumanAddr(TEST_CREATOR.to_string()),
        handle_res,
    );

    let msg = HandleMsg::StakeVotingTokens {};
    let info = mock_info(TEST_VOTER, &coins(stake_amount, VOTING_TOKEN));

    let handle_res: HandleResponse =
        handle(&mut deps, mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());

    let msg = HandleMsg::CastVote {
        poll_id: POLL_ID,
        vote: "yes".to_string(),
        weight: Uint128::from(stake_amount),
    };
    let handle_res: HandleResponse = handle(&mut deps, mock_env(), info.clone(), msg).unwrap();

    assert_eq!(
        handle_res.attributes,
        vec![
            attr("action", "vote_casted"),
            attr("poll_id", POLL_ID),
            attr("weight", "1000"),
            attr("voter", TEST_VOTER),
        ]
    );
    creator_env.block.height = &creator_env.block.height + 1;

    let msg = HandleMsg::EndPoll { poll_id: POLL_ID };

    let handle_res: HandleResponse =
        handle(&mut deps, creator_env.clone(), creator_info.clone(), msg).unwrap();

    assert_eq!(
        handle_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );

    let res = query(&mut deps, mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(PollStatus::Passed, value.status);
}

#[test]
fn end_poll_zero_quorum() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let creator = &address(0);
    let (env, info) = mock_info_height(creator, &coins(1000, "token"), 0, 0);
    let res: InitResponse = init(&mut deps, env.clone(), info.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    //create poll
    let (env2, _) = mock_info_height(&address(0), &[], 1001, 0);
    let msg = create_poll_msg(0, "test".to_string(), None, Some(env2.block.height));
    let handle_res: HandleResponse =
        handle(&mut deps, env.clone(), info.clone(), msg.clone()).unwrap();
    assert_create_poll_result(1, 0, 1001, 0, creator, handle_res);

    //end poll
    let msg = HandleMsg::EndPoll { poll_id: 1 };
    let handle_res: HandleResponse = handle(&mut deps, env2.clone(), info.clone(), msg).unwrap();

    assert_eq!(
        handle_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );
    let res = query(&mut deps, mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(PollStatus::Rejected, value.status);
}

#[test]
fn end_poll_quorum_rejected() {
    let stake_amount = 100;
    let mut deps = mock_instance(WASM, &coins(stake_amount, VOTING_TOKEN));
    let msg = init_msg();
    let (mut env, info) = mock_info_height(TEST_CREATOR, &coins(stake_amount, VOTING_TOKEN), 0, 1);
    let init_res: InitResponse = init(&mut deps, env.clone(), info.clone(), msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    let msg = create_poll_msg(30, "test".to_string(), None, Some(&env.block.height + 1));

    let handle_res: HandleResponse =
        handle(&mut deps, env.clone(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        handle_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", &HumanAddr(TEST_CREATOR.to_string())),
            attr("poll_id", "1"),
            attr("quorum_percentage", "30"),
            attr("end_height", "1"),
            attr("start_height", "0"),
        ]
    );

    let msg = HandleMsg::StakeVotingTokens {};

    let handle_res: HandleResponse =
        handle(&mut deps, env.clone(), info.clone(), msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());

    let msg = HandleMsg::CastVote {
        poll_id: 1,
        vote: "yes".to_string(),
        weight: Uint128::from(10u128),
    };
    let handle_res: HandleResponse = handle(&mut deps, env.clone(), info.clone(), msg).unwrap();

    assert_eq!(
        handle_res.attributes,
        vec![
            attr("action", "vote_casted"),
            attr("poll_id", "1"),
            attr("weight", "10"),
            attr("voter", TEST_CREATOR),
        ]
    );

    let msg = HandleMsg::EndPoll { poll_id: 1 };

    env.block.height = &env.block.height + 2;

    let handle_res: HandleResponse =
        handle(&mut deps, env.clone(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        handle_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );

    let res = query(&mut deps, mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(PollStatus::Rejected, value.status);
}

#[test]
fn end_poll_nay_rejected() {
    let voter1_stake = 100;
    let voter2_stake = 1000;
    let stake_amount = 100;
    let mut deps = mock_instance(WASM, &coins(stake_amount, VOTING_TOKEN));
    let msg = init_msg();
    let (mut creator_env, creator_info) = mock_info_height(TEST_CREATOR, &[], 0, 0);
    let init_res: InitResponse =
        init(&mut deps, creator_env.clone(), creator_info.clone(), msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    let msg = create_poll_msg(
        10,
        "test".to_string(),
        None,
        Some(creator_env.block.height + 1),
    );

    let handle_res: HandleResponse = handle(
        &mut deps,
        creator_env.clone(),
        creator_info.clone(),
        msg.clone(),
    )
    .unwrap();
    assert_eq!(
        handle_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", &HumanAddr(TEST_CREATOR.to_string())),
            attr("poll_id", "1"),
            attr("quorum_percentage", "10"),
            attr("end_height", "1"),
            attr("start_height", "0"),
        ]
    );

    let msg = HandleMsg::StakeVotingTokens {};
    let info = mock_info(TEST_VOTER, &coins(voter1_stake, VOTING_TOKEN));

    let handle_res: HandleResponse = handle(&mut deps, mock_env(), info, msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());

    let msg = HandleMsg::StakeVotingTokens {};
    let info = mock_info(TEST_VOTER_2, &coins(voter2_stake, VOTING_TOKEN));

    let handle_res: HandleResponse = handle(&mut deps, mock_env(), info, msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());

    let info = mock_info(TEST_VOTER_2, &[]);
    let msg = HandleMsg::CastVote {
        poll_id: 1,
        vote: "no".to_string(),
        weight: Uint128::from(voter2_stake),
    };
    let handle_res: HandleResponse = handle(&mut deps, mock_env(), info, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER_2, voter2_stake, 1, handle_res);

    let msg = HandleMsg::EndPoll { poll_id: 1 };

    creator_env.block.height = &creator_env.block.height + 2;
    let handle_res: HandleResponse = handle(
        &mut deps,
        creator_env.clone(),
        creator_info.clone(),
        msg.clone(),
    )
    .unwrap();
    assert_eq!(
        handle_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Threshold not reached"),
            attr("passed", "false"),
        ]
    );

    let res = query(&mut deps, mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(PollStatus::Rejected, value.status);
}

#[test]
fn happy_days_cast_vote() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let creator = &address(0);
    let (env, info) = mock_info_height(creator, &[], 0, 0);
    let res: InitResponse = init(&mut deps, env.clone(), info.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let quorum_percentage = 30;

    let msg = create_poll_msg(quorum_percentage, "test".to_string(), None, None);

    let handle_res: HandleResponse = handle(&mut deps, env, info, msg.clone()).unwrap();
    assert_create_poll_result(
        1,
        quorum_percentage,
        DEFAULT_END_HEIGHT,
        0,
        &HumanAddr(TEST_CREATOR.to_string()),
        handle_res,
    );

    let msg = HandleMsg::StakeVotingTokens {};
    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));

    let handle_res: HandleResponse = handle(&mut deps, mock_env(), info, msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());

    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let weight = 10u128;
    let msg = HandleMsg::CastVote {
        poll_id: 1,
        vote: "yes".to_string(),
        weight: Uint128::from(weight),
    };

    let handle_res: HandleResponse = handle(&mut deps, mock_env(), info, msg.clone()).unwrap();
    assert_cast_vote_success(TEST_VOTER, weight, 1, handle_res);
}

#[test]
fn happy_days_withdraw_voting_tokens() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let creator = &address(0);
    let (env, info) = mock_info_height(creator, &[], 0, 0);
    let res: InitResponse = init(&mut deps, env.clone(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    let msg = HandleMsg::StakeVotingTokens {};
    let staked_tokens = 11;
    let info = mock_info(TEST_VOTER, &coins(staked_tokens, VOTING_TOKEN));

    let handle_res: HandleResponse = handle(&mut deps, env, info, msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());

    let api = deps.api;
    //confirm stake increased
    deps.with_storage(|store| {
        let config_key_raw = to_length_prefixed(b"config");
        let state: State = from_slice(&store.get(&config_key_raw).0.unwrap().unwrap()).unwrap();
        assert_eq!(
            state,
            State {
                denom: String::from(VOTING_TOKEN),
                owner: api
                    .canonical_address(&HumanAddr::from(&HumanAddr(TEST_CREATOR.to_string())))
                    .0
                    .unwrap(),
                poll_count: 0,
                staked_tokens: Uint128::from(staked_tokens),
            }
        );
        Ok(())
    })
    .unwrap();

    // withdraw all stake
    let info = mock_info(TEST_VOTER, &coins(staked_tokens, VOTING_TOKEN));
    let msg = HandleMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(staked_tokens)),
    };

    let handle_res: HandleResponse = handle(&mut deps, mock_env(), info, msg.clone()).unwrap();
    let msg = handle_res.messages.get(0).expect("no message");

    assert_eq!(
        msg,
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from("cosmos2contract"),
            to_address: HumanAddr::from(TEST_VOTER),
            amount: coins(staked_tokens, VOTING_TOKEN),
        })
    );

    // staked is reduced
    &deps
        .with_storage(|store| {
            let config_key_raw = to_length_prefixed(b"config");
            let state: State = from_slice(&store.get(&config_key_raw).0.unwrap().unwrap()).unwrap();
            assert_eq!(
                state,
                State {
                    denom: String::from(VOTING_TOKEN),
                    owner: api
                        .canonical_address(&HumanAddr::from(&HumanAddr(TEST_CREATOR.to_string())))
                        .0
                        .unwrap(),
                    poll_count: 0,
                    staked_tokens: Uint128::zero(),
                }
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn happy_days_stake_voting_tokens() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let creator = &address(0);
    let (env, info) = mock_info_height(creator, &[], 0, 0);
    let res: InitResponse = init(&mut deps, env.clone(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));

    let msg = HandleMsg::StakeVotingTokens {};
    let handle_res: HandleResponse = handle(&mut deps, env, info, msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());
}

// helper to confirm the expected create_poll response
fn assert_create_poll_result(
    poll_id: u64,
    quorum: u8,
    end_height: u64,
    start_height: u64,
    creator: &HumanAddr,
    handle_res: HandleResponse,
) {
    assert_eq!(
        handle_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", creator),
            attr("poll_id", poll_id.to_string()),
            attr("quorum_percentage", quorum.to_string()),
            attr("end_height", end_height.to_string()),
            attr("start_height", start_height.to_string()),
        ]
    );
}

fn assert_cast_vote_success(voter: &str, weight: u128, poll_id: u64, handle_res: HandleResponse) {
    assert_eq!(
        handle_res.attributes,
        vec![
            attr("action", "vote_casted"),
            attr("poll_id", poll_id.to_string()),
            attr("weight", weight.to_string()),
            attr("voter", voter),
        ]
    );
}
