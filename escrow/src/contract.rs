use cosmwasm_std::{
    attr, to_binary, Api, BankMsg, Binary, Coin, CosmosMsg, Env, Extern, HandleResponse, HumanAddr,
    InitResponse, MessageInfo, Querier, StdResult, Storage,
};

use crate::error::ContractError;
use crate::msg::{ArbiterResponse, HandleMsg, InitMsg, QueryMsg};
use crate::state::{config, config_read, State};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    info: MessageInfo,
    msg: InitMsg,
) -> Result<InitResponse, ContractError> {
    let state = State {
        arbiter: deps.api.canonical_address(&msg.arbiter)?,
        recipient: deps.api.canonical_address(&msg.recipient)?,
        source: deps.api.canonical_address(&info.sender)?,
        end_height: msg.end_height,
        end_time: msg.end_time,
    };

    if state.is_expired(&env) {
        return Err(ContractError::Expired {
            end_height: msg.end_height,
            end_time: msg.end_time,
        });
    }

    config(&mut deps.storage).save(&state)?;
    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    info: MessageInfo,
    msg: HandleMsg,
) -> Result<HandleResponse, ContractError> {
    let state = config_read(&deps.storage).load()?;
    match msg {
        HandleMsg::Approve { quantity } => try_approve(deps, env, state, info, quantity),
        HandleMsg::Refund {} => try_refund(deps, env, info, state),
    }
}

fn try_approve<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    state: State,
    info: MessageInfo,
    quantity: Option<Vec<Coin>>,
) -> Result<HandleResponse, ContractError> {
    if deps.api.canonical_address(&info.sender)? != state.arbiter {
        return Err(ContractError::Unauthorized {});
    }

    // throws error if state is expired
    if state.is_expired(&env) {
        return Err(ContractError::Expired {
            end_height: state.end_height,
            end_time: state.end_time,
        });
    }

    let amount = if let Some(quantity) = quantity {
        quantity
    } else {
        // release everything

        // Querier guarantees to returns up-to-date data, including funds sent in this handle message
        // https://github.com/CosmWasm/wasmd/blob/master/x/wasm/internal/keeper/keeper.go#L185-L192
        deps.querier.query_all_balances(&env.contract.address)?
    };

    send_tokens(
        env.contract.address,
        deps.api.human_address(&state.recipient)?,
        amount,
        "approve",
    )
}

fn try_refund<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    _info: MessageInfo,
    state: State,
) -> Result<HandleResponse, ContractError> {
    // anyone can try to refund, as long as the contract is expired
    if !state.is_expired(&env) {
        return Err(ContractError::NotExpired {});
    }

    // Querier guarantees to returns up-to-date data, including funds sent in this handle message
    // https://github.com/CosmWasm/wasmd/blob/master/x/wasm/internal/keeper/keeper.go#L185-L192
    let balance = deps.querier.query_all_balances(&env.contract.address)?;
    send_tokens(
        env.contract.address,
        deps.api.human_address(&state.source)?,
        balance,
        "refund",
    )
}

// this is a helper to move the tokens, so the business logic is easy to read
fn send_tokens(
    from_address: HumanAddr,
    to_address: HumanAddr,
    amount: Vec<Coin>,
    action: &str,
) -> Result<HandleResponse, ContractError> {
    let attributes = vec![attr("action", action), attr("to", to_address.clone())];

    let r = HandleResponse {
        messages: vec![CosmosMsg::Bank(BankMsg::Send {
            from_address,
            to_address,
            amount,
        })],
        data: None,
        attributes,
    };
    Ok(r)
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    _env: Env,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Arbiter {} => to_binary(&query_arbiter(deps)?),
    }
}

