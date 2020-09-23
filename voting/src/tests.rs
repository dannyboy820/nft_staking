#[cfg(test)]
mod tests {
    use crate::contract::{handle, init, query, VOTING_TOKEN};
    use crate::error::ContractError;
    use crate::msg::{HandleMsg, InitMsg, PollResponse, QueryMsg};
    use crate::state::{config_read, PollStatus, State};
    use cosmwasm_std::testing::{mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage};
    use cosmwasm_std::{
        attr, coins, from_binary, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse,
        HumanAddr, StdError, Uint128,
    };

    const DEFAULT_END_HEIGHT: u64 = 100800u64;
    const TEST_CREATOR: &str = "creator";
    const TEST_VOTER: &str = "voter1";
    const TEST_VOTER_2: &str = "voter2";

    fn mock_init(mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>) {
        let msg = InitMsg {
            denom: String::from(VOTING_TOKEN),
        };

        let env = mock_env(TEST_CREATOR, &coins(2, &msg.denom));
        let _res = init(&mut deps, env, msg).expect("contract successfully handles InitMsg");
    }

    fn mock_env_height(sender: &str, sent: &[Coin], height: u64, time: u64) -> Env {
        let mut env = mock_env(sender, sent);
        env.block.height = height;
        env.block.time = time;
        env
    }

    fn init_msg() -> InitMsg {
        InitMsg {
            denom: String::from(VOTING_TOKEN),
        }
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = init_msg();
        let env = mock_env(TEST_CREATOR, &coins(2, VOTING_TOKEN));
        let res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let state = config_read(&mut deps.storage).load().unwrap();
        assert_eq!(
            state,
            State {
                denom: String::from(VOTING_TOKEN),
                owner: deps
                    .api
                    .canonical_address(&HumanAddr::from(TEST_CREATOR))
                    .unwrap(),
                poll_count: 0,
                staked_tokens: Uint128::zero(),
            }
        );
    }

    #[test]
    fn poll_not_found() {
        let mut deps = mock_dependencies(20, &[]);
        mock_init(&mut deps);

        let res = query(&deps, QueryMsg::Poll { poll_id: 1 });

        match res {
            Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Poll does not exist"),
            Err(e) => panic!("Unexpected error: {:?}", e),
            _ => panic!("Must return error"),
        }
    }

    #[test]
    fn fails_create_poll_invalid_quorum_percentage() {
        let mut deps = mock_dependencies(20, &[]);
        let env = mock_env("voter", &coins(11, VOTING_TOKEN));

        let qp = 101;
        let msg = create_poll_msg(qp, "test".to_string(), None, None);

        let res = handle(&mut deps, env, msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::PollQuorumPercentage { quorum_percentage }) => {
                assert_eq!(quorum_percentage, qp)
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn fails_create_poll_invalid_description() {
        let mut deps = mock_dependencies(20, &[]);
        let env = mock_env(TEST_VOTER, &coins(11, VOTING_TOKEN));

        let msg = create_poll_msg(30, "a".to_string(), None, None);

        match handle(&mut deps, env.clone(), msg) {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::DescriptionTooShort { .. }) => {}
            Err(_) => panic!("Unknown error"),
        }

        let msg = create_poll_msg(
            100,
            "01234567890123456789012345678901234567890123456789012345678901234".to_string(),
            None,
            None,
        );

        match handle(&mut deps, env.clone(), msg) {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::DescriptionTooLong { .. }) => {}
            Err(_) => panic!("Unknown error"),
        }
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
        let mut deps = mock_dependencies(20, &[]);
        mock_init(&mut deps);
        let env = mock_env_height(TEST_CREATOR, &coins(2, VOTING_TOKEN), 0, 10000);

        let quorum = 30;
        let msg = create_poll_msg(quorum, "test".to_string(), None, None);

        let handle_res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
        assert_create_poll_result(
            1,
            quorum,
            DEFAULT_END_HEIGHT,
            0,
            TEST_CREATOR,
            handle_res,
            &mut deps,
        );
    }

