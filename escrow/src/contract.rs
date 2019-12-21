use std::str::from_utf8;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{OptionExt, ResultExt};

use cosmwasm::errors::{ContractErr, ParseErr, Result, SerializeErr, Unauthorized, Utf8Err};
use cosmwasm::query::perform_raw_query;
use cosmwasm::serde::{from_slice, to_vec};
use cosmwasm::storage::Storage;
use cosmwasm::types::{Coin, CosmosMsg, Params, QueryResponse, RawQuery, Response};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub arbiter: String,
    pub recipient: String,
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
    Raw(RawQuery),
}

// raw_query is a helper to generate a serialized format of a raw_query
// meant for test code and integration tests
pub fn raw_query(key: &[u8]) -> Result<Vec<u8>> {
    let key = from_utf8(key).context(Utf8Err {})?.to_string();
    to_vec(&QueryMsg::Raw(RawQuery { key })).context(SerializeErr { kind: "QueryMsg" })
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub arbiter: String,
    pub recipient: String,
    pub source: String,
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

pub fn init<T: Storage>(store: &mut T, params: Params, msg: Vec<u8>) -> Result<Response> {
    let msg: InitMsg = from_slice(&msg).context(ParseErr { kind: "InitMsg" })?;
    let state = State {
        arbiter: msg.arbiter,
        recipient: msg.recipient,
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
        store.set(
            CONFIG_KEY,
            &to_vec(&state).context(SerializeErr { kind: "State" })?,
        );
        Ok(Response::default())
    }
}

pub fn handle<T: Storage>(store: &mut T, params: Params, msg: Vec<u8>) -> Result<Response> {
    let msg: HandleMsg = from_slice(&msg).context(ParseErr { kind: "HandleMsg" })?;
    let data = store.get(CONFIG_KEY).context(ContractErr {
        msg: "uninitialized data",
    })?;
    let state: State = from_slice(&data).context(ParseErr { kind: "State" })?;

    match msg {
        HandleMsg::Approve { quantity } => try_approve(params, state, quantity),
        HandleMsg::Refund {} => try_refund(params, state),
    }
}

fn try_approve(params: Params, state: State, quantity: Option<Vec<Coin>>) -> Result<Response> {
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
                from_address: params.contract.address,
                to_address: state.recipient,
                amount,
            }],
            log: Some("paid out funds".to_string()),
            data: None,
        };
        Ok(res)
    }
}

fn try_refund(params: Params, state: State) -> Result<Response> {
    // anyone can try to refund, as long as the contract is expired
    if !state.is_expired(&params) {
        ContractErr {
            msg: "escrow not yet expired",
        }
        .fail()
    } else {
        let res = Response {
            messages: vec![CosmosMsg::Send {
                from_address: params.contract.address,
                to_address: state.source,
                amount: params.contract.balance.unwrap_or_default(),
            }],
            log: Some("returned funds".to_string()),
            data: None,
        };
        Ok(res)
    }
}

pub fn query<T: Storage>(store: &T, msg: Vec<u8>) -> Result<QueryResponse> {
    let msg: QueryMsg = from_slice(&msg).context(ParseErr { kind: "QueryMsg" })?;
    match msg {
        QueryMsg::Raw(raw) => perform_raw_query(store, raw),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm::errors::Error;
    use cosmwasm::mock::MockStorage;
    use cosmwasm::types::{coin, mock_params};

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
        let mut store = MockStorage::new();
        let msg = init_msg(1000, 0);
        let params = mock_params_height("creator", &coin("1000", "earth"), &[], 876, 0);
        let res = init(&mut store, params, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let q_res = query(&store, raw_query(CONFIG_KEY).unwrap()).unwrap();
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
        let mut store = MockStorage::new();
        let msg = init_msg(1000, 0);
        let params = mock_params_height("creator", &coin("1000", "earth"), &[], 1001, 0);
        let res = init(&mut store, params, msg);
        assert!(res.is_err());
        if let Err(Error::ContractErr { msg, .. }) = res {
            assert_eq!(msg, "creating expired escrow".to_string());
        } else {
            assert!(false, "wrong error type");
        }
    }

    #[test]
    fn fails_on_bad_init_data() {
        let mut store = MockStorage::new();
        let bad_msg = b"{}".to_vec();
        let params = mock_params_height("creator", &coin("1000", "earth"), &[], 876, 0);
        let res = init(&mut store, params, bad_msg);
        assert!(res.is_err());
        if let Err(Error::ParseErr { .. }) = res {
        } else {
            assert!(false, "wrong error type");
        }
    }

    #[test]
    fn handle_approve() {
        let mut store = MockStorage::new();

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
            Ok(_) => panic!("expected error"),
            Err(Error::Unauthorized { .. }) => {}
            Err(e) => panic!("unexpected error: {:?}", e),
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
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "escrow expired".to_string()),
            Err(e) => panic!("unexpected error: {:?}", e),
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
        let mut store = MockStorage::new();

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
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => {
                assert_eq!(msg, "escrow not yet expired".to_string())
            }
            Err(e) => panic!("unexpected error: {:?}", e),
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
}