fn query_arbiter<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ArbiterResponse> {
    let state = config_read(&deps.storage).load()?;
    let addr = deps.api.human_address(&state.arbiter)?;
    Ok(ArbiterResponse { arbiter: addr })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, Api, HumanAddr};

    fn init_msg_expire_by_height(height: u64) -> InitMsg {
        InitMsg {
            arbiter: HumanAddr::from("verifies"),
            recipient: HumanAddr::from("benefits"),
            end_height: Some(height),
            end_time: None,
        }
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(&[]);

        let msg = init_msg_expire_by_height(1000);
        let mut env = mock_env();
        env.block.height = 876;
        env.block.time = 0;
        let info = mock_info("creator", &coins(1000, "earth"));

        let res = init(&mut deps, env, info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let state = config_read(&mut deps.storage).load().unwrap();
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
                end_height: Some(1000),
                end_time: None,
            }
        );
    }

    #[test]
    fn cannot_initialize_expired() {
        let mut deps = mock_dependencies(&[]);

        let msg = init_msg_expire_by_height(1000);
        let mut env = mock_env();
        env.block.height = 1001;
        env.block.time = 0;
        let info = mock_info("creator", &coins(1000, "earth"));

        let res = init(&mut deps, env, info, msg);
        match res.unwrap_err() {
            ContractError::Expired { .. } => {}
            e => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn init_and_query() {
        let mut deps = mock_dependencies(&[]);

        let arbiter = HumanAddr::from("arbiters");
        let recipient = HumanAddr::from("receives");
        let creator = HumanAddr::from("creates");
        let msg = InitMsg {
            arbiter: arbiter.clone(),
            recipient,
            end_height: None,
            end_time: None,
        };
        let mut env = mock_env();
        env.block.height = 876;
        env.block.time = 0;
        let info = mock_info(creator, &[]);
        let res = init(&mut deps, env, info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // now let's query
        let query_response = query_arbiter(&deps).unwrap();
        assert_eq!(query_response.arbiter, arbiter);
    }

    #[test]
    fn handle_approve() {
        let mut deps = mock_dependencies(&[]);

        // initialize the store
        let init_amount = coins(1000, "earth");
        let msg = init_msg_expire_by_height(1000);
        let mut env = mock_env();
        env.block.height = 876;
        env.block.time = 0;
        let info = mock_info("creator", &init_amount);
        let contract_addr = env.clone().contract.address;
        let init_res = init(&mut deps, env, info, msg).unwrap();
        assert_eq!(0, init_res.messages.len());

        // balance changed in init
        deps.querier.update_balance(&contract_addr, init_amount);

        // beneficiary cannot release it
        let msg = HandleMsg::Approve { quantity: None };
        let mut env = mock_env();
        env.block.height = 900;
        env.block.time = 0;
        let info = mock_info("beneficiary", &[]);
        let handle_res = handle(&mut deps, env, info, msg.clone());
        match handle_res.unwrap_err() {
            ContractError::Unauthorized { .. } => {}
            e => panic!("unexpected error: {:?}", e),
        }

        // verifier cannot release it when expired
        let mut env = mock_env();
        env.block.height = 1100;
        env.block.time = 0;
        let info = mock_info("verifies", &[]);
        let handle_res = handle(&mut deps, env, info, msg.clone());
        match handle_res.unwrap_err() {
            ContractError::Expired { .. } => {}
            e => panic!("unexpected error: {:?}", e),
        }

        // complete release by verfier, before expiration
        let mut env = mock_env();
        env.block.height = 999;
        env.block.time = 0;
        let info = mock_info("verifies", &[]);
        let handle_res = handle(&mut deps, env, info, msg.clone()).unwrap();
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
        let mut env = mock_env();
        env.block.height = 999;
        env.block.time = 0;
        let info = mock_info("verifies", &[]);
        let handle_res = handle(&mut deps, env, info, partial_msg).unwrap();
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
        let mut deps = mock_dependencies(&[]);

        // initialize the store
        let init_amount = coins(1000, "earth");
        let msg = init_msg_expire_by_height(1000);
        let mut env = mock_env();
        env.block.height = 876;
        env.block.time = 0;
        let info = mock_info("creator", &init_amount);
        let contract_addr = env.clone().contract.address;
        let init_res = init(&mut deps, env, info, msg).unwrap();
        assert_eq!(0, init_res.messages.len());

        // balance changed in init
        deps.querier.update_balance(&contract_addr, init_amount);

        // cannot release when unexpired (height < end_height)
        let msg = HandleMsg::Refund {};
        let mut env = mock_env();
        env.block.height = 800;
        env.block.time = 0;
        let info = mock_info("anybody", &[]);
        let handle_res = handle(&mut deps, env, info, msg.clone());
        match handle_res.unwrap_err() {
            ContractError::NotExpired { .. } => {}
            e => panic!("unexpected error: {:?}", e),
        }

        // cannot release when unexpired (height == end_height)
        let msg = HandleMsg::Refund {};
        let mut env = mock_env();
        env.block.height = 1000;
        env.block.time = 0;
        let info = mock_info("anybody", &[]);
        let handle_res = handle(&mut deps, env, info, msg.clone());
        match handle_res.unwrap_err() {
            ContractError::NotExpired { .. } => {}
            e => panic!("unexpected error: {:?}", e),
        }

        // anyone can release after expiration
        let mut env = mock_env();
        env.block.height = 1001;
        env.block.time = 0;
        let info = mock_info("anybody", &[]);
        let handle_res = handle(&mut deps, env, info, msg.clone()).unwrap();
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
}
