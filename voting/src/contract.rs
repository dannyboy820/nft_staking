use crate::coin_helpers::assert_sent_sufficient_coin;
use crate::msg::{
    CreatePollResponse, HandleMsg, InitMsg, PollResponse, QueryMsg, TokenStakeResponse,
};
use crate::state::{
    bank, bank_read, config, config_read, poll, poll_read, Poll, PollStatus, State, Voter,
};
use cosmwasm_std::{
    coin, log, to_binary, Api, BankMsg, Binary, CanonicalAddr, Coin, CosmosMsg, Env, Extern,
    HandleResponse, HandleResult, HumanAddr, InitResponse, InitResult, Querier, StdError,
    StdResult, Storage, Uint128,
};

pub const VOTING_TOKEN: &str = "voting_token";
pub const DEFAULT_END_HEIGHT_BLOCKS: &u64 = &100_800_u64;
const MIN_STAKE_AMOUNT: u128 = 1;
const MIN_DESC_LENGTH: usize = 3;
const MAX_DESC_LENGTH: usize = 64;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> InitResult {
    let state = State {
        denom: msg.denom,
        owner: deps.api.canonical_address(&env.message.sender)?,
        poll_count: 0,
        staked_tokens: Uint128::zero(),
    };

    config(&mut deps.storage).save(&state)?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::StakeVotingTokens {} => stake_voting_tokens(deps, env),
        HandleMsg::WithdrawVotingTokens { amount } => withdraw_voting_tokens(deps, env, amount),
        HandleMsg::CastVote {
            poll_id,
            vote,
            weight,
        } => cast_vote(deps, env, poll_id, vote, weight),
        HandleMsg::EndPoll { poll_id } => end_poll(deps, env, poll_id),
        HandleMsg::CreatePoll {
            quorum_percentage,
            description,
            start_height,
            end_height,
        } => create_poll(
            deps,
            env,
            quorum_percentage,
            description,
            start_height,
            end_height,
        ),
    }
}

pub fn stake_voting_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
    let key = &sender_address_raw.as_slice();

    let mut token_manager = bank_read(&deps.storage).may_load(key)?.unwrap_or_default();

    let mut state = config(&mut deps.storage).load()?;

    assert_sent_sufficient_coin(
        &env.message.sent_funds,
        Some(coin(MIN_STAKE_AMOUNT, &state.denom)),
    )?;
    let sent_funds = env
        .message
        .sent_funds
        .iter()
        .find(|coin| coin.denom.eq(&state.denom))
        .unwrap();

    token_manager.token_balance += sent_funds.amount;

    let staked_tokens = state.staked_tokens.u128() + sent_funds.amount.u128();
    state.staked_tokens = Uint128::from(staked_tokens);
    config(&mut deps.storage).save(&state)?;

    bank(&mut deps.storage).save(key, &token_manager)?;

    Ok(HandleResponse::default())
}

// Withdraw amount if not staked. By default all funds will be withdrawn.
pub fn withdraw_voting_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    amount: Option<Uint128>,
) -> HandleResult {
    let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
    let contract_address_raw = deps.api.canonical_address(&env.contract.address)?;
    let key = sender_address_raw.as_slice();

    if let Some(mut token_manager) = bank_read(&deps.storage).may_load(key)? {
        let largest_staked = locked_amount(&sender_address_raw, deps);
        let withdraw_amount = match amount {
            Some(amount) => Some(amount.u128()),
            None => Some(token_manager.token_balance.u128()),
        }
        .unwrap();
        if largest_staked + withdraw_amount > token_manager.token_balance.u128() {
            Err(StdError::generic_err(
                "User is trying to withdraw too many tokens.",
            ))
        } else {
            let balance = token_manager.token_balance.u128() - withdraw_amount;
            token_manager.token_balance = Uint128::from(balance);

            bank(&mut deps.storage).save(key, &token_manager)?;

            let mut state = config(&mut deps.storage).load()?;
            let staked_tokens = state.staked_tokens.u128() - withdraw_amount;
            state.staked_tokens = Uint128::from(staked_tokens);
            config(&mut deps.storage).save(&state)?;

            send_tokens(
                &deps.api,
                &contract_address_raw,
                &sender_address_raw,
                vec![coin(withdraw_amount, &state.denom)],
                "approve",
            )
        }
    } else {
        Err(StdError::generic_err("Nothing staked"))
    }
}

/// validate_description returns an error if the description is invalid
fn validate_description(description: &str) -> StdResult<()> {
    if description.len() < MIN_DESC_LENGTH {
        Err(StdError::generic_err("Description too short"))
    } else if description.len() > MAX_DESC_LENGTH {
        Err(StdError::generic_err("Description too long"))
    } else {
        Ok(())
    }
}