    #[test]
    fn create_poll_no_quorum() {
        let mut deps = mock_dependencies(20, &[]);
        mock_init(&mut deps);
        let env = mock_env_height(TEST_CREATOR, &coins(2, VOTING_TOKEN), 0, 10000);

        let quorum = 0;
        let msg = create_poll_msg(quorum, "test".to_string(), None, None);

        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
        assert_create_poll_result(
            1,
            quorum,
            DEFAULT_END_HEIGHT,
            0,
            TEST_CREATOR,
            handle_res,
            &mut deps,
        );
    }

    #[test]
    fn fails_end_poll_before_end_height() {
        let mut deps = mock_dependencies(20, &[]);
        mock_init(&mut deps);
        let env = mock_env_height(TEST_CREATOR, &coins(2, VOTING_TOKEN), 0, 10000);

        let msg_end_height = 10001;
        let msg = create_poll_msg(0, "test".to_string(), None, Some(msg_end_height));

        let handle_res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
        assert_create_poll_result(1, 0, msg_end_height, 0, TEST_CREATOR, handle_res, &mut deps);

        let res = query(&deps, QueryMsg::Poll { poll_id: 1 }).unwrap();
        let value: PollResponse = from_binary(&res).unwrap();
        assert_eq!(Some(10001), value.end_height);

        let msg = HandleMsg::EndPoll { poll_id: 1 };

        let handle_res = handle(&mut deps, env.clone(), msg);

        match handle_res {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::PollVotingPeriodNotExpired { expire_height }) => {
                assert_eq!(expire_height, msg_end_height)
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn happy_days_end_poll() {
        const POLL_END_HEIGHT: u64 = 1000;
        const POLL_ID: u64 = 1;
        let stake_amount = 1000;

        let mut deps = mock_dependencies(20, &coins(1000, VOTING_TOKEN));
        mock_init(&mut deps);
        let mut creator_env = mock_env_height(
            TEST_CREATOR,
            &coins(2, VOTING_TOKEN),
            POLL_END_HEIGHT,
            10000,
        );

        let msg = create_poll_msg(
            0,
            "test".to_string(),
            None,
            Some(creator_env.block.height + 1),
        );

        let handle_res = handle(&mut deps, creator_env.clone(), msg).unwrap();

        assert_create_poll_result(
            1,
            0,
            creator_env.block.height + 1,
            0,
            TEST_CREATOR,
            handle_res,
            &mut deps,
        );

        let msg = HandleMsg::StakeVotingTokens {};
        let env = mock_env(TEST_VOTER, &coins(stake_amount, VOTING_TOKEN));

        let handle_res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
        assert_stake_tokens_result(stake_amount, Some(1), handle_res, &mut deps);

        let msg = HandleMsg::CastVote {
            poll_id: 1,
            vote: "yes".to_string(),
            weight: Uint128::from(stake_amount),
        };
        let handle_res = handle(&mut deps, env.clone(), msg).unwrap();

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

        let msg = HandleMsg::EndPoll { poll_id: 1 };

        let handle_res = handle(&mut deps, creator_env.clone(), msg).unwrap();

        assert_eq!(
            handle_res.attributes,
            vec![
                attr("action", "end_poll"),
                attr("poll_id", "1"),
                attr("rejected_reason", ""),
                attr("passed", "true"),
            ]
        );
        let res = query(&deps, QueryMsg::Poll { poll_id: 1 }).unwrap();
        let value: PollResponse = from_binary(&res).unwrap();
        assert_eq!(PollStatus::Passed, value.status);
    }

    #[test]
    fn end_poll_zero_quorum() {
        let mut deps = mock_dependencies(20, &coins(1000, VOTING_TOKEN));
        mock_init(&mut deps);
        let mut env = mock_env_height(TEST_CREATOR, &coins(2, VOTING_TOKEN), 1000, 10000);

        let msg = create_poll_msg(0, "test".to_string(), None, Some(env.block.height + 1));

        let handle_res = handle(&mut deps, env.clone(), msg).unwrap();
        assert_create_poll_result(1, 0, 1001, 0, TEST_CREATOR, handle_res, &mut deps);
        let msg = HandleMsg::EndPoll { poll_id: 1 };
        env.block.height = &env.block.height + 2;

        let handle_res = handle(&mut deps, env.clone(), msg).unwrap();

        assert_eq!(
            handle_res.attributes,
            vec![
                attr("action", "end_poll"),
                attr("poll_id", "1"),
                attr("rejected_reason", "Quorum not reached"),
                attr("passed", "false"),
            ]
        );

        let res = query(&deps, QueryMsg::Poll { poll_id: 1 }).unwrap();
        let value: PollResponse = from_binary(&res).unwrap();
        assert_eq!(PollStatus::Rejected, value.status);
    }

    #[test]
    fn end_poll_quorum_rejected() {
        let mut deps = mock_dependencies(20, &coins(100, VOTING_TOKEN));
        mock_init(&mut deps);
        let mut creator_env = mock_env(TEST_CREATOR, &coins(2, VOTING_TOKEN));

        let msg = create_poll_msg(
            30,
            "test".to_string(),
            None,
            Some(creator_env.block.height + 1),
        );

        let handle_res = handle(&mut deps, creator_env.clone(), msg.clone()).unwrap();
        assert_eq!(
            handle_res.attributes,
            vec![
                attr("action", "create_poll"),
                attr("creator", TEST_CREATOR),
                attr("poll_id", "1"),
                attr("quorum_percentage", "30"),
                attr("end_height", "12346"),
                attr("start_height", "0"),
            ]
        );

        let msg = HandleMsg::StakeVotingTokens {};
        let stake_amount = 100;
        let env = mock_env(TEST_VOTER, &coins(stake_amount, VOTING_TOKEN));

        let handle_res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
        assert_stake_tokens_result(stake_amount, Some(1), handle_res, &mut deps);

        let msg = HandleMsg::CastVote {
            poll_id: 1,
            vote: "yes".to_string(),
            weight: Uint128::from(10u128),
        };
        let handle_res = handle(&mut deps, env.clone(), msg).unwrap();

        assert_eq!(
            handle_res.attributes,
            vec![
                attr("action", "vote_casted"),
                attr("poll_id", "1"),
                attr("weight", "10"),
                attr("voter", TEST_VOTER),
            ]
        );

        let msg = HandleMsg::EndPoll { poll_id: 1 };

        creator_env.block.height = &creator_env.block.height + 2;

        let handle_res = handle(&mut deps, creator_env.clone(), msg.clone()).unwrap();
        assert_eq!(
            handle_res.attributes,
            vec![
                attr("action", "end_poll"),
                attr("poll_id", "1"),
                attr("rejected_reason", "Quorum not reached"),
                attr("passed", "false"),
            ]
        );

        let res = query(&deps, QueryMsg::Poll { poll_id: 1 }).unwrap();
        let value: PollResponse = from_binary(&res).unwrap();
        assert_eq!(PollStatus::Rejected, value.status);
    }

    #[test]
    fn end_poll_nay_rejected() {
        let voter1_stake = 100;
        let voter2_stake = 1000;
        let mut deps = mock_dependencies(20, &coins(voter1_stake, VOTING_TOKEN));
        mock_init(&mut deps);
        let mut creator_env = mock_env(TEST_CREATOR, &coins(2, VOTING_TOKEN));

        let msg = create_poll_msg(
            10,
            "test".to_string(),
            None,
            Some(creator_env.block.height + 1),
        );

        let handle_res = handle(&mut deps, creator_env.clone(), msg.clone()).unwrap();
        assert_eq!(
            handle_res.attributes,
            vec![
                attr("action", "create_poll"),
                attr("creator", TEST_CREATOR),
                attr("poll_id", "1"),
                attr("quorum_percentage", "10"),
                attr("end_height", "12346"),
                attr("start_height", "0"),
            ]
        );

        let msg = HandleMsg::StakeVotingTokens {};
        let env = mock_env(TEST_VOTER, &coins(voter1_stake, VOTING_TOKEN));

        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
        assert_stake_tokens_result(voter1_stake, Some(1), handle_res, &mut deps);

        let msg = HandleMsg::StakeVotingTokens {};
        let env = mock_env(TEST_VOTER_2, &coins(voter2_stake, VOTING_TOKEN));

        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
        assert_stake_tokens_result(voter1_stake + voter2_stake, Some(1), handle_res, &mut deps);

        let env = mock_env(TEST_VOTER_2, &[]);
        let msg = HandleMsg::CastVote {
            poll_id: 1,
            vote: "no".to_string(),
            weight: Uint128::from(voter2_stake),
        };
        let handle_res = handle(&mut deps, env, msg).unwrap();
        assert_cast_vote_success(TEST_VOTER_2, voter2_stake, 1, handle_res);

        let msg = HandleMsg::EndPoll { poll_id: 1 };

        creator_env.block.height = &creator_env.block.height + 2;
        let handle_res = handle(&mut deps, creator_env.clone(), msg.clone()).unwrap();
        assert_eq!(
            handle_res.attributes,
            vec![
                attr("action", "end_poll"),
                attr("poll_id", "1"),
                attr("rejected_reason", "Threshold not reached"),
                attr("passed", "false"),
            ]
        );

        let res = query(&deps, QueryMsg::Poll { poll_id: 1 }).unwrap();
        let value: PollResponse = from_binary(&res).unwrap();
        assert_eq!(PollStatus::Rejected, value.status);
    }

    #[test]
    fn fails_end_poll_before_start_height() {
        let mut deps = mock_dependencies(20, &[]);
        mock_init(&mut deps);
        let env = mock_env_height(TEST_CREATOR, &coins(2, VOTING_TOKEN), 0, 10000);

        let msg_start_height = 1001;
        let quorum_percentage = 30;
        let msg = create_poll_msg(
            quorum_percentage,
            "test".to_string(),
            Some(msg_start_height),
            None,
        );

        let handle_res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
        assert_create_poll_result(
            1,
            quorum_percentage,
            DEFAULT_END_HEIGHT,
            msg_start_height,
            TEST_CREATOR,
            handle_res,
            &mut deps,
        );
        let msg = HandleMsg::EndPoll { poll_id: 1 };

        let handle_res = handle(&mut deps, env.clone(), msg);

        match handle_res {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::PoolVotingPeriodNotStarted { start_height }) => {
                assert_eq!(start_height, msg_start_height)
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn fails_cast_vote_not_enough_staked() {
        let mut deps = mock_dependencies(20, &[]);
        mock_init(&mut deps);
        let env = mock_env_height(TEST_CREATOR, &coins(2, VOTING_TOKEN), 0, 10000);

        let msg = create_poll_msg(0, "test".to_string(), None, None);

        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
        assert_create_poll_result(
            1,
            0,
            DEFAULT_END_HEIGHT,
            0,
            TEST_CREATOR,
            handle_res,
            &mut deps,
        );

        let env = mock_env(TEST_VOTER, &coins(11, VOTING_TOKEN));
        let msg = HandleMsg::CastVote {
            poll_id: 1,
            vote: "yes".to_string(),
            weight: Uint128::from(1u128),
        };

        let res = handle(&mut deps, env, msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::PollInsufficientStake {}) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn happy_days_cast_vote() {
        let mut deps = mock_dependencies(20, &[]);
        mock_init(&mut deps);

        let env = mock_env_height(TEST_CREATOR, &coins(2, VOTING_TOKEN), 0, 10000);

        let quorum_percentage = 30;

        let msg = create_poll_msg(quorum_percentage, "test".to_string(), None, None);

        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
        assert_create_poll_result(
            1,
            quorum_percentage,
            DEFAULT_END_HEIGHT,
            0,
            TEST_CREATOR,
            handle_res,
            &mut deps,
        );

        let msg = HandleMsg::StakeVotingTokens {};
        let env = mock_env(TEST_VOTER, &coins(11, VOTING_TOKEN));

        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
        assert_stake_tokens_result(11, Some(1), handle_res, &mut deps);

        let env = mock_env(TEST_VOTER, &coins(11, VOTING_TOKEN));
        let weight = 10u128;
        let msg = HandleMsg::CastVote {
            poll_id: 1,
            vote: "yes".to_string(),
            weight: Uint128::from(weight),
        };

        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
        assert_cast_vote_success(TEST_VOTER, weight, 1, handle_res);
    }

    #[test]
    fn happy_days_withdraw_voting_tokens() {
        let mut deps = mock_dependencies(20, &[]);
        mock_init(&mut deps);

        let msg = HandleMsg::StakeVotingTokens {};
        let env = mock_env(TEST_VOTER, &coins(11, VOTING_TOKEN));

        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
        assert_stake_tokens_result(11, None, handle_res, &mut deps);

        let state = config_read(&mut deps.storage).load().unwrap();
        assert_eq!(
            state,
            State {
                denom: String::from(VOTING_TOKEN),
                owner: deps
                    .api
                    .canonical_address(&HumanAddr::from(TEST_CREATOR))
                    .unwrap(),
                poll_count: 0,
                staked_tokens: Uint128::from(11u128),
            }
        );

        let env = mock_env(TEST_VOTER, &coins(11, VOTING_TOKEN));
        let msg = HandleMsg::WithdrawVotingTokens {
            amount: Some(Uint128::from(11u128)),
        };

        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
        let msg = handle_res.messages.get(0).expect("no message");

        assert_eq!(
            msg,
            &CosmosMsg::Bank(BankMsg::Send {
                from_address: HumanAddr::from("cosmos2contract"),
                to_address: HumanAddr::from(TEST_VOTER),
                amount: coins(11, VOTING_TOKEN),
            })
        );

        let state = config_read(&mut deps.storage).load().unwrap();
        assert_eq!(
            state,
            State {
                denom: String::from(VOTING_TOKEN),
                owner: deps
                    .api
                    .canonical_address(&HumanAddr::from(TEST_CREATOR))
                    .unwrap(),
                poll_count: 0,
                staked_tokens: Uint128::zero(),
            }
        );
    }

    #[test]
    fn fails_withdraw_voting_tokens_no_stake() {
        let mut deps = mock_dependencies(20, &[]);
        mock_init(&mut deps);

        let env = mock_env(TEST_VOTER, &coins(11, VOTING_TOKEN));
        let msg = HandleMsg::WithdrawVotingTokens {
            amount: Some(Uint128::from(11u128)),
        };

        let res = handle(&mut deps, env, msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::PollNoStake {}) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn fails_withdraw_too_many_tokens() {
        let mut deps = mock_dependencies(20, &[]);
        mock_init(&mut deps);

        let msg = HandleMsg::StakeVotingTokens {};
        let env = mock_env(TEST_VOTER, &coins(10, VOTING_TOKEN));

        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
        assert_stake_tokens_result(10, None, handle_res, &mut deps);

        let env = mock_env(TEST_VOTER, &[]);
        let msg = HandleMsg::WithdrawVotingTokens {
            amount: Some(Uint128::from(11u128)),
        };

        let res = handle(&mut deps, env, msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::ExcessiveWithdraw { max_amount }) => assert_eq!(max_amount, 10),
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn fails_cast_vote_twice() {
        let mut deps = mock_dependencies(20, &[]);
        mock_init(&mut deps);

        let env = mock_env_height(TEST_CREATOR, &coins(2, VOTING_TOKEN), 0, 10000);

        let quorum_percentage = 30;
        let msg = create_poll_msg(quorum_percentage, "test".to_string(), None, None);
        let handle_res = handle(&mut deps, env.clone(), msg.clone()).unwrap();

        assert_create_poll_result(
            1,
            quorum_percentage,
            DEFAULT_END_HEIGHT,
            0,
            TEST_CREATOR,
            handle_res,
            &mut deps,
        );

        let env = mock_env(TEST_VOTER, &coins(11, VOTING_TOKEN));
        let msg = HandleMsg::StakeVotingTokens {};

        let handle_res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
        assert_stake_tokens_result(11, Some(1), handle_res, &mut deps);

        let weight = 1u128;
        let msg = HandleMsg::CastVote {
            poll_id: 1,
            vote: "yes".to_string(),
            weight: Uint128::from(weight),
        };
        let handle_res = handle(&mut deps, env.clone(), msg).unwrap();
        assert_cast_vote_success(TEST_VOTER, weight, 1, handle_res);

        let msg = HandleMsg::CastVote {
            poll_id: 1,
            vote: "yes".to_string(),
            weight: Uint128::from(weight),
        };
        let res = handle(&mut deps, env.clone(), msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::PollSenderVoted {}) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn fails_cast_vote_without_poll() {
        let mut deps = mock_dependencies(20, &[]);
        mock_init(&mut deps);

        let msg = HandleMsg::CastVote {
            poll_id: 0,
            vote: "yes".to_string(),
            weight: Uint128::from(1u128),
        };
        let env = mock_env(TEST_VOTER, &coins(11, VOTING_TOKEN));

        let res = handle(&mut deps, env, msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::PollNotExist {}) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn happy_days_stake_voting_tokens() {
        let mut deps = mock_dependencies(20, &[]);
        mock_init(&mut deps);

        let msg = HandleMsg::StakeVotingTokens {};

        let env = mock_env(TEST_VOTER, &coins(11, VOTING_TOKEN));

        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
        assert_stake_tokens_result(11, None, handle_res, &mut deps);
    }

    #[test]
    fn fails_insufficient_funds() {
        let mut deps = mock_dependencies(20, &[]);

        // initialize the store
        let msg = init_msg();
        let env = mock_env(TEST_VOTER, &coins(2, VOTING_TOKEN));
        let init_res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, init_res.messages.len());

        // insufficient token
        let msg = HandleMsg::StakeVotingTokens {};
        let env = mock_env(TEST_VOTER, &coins(0, VOTING_TOKEN));

        let res = handle(&mut deps, env, msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::InsufficientFunds {}) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn fails_staking_wrong_token() {
        let mut deps = mock_dependencies(20, &[]);

        // initialize the store
        let msg = init_msg();
        let env = mock_env(TEST_VOTER, &coins(2, VOTING_TOKEN));
        let init_res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, init_res.messages.len());

        // wrong token
        let msg = HandleMsg::StakeVotingTokens {};
        let env = mock_env(TEST_VOTER, &coins(11, "play money"));

        let res = handle(&mut deps, env, msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::InsufficientFunds {}) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    // helper to confirm the expected create_poll response
    fn assert_create_poll_result(
        poll_id: u64,
        quorum: u8,
        end_height: u64,
        start_height: u64,
        creator: &str,
        handle_res: HandleResponse,
        deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
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

        //confirm poll count
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(
            state,
            State {
                denom: String::from(VOTING_TOKEN),
                owner: deps
                    .api
                    .canonical_address(&HumanAddr::from(TEST_CREATOR))
                    .unwrap(),
                poll_count: 1,
                staked_tokens: Uint128::zero(),
            }
        );
    }

    fn assert_stake_tokens_result(
        staked_tokens: u128,
        poll_count: Option<u64>,
        handle_res: HandleResponse,
        deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
    ) {
        assert_eq!(handle_res, HandleResponse::default());

        let state = config_read(&mut deps.storage).load().unwrap();
        assert_eq!(
            state,
            State {
                denom: String::from(VOTING_TOKEN),
                owner: deps
                    .api
                    .canonical_address(&HumanAddr::from(TEST_CREATOR))
                    .unwrap(),
                poll_count: poll_count.unwrap_or_default(),
                staked_tokens: Uint128::from(staked_tokens),
            }
        );
    }

    fn assert_cast_vote_success(
        voter: &str,
        weight: u128,
        poll_id: u64,
        handle_res: HandleResponse,
    ) {
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
}
