use named_type::NamedType;
use named_type_derive::NamedType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::{HandleMsg, InitMsg, QueryMsg};
use cosmwasm::errors::{contract_err, unauthorized, Result};
use cosmwasm::traits::{Api, Extern, Storage};
use cosmwasm::types::{CanonicalAddr, Coin, CosmosMsg, Params, Response};
use cw_storage::{singleton, Singleton};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, NamedType)]
pub struct State {
    pub arbiter: CanonicalAddr,
    pub recipient: CanonicalAddr,
    pub source: CanonicalAddr,
    pub end_height: i64,
    pub end_time: i64,
}

impl State {
    fn is_expired(&self, params: &Params) -> bool {
        (self.end_height != 0 && params.block.height >= self.end_height)
            || (self.end_time != 0 && params.block.time >= self.end_time)
    }
}

pub fn config<S: Storage>(storage: &mut S) -> Singleton<S, State> {
    singleton(storage, b"config")
}

pub fn init<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    params: Params,
    msg: InitMsg,
) -> Result<Response> {
    let state = State {
        arbiter: deps.api.canonical_address(&msg.arbiter)?,
        recipient: deps.api.canonical_address(&msg.recipient)?,
        source: params.message.signer.clone(),
        end_height: msg.end_height,
        end_time: msg.end_time,
    };
    if state.is_expired(&params) {
        contract_err("creating expired escrow")
    } else {
        config(&mut deps.storage).save(&state)?;
        Ok(Response::default())
    }
}

pub fn handle<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    params: Params,
    msg: HandleMsg,
) -> Result<Response> {
    let state = config(&mut deps.storage).load()?;
    match msg {
        HandleMsg::Approve { quantity } => try_approve(&deps.api, params, state, quantity),
        HandleMsg::Refund {} => try_refund(&deps.api, params, state),
    }
}

fn try_approve<A: Api>(
    api: &A,
    params: Params,
    state: State,
    quantity: Option<Vec<Coin>>,
) -> Result<Response> {
    if params.message.signer != state.arbiter {
        unauthorized()
    } else if state.is_expired(&params) {
        contract_err("escrow expired")
    } else {
        #[allow(clippy::or_fun_call)]
        let amount = quantity.unwrap_or(params.contract.balance.unwrap_or_default());
        send_tokens(
            api,
            &params.contract.address,
            &state.recipient,
            amount,
            "paid out funds",
        )
    }
}

fn try_refund<A: Api>(api: &A, params: Params, state: State) -> Result<Response> {
    // anyone can try to refund, as long as the contract is expired
    if !state.is_expired(&params) {
        contract_err("escrow not yet expired")
    } else {
        send_tokens(
            api,
            &params.contract.address,
            &state.source,
            params.contract.balance.unwrap_or_default(),
            "returned funds",
        )
    }
}

// this is a helper to move the tokens, so the business logic is easy to read
fn send_tokens<A: Api>(
    api: &A,
    from_address: &CanonicalAddr,
    to_address: &CanonicalAddr,
    amount: Vec<Coin>,
    log: &str,
) -> Result<Response> {
    let r = Response {
        messages: vec![CosmosMsg::Send {
            from_address: api.human_address(from_address)?,
            to_address: api.human_address(to_address)?,
            amount,
        }],
        log: Some(log.to_string()),
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
    use cosmwasm::mock::{dependencies, mock_params};
    use cosmwasm::traits::Api;
    use cosmwasm::types::coin;

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
        let mut deps = dependencies(20);

        let msg = init_msg(1000, 0);
        let params = mock_params_height(&deps.api, "creator", &coin("1000", "earth"), &[], 876, 0);
        let res = init(&mut deps, params, msg).unwrap();
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
                end_height: 1000,
                end_time: 0,
            }
        );
    }

    #[test]
    fn cannot_initialize_expired() {
        let mut deps = dependencies(20);

        let msg = init_msg(1000, 0);
        let params = mock_params_height(&deps.api, "creator", &coin("1000", "earth"), &[], 1001, 0);
        let res = init(&mut deps, params, msg);
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
            Ok(_) => panic!("expected error"),
            Err(Error::Unauthorized { .. }) => {}
            Err(e) => panic!("unexpected error: {:?}", e),
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
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "escrow expired".to_string()),
            Err(e) => panic!("unexpected error: {:?}", e),
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
        let mut deps = dependencies(20);

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
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => {
                assert_eq!(msg, "escrow not yet expired".to_string())
            }
            Err(e) => panic!("unexpected error: {:?}", e),
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
}