/// validate_quorum_percentage returns an error if the quorum_percentage is invalid
/// (we require 0-100)
fn validate_quorum_percentage(quorum_percentage: Option<u8>) -> StdResult<()> {
    if quorum_percentage.is_some() && quorum_percentage.unwrap() > 100 {
        Err(StdError::generic_err("quorum_percentage must be 0 to 100"))
    } else {
        Ok(())
    }
}

/// validate_end_height returns an error if the poll ends in the past
fn validate_end_height(end_height: Option<u64>, env: Env) -> StdResult<()> {
    if end_height.is_some() && env.block.height >= end_height.unwrap() {
        Err(StdError::generic_err("Poll cannot end in the past"))
    } else {
        Ok(())
    }
}

/// create a new poll
pub fn create_poll<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    quorum_percentage: Option<u8>,
    description: String,
    start_height: Option<u64>,
    end_height: Option<u64>,
) -> StdResult<HandleResponse> {
    validate_quorum_percentage(quorum_percentage)?;
    validate_end_height(end_height, env.clone())?;
    validate_description(&description)?;

    let mut state = config(&mut deps.storage).load()?;
    let poll_count = state.poll_count;
    let poll_id = poll_count + 1;
    state.poll_count = poll_id;

    let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
    let new_poll = Poll {
        creator: sender_address_raw,
        status: PollStatus::InProgress,
        quorum_percentage,
        yes_votes: Uint128::zero(),
        no_votes: Uint128::zero(),
        voters: vec![],
        voter_info: vec![],
        end_height: end_height.unwrap_or(env.block.height + DEFAULT_END_HEIGHT_BLOCKS),
        start_height,
        description,
    };
    let key = state.poll_count.to_string();
    poll(&mut deps.storage).save(key.as_bytes(), &new_poll)?;

    config(&mut deps.storage).save(&state)?;

    let r = HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "create_poll"),
            log(
                "creator",
                deps.api.human_address(&new_poll.creator)?.as_str(),
            ),
            log("poll_id", &poll_id.to_string()),
            log("quorum_percentage", quorum_percentage.unwrap_or(0)),
            log("end_height", new_poll.end_height),
            log("start_height", start_height.unwrap_or(0)),
        ],
        data: Some(to_binary(&CreatePollResponse { poll_id })?),
    };
    Ok(r)
}

/*
 * Ends a poll. Only the creator of a given poll can end that poll.
 */
