use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{OptionExt, ResultExt};

use cosmwasm::errors::{ContractErr, ParseErr, Result, SerializeErr, Unauthorized};
use cosmwasm::serde::{from_slice, to_vec};
use cosmwasm::traits::{Api, Extern, Storage};
use cosmwasm::types::{CanonicalAddr, Coin, CosmosMsg, HumanAddr, Params, Response};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub arbiter: HumanAddr,
    pub recipient: HumanAddr,
    // you can set a last time or block height the contract is valid at
    // if *either* is non-zero and below current state, the contract is considered expired
    // and will be returned to the original funder
    pub end_height: i64,
    pub end_time: i64,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HandleMsg {
    Approve {
        // release some coins - if quantity is None, release all coins in balance
        quantity: Option<Vec<Coin>>,
    },
    Refund {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum QueryMsg {
//    // GetCount returns the current count as a json-encoded number
//    GetCount {},
}

//// We define a custom struct for each query response
//#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
//pub struct CountResponse {
//    pub count: i32,
//}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
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

pub static CONFIG_KEY: &[u8] = b"config";

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
        ContractErr {
            msg: "creating expired escrow",
        }
            .fail()
    } else {
        deps.storage.set(
            CONFIG_KEY,
            &to_vec(&state).context(SerializeErr { kind: "State" })?,
        );
        Ok(Response::default())
    }
}

pub fn handle<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    params: Params,
    msg: HandleMsg,
) -> Result<Response> {
    let data = deps.storage.get(CONFIG_KEY).context(ContractErr {
        msg: "uninitialized data",
    })?;
    let state: State = from_slice(&data).context(ParseErr { kind: "State" })?;

    match msg {
        HandleMsg::Approve { quantity } => try_approve(&deps.api, params, state, quantity),
        HandleMsg::Refund {} => try_refund(&deps.api, params, state),
    }
}

fn try_approve<A: Api>(api: &A, params: Params, state: State, quantity: Option<Vec<Coin>>) -> Result<Response> {
    if params.message.signer != state.arbiter {
        Unauthorized {}.fail()
    } else if state.is_expired(&params) {
        ContractErr {
            msg: "escrow expired",
        }
            .fail()
    } else {
        let amount = match quantity {
            None => params.contract.balance.unwrap_or_default(),
            Some(coins) => coins,
        };
        let res = Response {
            messages: vec![CosmosMsg::Send {
                from_address: api.human_address(&params.contract.address)?,
                to_address: api.human_address(&state.recipient)?,
                amount,
            }],
            log: Some("paid out funds".to_string()),
            data: None,
        };
        Ok(res)
    }
}

fn try_refund<A: Api>(api: &A, params: Params, state: State) -> Result<Response> {
    // anyone can try to refund, as long as the contract is expired
    if !state.is_expired(&params) {
        ContractErr {
            msg: "escrow not yet expired",
        }
            .fail()
    } else {
        let res = Response {
            messages: vec![CosmosMsg::Send {
                from_address: api.human_address(&params.contract.address)?,
                to_address: api.human_address(&state.source)?,
                amount: params.contract.balance.unwrap_or_default(),
            }],
            log: Some("returned funds".to_string()),
            data: None,
        };
        Ok(res)
    }
}

pub fn query<S: Storage, A: Api>(deps: &Extern<S, A>, msg: QueryMsg) -> Result<Vec<u8>> {
    match msg {
//        QueryMsg::GetCount {} => query_count(deps),
    }
}

//fn query_count<S: Storage, A: Api>(deps: &Extern<S, A>) -> Result<Vec<u8>> {
//    let data = deps.storage.get(CONFIG_KEY).context(ContractErr {
//        msg: "uninitialized data",
//    })?;
//    let state: State = from_slice(&data).context(ParseErr { kind: "State" })?;
//    let resp = CountResponse { count: state.count };
//    to_vec(&resp).context(SerializeErr {
//        kind: "CountResponse",
//    })
//}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm::errors::Error;
    use cosmwasm::mock::{dependencies, mock_params};
    use cosmwasm::types::coin;

    #[test]
    fn proper_initialization() {
        let mut deps = dependencies(20);

        let msg = InitMsg { count: 17 };
        let params = mock_params(&deps.api, "creator", &coin("1000", "earth"), &[]);

        // we can just call .unwrap() to assert this was a success
        let res = init(&mut deps, params, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(&deps, QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_slice(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = dependencies(20);

        let msg = InitMsg { count: 17 };
        let params = mock_params(
            &deps.api,
            "creator",
            &coin("2", "token"),
            &coin("2", "token"),
        );
        let _res = init(&mut deps, params, msg).unwrap();

        // beneficiary can release it
        let params = mock_params(&deps.api, "anyone", &coin("2", "token"), &[]);
        let msg = HandleMsg::Increment {};
        let _res = handle(&mut deps, params, msg).unwrap();

        // should increase counter by 1
        let res = query(&deps, QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_slice(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = dependencies(20);

        let msg = InitMsg { count: 17 };
        let params = mock_params(
            &deps.api,
            "creator",
            &coin("2", "token"),
            &coin("2", "token"),
        );
        let _res = init(&mut deps, params, msg).unwrap();

        // beneficiary can release it
        let unauth_params = mock_params(&deps.api, "anyone", &coin("2", "token"), &[]);
        let msg = HandleMsg::Reset { count: 5 };
        let res = handle(&mut deps, unauth_params, msg);
        match res {
            Err(Error::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_params = mock_params(&deps.api, "creator", &coin("2", "token"), &[]);
        let msg = HandleMsg::Reset { count: 5 };
        let _res = handle(&mut deps, auth_params, msg).unwrap();

        // should now be 5
        let res = query(&deps, QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_slice(&res).unwrap();
        assert_eq!(5, value.count);
    }
}
