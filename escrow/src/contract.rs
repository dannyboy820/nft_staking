use cosmwasm_std::{
    generic_err, log, unauthorized, Api, BankMsg, Binary, CanonicalAddr, Coin, CosmosMsg, Env,
    Extern, HandleResponse, HandleResult, InitResponse, InitResult, Querier, StdResult, Storage,
};
use cosmwasm_storage::{singleton, Singleton};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::{HandleMsg, InitMsg, QueryMsg};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub arbiter: CanonicalAddr,
    pub recipient: CanonicalAddr,
    pub source: CanonicalAddr,
    pub end_height: Option<u64>,
    pub end_time: Option<u64>,
}

impl State {
    fn is_expired(&self, env: &Env) -> bool {
        if let Some(end_height) = self.end_height {
            if env.block.height > end_height {
                return true;
            }
        }

        if let Some(end_time) = self.end_time {
            if env.block.time > end_time {
                return true;
            }
        }

        false
    }
}

pub fn config<S: Storage>(storage: &mut S) -> Singleton<S, State> {
    singleton(storage, b"config")
}

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> InitResult {
    let state = State {
        arbiter: deps.api.canonical_address(&msg.arbiter)?,
        recipient: deps.api.canonical_address(&msg.recipient)?,
        source: env.message.sender.clone(),
        end_height: msg.end_height,
        end_time: msg.end_time,
    };
    if state.is_expired(&env) {
        Err(generic_err("creating expired escrow"))
    } else {
        config(&mut deps.storage).save(&state)?;
        Ok(InitResponse::default())
    }
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> HandleResult {
    let state = config(&mut deps.storage).load()?;
    match msg {
        HandleMsg::Approve { quantity } => try_approve(deps, env, state, quantity),
        HandleMsg::Refund {} => try_refund(deps, env, state),
    }
}

fn try_approve<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    state: State,
    quantity: Option<Vec<Coin>>,
) -> HandleResult {
    if env.message.sender != state.arbiter {
        Err(unauthorized())
    } else if state.is_expired(&env) {
        Err(generic_err("escrow expired"))
    } else {
        let amount = if let Some(quantity) = quantity {
            quantity
        } else {
            // release everything

            // TODO: avoid the necessarity of this conversion
            let human = deps.api.human_address(&env.contract.address)?;
            // TODO: make sure querier guarantees to returns up-to-date data
            deps.querier.query_all_balances(human)?
        };

        send_tokens(
            &deps.api,
            &env.contract.address,
            &state.recipient,
            amount,
            "approve",
        )
    }
}

fn try_refund<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    state: State,
) -> HandleResult {
    // anyone can try to refund, as long as the contract is expired
    if !state.is_expired(&env) {
        Err(generic_err("escrow not yet expired"))
    } else {
        // TODO: avoid the necessarity of this conversion
        let human = deps.api.human_address(&env.contract.address)?;
        // TODO: make sure querier guarantees to returns up-to-date data
        let balance = deps.querier.query_all_balances(human)?;
        send_tokens(
            &deps.api,
            &env.contract.address,
            &state.source,
            balance,
            "refund",
        )
    }
}

