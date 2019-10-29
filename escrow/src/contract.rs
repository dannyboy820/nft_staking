use serde::{Deserialize, Serialize};
use snafu::{OptionExt, ResultExt};

use cosmwasm::errors::{ContractErr, ParseErr, Result, SerializeErr, Unauthorized};
use cosmwasm::serde::{from_slice, to_vec};
use cosmwasm::storage::Storage;
use cosmwasm::types::{Coin, CosmosMsg, Params, Response};

#[derive(Serialize, Deserialize)]
pub struct InitMsg {
    pub arbiter: String,
    pub recipient: String,
    // you can set a last time or block height the contract is valid at
    // if *either* is non-zero and below current state, the contract is considered expired
    // and will be returned to the original funder
    pub end_height: i64,
    pub end_time: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HandleMsg {
    Approve {
        // release some coins - if quantity is None, release all coins in balance
        quantity: Option<Vec<Coin>>,
    },
    Refund {},
}

#[derive(Serialize, Deserialize)]
pub struct State {
    pub arbiter: String,
    pub recipient: String,
    pub source: String,
    pub end_height: i64,
    pub end_time: i64,
}

impl State {
    fn is_expired(&self, params: &Params) -> bool {
        (self.end_height != 0 && params.block.height >= self.end_height) ||
            (self.end_time != 0 && params.block.time >= self.end_time)
    }
}

pub static CONFIG_KEY: &[u8] = b"config";

pub fn init<T: Storage>(store: &mut T, params: Params, msg: Vec<u8>) -> Result<Response> {
    let msg: InitMsg = from_slice(&msg).context(ParseErr {})?;
    let state = State {
        arbiter: msg.arbiter,
        recipient: msg.recipient,
        source: params.message.signer.clone(),
        end_height: msg.end_height,
        end_time: msg.end_time,
    };
    if state.is_expired(&params) {
        ContractErr { msg: "creating expired escrow".to_string() }.fail()
    } else {
        store.set(
            CONFIG_KEY,
            &to_vec(&state)
                .context(SerializeErr {})?,
        );
        Ok(Response::default())
    }
}

pub fn handle<T: Storage>(store: &mut T, params: Params, msg: Vec<u8>) -> Result<Response> {
    let msg: HandleMsg = from_slice(&msg).context(ParseErr {})?;
    let data = store.get(CONFIG_KEY).context(ContractErr {
        msg: "uninitialized data".to_string(),
    })?;
    let state: State = from_slice(&data).context(ParseErr {})?;

    match msg {
        HandleMsg::Approve { quantity } => try_approve(params, state, quantity),
        HandleMsg::Refund {} => try_refund(params, state),
    }
}

fn try_approve(params: Params, state: State, quantity: Option<Vec<Coin>>) -> Result<Response> {
    if params.message.signer != state.arbiter {
        Unauthorized {}.fail()
    } else if state.is_expired(&params) {
        ContractErr { msg: "escrow expired".to_string() }.fail()
    } else {
        let amount = match quantity {
            None => params.contract.balance,
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
        ContractErr { msg: "escrow not yet expired".to_string() }.fail()
    } else {
        let res = Response {
            messages: vec![CosmosMsg::Send {
                from_address: params.contract.address,
                to_address: state.recipient,
                amount: params.contract.balance,
            }],
            log: Some("returned funds".to_string()),
            data: None,
        };
        Ok(res)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm::mock::MockStorage;
    use cosmwasm::types::{coin, mock_params};

    #[test]
    fn proper_initialization() {
        let mut store = MockStorage::new();
        let msg = to_vec(&InitMsg {
            verifier: String::from("verifies"),
            beneficiary: String::from("benefits"),
        })
            .unwrap();
        let params = mock_params("creator", &coin("1000", "earth"), &[]);
        let res = init(&mut store, params, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's check the state
        let data = store.get(CONFIG_KEY).expect("no data stored");
        let state: State = from_slice(&data).unwrap();
        assert_eq!(state.verifier, String::from("verifies"));
        assert_eq!(state.beneficiary, String::from("benefits"));
        assert_eq!(state.funder, String::from("creator"));
    }

    #[test]
    fn fails_on_bad_init() {
        let mut store = MockStorage::new();
        let bad_msg = b"{}".to_vec();
        let params = mock_params("creator", &coin("1000", "earth"), &[]);
        let res = init(&mut store, params, bad_msg);
        assert_eq!(true, res.is_err());
    }

    #[test]
    fn proper_handle() {
        let mut store = MockStorage::new();

        // initialize the store
        let init_msg = to_vec(&InitMsg {
            verifier: String::from("verifies"),
            beneficiary: String::from("benefits"),
        })
            .unwrap();
        let init_params = mock_params("creator", &coin("1000", "earth"), &coin("1000", "earth"));
        let init_res = init(&mut store, init_params, init_msg).unwrap();
        assert_eq!(0, init_res.messages.len());

        // beneficiary can release it
        let handle_params = mock_params("verifies", &coin("15", "earth"), &coin("1015", "earth"));
        let handle_res = handle(&mut store, handle_params, Vec::new()).unwrap();
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
                assert_eq!(coin.amount, "1015");
            }
            _ => panic!("Unexpected message type"),
        }

        // it worked, let's check the state
        let data = store.get(CONFIG_KEY).expect("no data stored");
        let state: State = from_slice(&data).unwrap();
        assert_eq!(state.verifier, String::from("verifies"));
        assert_eq!(state.beneficiary, String::from("benefits"));
        assert_eq!(state.funder, String::from("creator"));
    }

    #[test]
    fn failed_handle() {
        let mut store = MockStorage::new();

        // initialize the store
        let init_msg = to_vec(&InitMsg {
            verifier: String::from("verifies"),
            beneficiary: String::from("benefits"),
        })
            .unwrap();
        let init_params = mock_params("creator", &coin("1000", "earth"), &coin("1000", "earth"));
        let init_res = init(&mut store, init_params, init_msg).unwrap();
        assert_eq!(0, init_res.messages.len());

        // beneficiary can release it
        let handle_params = mock_params("benefits", &[], &coin("1000", "earth"));
        let handle_res = handle(&mut store, handle_params, Vec::new());
        assert!(handle_res.is_err());

        // state should not change
        let data = store.get(CONFIG_KEY).expect("no data stored");
        let state: State = from_slice(&data).unwrap();
        assert_eq!(state.verifier, String::from("verifies"));
        assert_eq!(state.beneficiary, String::from("benefits"));
        assert_eq!(state.funder, String::from("creator"));
    }
}