pub fn end_poll<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    poll_id: u64,
) -> HandleResult {
    let key = &poll_id.to_string();
    let mut a_poll = poll(&mut deps.storage).load(key.as_bytes())?;

    let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
    if a_poll.creator != sender_address_raw {
        return Err(StdError::generic_err(
            "User is not the creator of the poll.",
        ));
    }

    if a_poll.status != PollStatus::InProgress {
        return Err(StdError::generic_err("Poll is not in progress"));
    }

    if a_poll.start_height.is_some() && a_poll.start_height.unwrap() > env.block.height {
        return Err(StdError::generic_err("Voting period has not started."));
    }

    if a_poll.end_height > env.block.height {
        return Err(StdError::generic_err("Voting period has not expired."));
    }

    let mut no = 0u128;
    let mut yes = 0u128;

    for voter in &a_poll.voter_info {
        if voter.vote == "yes" {
            yes += voter.weight.u128();
        } else {
            no += voter.weight.u128();
        }
    }
    let tallied_weight = yes + no;

    let mut rejected_reason = "";
    let mut passed = false;

    if tallied_weight > 0 {
        let state = config_read(&deps.storage).load()?;

        let staked_weight = deps
            .querier
            .query_balance(&env.contract.address, &state.denom)
            .unwrap()
            .amount
            .u128();

        if staked_weight == 0 {
            return Err(StdError::generic_err("Nothing staked"));
        }

        let quorum = ((tallied_weight / staked_weight) * 100) as u8;
        if a_poll.quorum_percentage.is_some() && quorum < a_poll.quorum_percentage.unwrap() {
            // Quorum: More than quorum_percentage of the total staked tokens at the end of the voting
            // period need to have participated in the vote.
            rejected_reason = "Quorum not reached";
        } else if yes > tallied_weight / 2 {
            //Threshold: More than 50% of the tokens that participated in the vote
            // (after excluding “Abstain” votes) need to have voted in favor of the proposal (“Yes”).
            a_poll.status = PollStatus::Passed;
            passed = true;
        } else {
            rejected_reason = "Threshold not reached";
        }
    } else {
        rejected_reason = "Quorum not reached";
    }
    if !passed {
        a_poll.status = PollStatus::Rejected
    }
    poll(&mut deps.storage).save(key.as_bytes(), &a_poll)?;

    for voter in &a_poll.voters {
        unlock_tokens(deps, voter, poll_id)?;
    }

    let log = vec![
        log("action", "end_poll"),
        log("poll_id", &poll_id.to_string()),
        log("rejected_reason", rejected_reason),
        log("passed", &passed.to_string()),
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

// unlock voter's tokens in a given poll
fn unlock_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    voter: &CanonicalAddr,
    poll_id: u64,
) -> HandleResult {
    let voter_key = &voter.as_slice();
    let mut token_manager = bank_read(&deps.storage).load(voter_key).unwrap();

    // unlock entails removing the mapped poll_id, retaining the rest
    token_manager.locked_tokens.retain(|(k, _)| k != &poll_id);
    bank(&mut deps.storage).save(voter_key, &token_manager)?;
    Ok(HandleResponse::default())
}

// finds the largest locked amount in participated polls.
fn locked_amount<S: Storage, A: Api, Q: Querier>(
    voter: &CanonicalAddr,
    deps: &mut Extern<S, A, Q>,
) -> u128 {
    let voter_key = &voter.as_slice();
    let token_manager = bank_read(&deps.storage).load(voter_key).unwrap();
    token_manager
        .locked_tokens
        .iter()
        .map(|(_, v)| v.u128())
        .max()
        .unwrap_or_default()
}

fn has_voted(voter: &CanonicalAddr, a_poll: &Poll) -> bool {
    a_poll.voters.iter().any(|i| i == voter)
}

pub fn cast_vote<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    poll_id: u64,
    vote: String,
    weight: Uint128,
) -> HandleResult {
    let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
    let poll_key = &poll_id.to_string();
    let state = config_read(&deps.storage).load()?;
    if poll_id == 0 || state.poll_count > poll_id {
        return Err(StdError::generic_err("Poll does not exist"));
    }

    let mut a_poll = poll(&mut deps.storage).load(poll_key.as_bytes())?;

    if a_poll.status != PollStatus::InProgress {
        return Err(StdError::generic_err("Poll is not in progress"));
    }

    if has_voted(&sender_address_raw, &a_poll) {
        return Err(StdError::generic_err("User has already voted."));
    }

    let key = &sender_address_raw.as_slice();
    let mut token_manager = bank_read(&deps.storage).may_load(key)?.unwrap_or_default();

    if token_manager.token_balance < weight {
        return Err(StdError::generic_err(
            "User does not have enough staked tokens.",
        ));
    }
    token_manager.participated_polls.push(poll_id);
    token_manager.locked_tokens.push((poll_id, weight));
    bank(&mut deps.storage).save(key, &token_manager)?;

    a_poll.voters.push(sender_address_raw.clone());

    let voter_info = Voter { vote, weight };

    a_poll.voter_info.push(voter_info);
    poll(&mut deps.storage).save(poll_key.as_bytes(), &a_poll)?;

    let log = vec![
        log("action", "vote_casted"),
        log("poll_id", &poll_id.to_string()),
        log("weight", &weight.to_string()),
        log("voter", &env.message.sender.as_str()),
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

fn send_tokens<A: Api>(
    api: &A,
    from_address: &CanonicalAddr,
    to_address: &CanonicalAddr,
    amount: Vec<Coin>,
    action: &str,
) -> HandleResult {
    let from_human = api.human_address(from_address)?;
    let to_human = api.human_address(to_address)?;
    let log = vec![log("action", action), log("to", to_human.as_str())];

    let r = HandleResponse {
        messages: vec![CosmosMsg::Bank(BankMsg::Send {
            from_address: from_human,
            to_address: to_human,
            amount,
        })],
        log,
        data: None,
    };
    Ok(r)
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    _deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&config_read(&_deps.storage).load()?),

        QueryMsg::TokenStake { address } => token_balance(_deps, address),
        QueryMsg::Poll { poll_id } => query_poll(_deps, poll_id),
    }
}

fn query_poll<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    poll_id: u64,
) -> StdResult<Binary> {
    let key = &poll_id.to_string();

    let poll = match poll_read(&deps.storage).may_load(key.as_bytes())? {
        Some(poll) => Some(poll),
        None => return Err(StdError::generic_err("Poll does not exist")),
    }
    .unwrap();

    let resp = PollResponse {
        creator: deps.api.human_address(&poll.creator).unwrap(),
        status: poll.status,
        quorum_percentage: poll.quorum_percentage,
        end_height: Some(poll.end_height),
        start_height: poll.start_height,
        description: poll.description,
    };
    to_binary(&resp)
}

fn token_balance<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
) -> StdResult<Binary> {
    let key = deps.api.canonical_address(&address).unwrap();

    let token_manager = bank_read(&deps.storage)
        .may_load(key.as_slice())?
        .unwrap_or_default();

    let resp = TokenStakeResponse {
        token_balance: token_manager.token_balance,
    };

    to_binary(&resp)
}