// this is a helper to move the tokens, so the business logic is easy to read
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
    // this always returns error
    match msg {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::{coins, Api, HumanAddr, StdError};

    fn init_msg_expire_by_height(height: u64) -> InitMsg {
        InitMsg {
            arbiter: HumanAddr::from("verifies"),
            recipient: HumanAddr::from("benefits"),
            end_height: Some(height),
            end_time: None,
        }
    }

    fn mock_env_height<A: Api>(
        api: &A,
        signer: &str,
        sent: &[Coin],
        height: u64,
        time: u64,
    ) -> Env {
        let mut env = mock_env(api, signer, sent);
        env.block.height = height;
        env.block.time = time;
        env
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = init_msg_expire_by_height(1000);
        let env = mock_env_height(&deps.api, "creator", &coins(1000, "earth"), 876, 0);
        let res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let state = config(&mut deps.storage).load().unwrap();
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
        let mut deps = mock_dependencies(20, &[]);

        let msg = init_msg_expire_by_height(1000);
        let env = mock_env_height(&deps.api, "creator", &coins(1000, "earth"), 1001, 0);
        let res = init(&mut deps, env, msg);
        assert!(res.is_err());
        if let Err(StdError::GenericErr { msg, .. }) = res {
            assert_eq!(msg, "creating expired escrow".to_string());
        } else {
            assert!(false, "wrong error type");
        }
    }

    #[test]
    fn handle_approve() {
        let mut deps = mock_dependencies(20, &[]);

        // TODO: how to test chaning contract balance?

        // initialize the store
        let msg = init_msg_expire_by_height(1000);
        let env = mock_env_height(&deps.api, "creator", &coins(1000, "earth"), 876, 0);
        let init_res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, init_res.messages.len());

        // beneficiary cannot release it
        let msg = HandleMsg::Approve { quantity: None };
        let env = mock_env_height(
            &deps.api,
            "beneficiary",
            &coins(0, "earth"),
            // &coins(1000, "earth"),
            900,
            0,
        );
        let handle_res = handle(&mut deps, env, msg.clone());
        match handle_res {
            Ok(_) => panic!("expected error"),
            Err(StdError::Unauthorized { .. }) => {}
            Err(e) => panic!("unexpected error: {:?}", e),
        }

        // verifier cannot release it when expired
        let env = mock_env_height(
            &deps.api,
            "verifies",
            &coins(0, "earth"),
            // &coins(1000, "earth"),
            1100,
            0,
        );
        let handle_res = handle(&mut deps, env, msg.clone());
        match handle_res {
            Ok(_) => panic!("expected error"),
            Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "escrow expired"),
            Err(e) => panic!("unexpected error: {:?}", e),
        }

        // complete release by verfier, before expiration
        let env = mock_env_height(
            &deps.api,
            "verifies",
            &coins(0, "earth"),
            // &coins(1000, "earth"),
            999,
            0,
        );
        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
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
        let env = mock_env_height(
            &deps.api,
            "verifies",
            &coins(0, "earth"),
            // &coins(1000, "earth"),
            999,
            0,
        );
        let handle_res = handle(&mut deps, env, partial_msg).unwrap();
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
        let mut deps = mock_dependencies(20, &[]);
        // TODO: how to mock changing contract balance?

        // initialize the store
        let msg = init_msg_expire_by_height(1000);
        let env = mock_env_height(&deps.api, "creator", &coins(1000, "earth"), 876, 0);
        let init_res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, init_res.messages.len());

        // cannot release when unexpired (height < end_height)
        let msg = HandleMsg::Refund {};
        let env = mock_env_height(
            &deps.api,
            "anybody",
            &coins(0, "earth"),
            // &coins(1000, "earth"),
            800,
            0,
        );
        let handle_res = handle(&mut deps, env, msg.clone());
        match handle_res {
            Ok(_) => panic!("expected error"),
            Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "escrow not yet expired"),
            Err(e) => panic!("unexpected error: {:?}", e),
        }

        // cannot release when unexpired (height == end_height)
        let msg = HandleMsg::Refund {};
        let env = mock_env_height(
            &deps.api,
            "anybody",
            &coins(0, "earth"),
            // &coins(1000, "earth"),
            1000,
            0,
        );
        let handle_res = handle(&mut deps, env, msg.clone());
        match handle_res {
            Ok(_) => panic!("expected error"),
            Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "escrow not yet expired"),
            Err(e) => panic!("unexpected error: {:?}", e),
        }

        // anyone can release after expiration
        let env = mock_env_height(
            &deps.api,
            "anybody",
            &coins(0, "earth"),
            // &coins(1000, "earth"),
            1001,
            0,
        );
        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
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
