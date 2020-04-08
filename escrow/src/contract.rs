use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::{HandleMsg, InitMsg, QueryMsg};
use cosmwasm::errors::{contract_err, unauthorized, Result};
use cosmwasm::traits::{Api, Extern, Storage};
use cosmwasm::types::{log, CanonicalAddr, Coin, CosmosMsg, Env, Response};
use cw_storage::{singleton, Singleton};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub arbiter: CanonicalAddr,
    pub recipient: CanonicalAddr,
    pub source: CanonicalAddr,
    pub end_height: Option<i64>,
    pub end_time: Option<i64>,
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

        return false;
    }
}

pub fn config<S: Storage>(storage: &mut S) -> Singleton<S, State> {
    singleton(storage, b"config")
}

pub fn init<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    env: Env,
    msg: InitMsg,
) -> Result<Response> {
    let state = State {
        arbiter: deps.api.canonical_address(&msg.arbiter)?,
        recipient: deps.api.canonical_address(&msg.recipient)?,
        source: env.message.signer.clone(),
        end_height: msg.end_height,
        end_time: msg.end_time,
    };
    if state.is_expired(&env) {
        contract_err("creating expired escrow")
    } else {
        config(&mut deps.storage).save(&state)?;
        Ok(Response::default())
    }
}

pub fn handle<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    env: Env,
    msg: HandleMsg,
) -> Result<Response> {
    let state = config(&mut deps.storage).load()?;
    match msg {
        HandleMsg::Approve { quantity } => try_approve(&deps.api, env, state, quantity),
        HandleMsg::Refund {} => try_refund(&deps.api, env, state),
    }
}

fn try_approve<A: Api>(
    api: &A,
    env: Env,
    state: State,
    quantity: Option<Vec<Coin>>,
) -> Result<Response> {
    if env.message.signer != state.arbiter {
        unauthorized()
    } else if state.is_expired(&env) {
        contract_err("escrow expired")
    } else {
        #[allow(clippy::or_fun_call)]
        let amount = quantity.unwrap_or(env.contract.balance.unwrap_or_default());
        send_tokens(
            api,
            &env.contract.address,
            &state.recipient,
            amount,
            "approve",
        )
    }
}

fn try_refund<A: Api>(api: &A, env: Env, state: State) -> Result<Response> {
    // anyone can try to refund, as long as the contract is expired
    if !state.is_expired(&env) {
        contract_err("escrow not yet expired")
    } else {
        send_tokens(
            api,
            &env.contract.address,
            &state.source,
            env.contract.balance.unwrap_or_default(),
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
) -> Result<Response> {
    let from_human = api.human_address(from_address)?;
    let to_human = api.human_address(to_address)?;
    let log = vec![log("action", action), log("to", to_human.as_str())];

    let r = Response {
        messages: vec![CosmosMsg::Send {
            from_address: from_human,
            to_address: to_human,
            amount,
        }],
        log: log,
        data: None,
    };
    Ok(r)
}

pub fn query<S: Storage, A: Api>(_deps: &Extern<S, A>, msg: QueryMsg) -> Result<Vec<u8>> {
    // this always returns error
    match msg {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm::errors::Error;
    use cosmwasm::mock::{dependencies, mock_env};
    use cosmwasm::traits::Api;
    use cosmwasm::types::{coin, HumanAddr};

    fn init_msg_expire_by_height(height: i64) -> InitMsg {
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
        balance: &[Coin],
        height: i64,
        time: i64,
    ) -> Env {
        let mut env = mock_env(api, signer, sent, balance);
        env.block.height = height;
        env.block.time = time;
        env
    }

    #[test]
    fn proper_initialization() {
        let mut deps = dependencies(20);

        let msg = init_msg_expire_by_height(1000);
        let env = mock_env_height(&deps.api, "creator", &coin("1000", "earth"), &[], 876, 0);
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
        let mut deps = dependencies(20);

        let msg = init_msg_expire_by_height(1000);
        let env = mock_env_height(&deps.api, "creator", &coin("1000", "earth"), &[], 1001, 0);
        let res = init(&mut deps, env, msg);
        assert!(res.is_err());
        if let Err(Error::ContractErr { msg, .. }) = res {
            assert_eq!(msg, "creating expired escrow".to_string());
        } else {
            assert!(false, "wrong error type");
        }
    }

    #[test]
    fn handle_approve() {
        let mut deps = dependencies(20);

        // initialize the store
        let msg = init_msg_expire_by_height(1000);
        let env = mock_env_height(&deps.api, "creator", &coin("1000", "earth"), &[], 876, 0);
        let init_res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, init_res.messages.len());

        // beneficiary cannot release it
        let msg = HandleMsg::Approve { quantity: None };
        let env = mock_env_height(
            &deps.api,
            "beneficiary",
            &coin("0", "earth"),
            &coin("1000", "earth"),
            900,
            0,
        );
        let handle_res = handle(&mut deps, env, msg.clone());
        match handle_res {
            Ok(_) => panic!("expected error"),
            Err(Error::Unauthorized { .. }) => {}
            Err(e) => panic!("unexpected error: {:?}", e),
        }

        // verifier cannot release it when expired
        let env = mock_env_height(
            &deps.api,
            "verifies",
            &coin("0", "earth"),
            &coin("1000", "earth"),
            1100,
            0,
        );
        let handle_res = handle(&mut deps, env, msg.clone());
        match handle_res {
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "escrow expired".to_string()),
            Err(e) => panic!("unexpected error: {:?}", e),
        }

        // complete release by verfier, before expiration
        let env = mock_env_height(
            &deps.api,
            "verifies",
            &coin("0", "earth"),
            &coin("1000", "earth"),
            999,
            0,
        );
        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
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
        let env = mock_env_height(
            &deps.api,
            "verifies",
            &coin("0", "earth"),
            &coin("1000", "earth"),
            999,
            0,
        );
        let handle_res = handle(&mut deps, env, partial_msg).unwrap();
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
        let mut deps = dependencies(20);

        // initialize the store
        let msg = init_msg_expire_by_height(1000);
        let env = mock_env_height(&deps.api, "creator", &coin("1000", "earth"), &[], 876, 0);
        let init_res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, init_res.messages.len());

        // cannot release when unexpired (height < end_height)
        let msg = HandleMsg::Refund {};
        let env = mock_env_height(
            &deps.api,
            "anybody",
            &coin("0", "earth"),
            &coin("1000", "earth"),
            800,
            0,
        );
        let handle_res = handle(&mut deps, env, msg.clone());
        match handle_res {
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => {
                assert_eq!(msg, "escrow not yet expired".to_string())
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }

        // cannot release when unexpired (height == end_height)
        let msg = HandleMsg::Refund {};
        let env = mock_env_height(
            &deps.api,
            "anybody",
            &coin("0", "earth"),
            &coin("1000", "earth"),
            1000,
            0,
        );
        let handle_res = handle(&mut deps, env, msg.clone());
        match handle_res {
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => {
                assert_eq!(msg, "escrow not yet expired".to_string())
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }

        // anyone can release after expiration
        let env = mock_env_height(
            &deps.api,
            "anybody",
            &coin("0", "earth"),
            &coin("1000", "earth"),
            1001,
            0,
        );
        let handle_res = handle(&mut deps, env, msg.clone()).unwrap();
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
}
