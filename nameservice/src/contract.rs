use cosmwasm::errors::{contract_err, Result, unauthorized};
use cosmwasm::traits::{Api, Extern, Storage};
use cosmwasm::types::{HumanAddr, Params, Response};

use crate::msg::{HandleMsg, InitMsg, QueryMsg, ResolveRecordResponse};
use crate::state::{resolver, resolver_read, NameRecord};

use cw_storage::serialize;

pub fn init<S: Storage, A: Api>(
    _deps: &mut Extern<S, A>,
    _params: Params,
    _msg: InitMsg,
) -> Result<Response> {
    Ok(Response::default())
}

pub fn handle<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    params: Params,
    msg: HandleMsg,
) -> Result<Response> {
    match msg {
        HandleMsg::Register { name } => try_register(deps, params, name),
        HandleMsg::Transfer { name, to } => try_transfer(deps, params, name, to),
    }
}

pub fn try_register<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    params: Params,
    name: String,
) -> Result<Response> {
    let key = name.as_bytes();

    let record = NameRecord {
        owner: params.message.signer,
    };

    if let None = resolver(&mut deps.storage).may_load(key)? {
        // name is available
        resolver(&mut deps.storage).save(key, &record)?;
    } else {
        // name is already taken
        contract_err("Name is already taken")?;
    }

    Ok(Response::default())
}

pub fn try_transfer<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    params: Params,
    name: String,
    to: HumanAddr,
) -> Result<Response> {
    let key = name.as_bytes();
    let new_owner = deps.api.canonical_address(&to)?;

    resolver(&mut deps.storage).update(key, &|mut record| {
        if params.message.signer != record.owner {
            unauthorized()?;
        }

        record.owner = new_owner.clone();
        Ok(record)
    })?;
    Ok(Response::default())
}

pub fn query<S: Storage, A: Api>(deps: &Extern<S, A>, msg: QueryMsg) -> Result<Vec<u8>> {
    match msg {
        QueryMsg::ResolveRecord { name } => query_resolver(deps, name),
    }
}

fn query_resolver<S: Storage, A: Api>(deps: &Extern<S, A>, name: String) -> Result<Vec<u8>> {
    let key = name.as_bytes();

    let record = resolver_read(&deps.storage).load(key)?;
    let address = deps.api.human_address(&record.owner)?;

    let resp = ResolveRecordResponse { address };

    serialize(&resp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm::errors::Error;
    use cosmwasm::mock::{dependencies, mock_params, MockApi, MockStorage};
    use cosmwasm::types::coin;

    use cw_storage::deserialize;

    fn assert_name_owner(deps: &mut Extern<MockStorage, MockApi>, name: &str, owner: &str) {
        let res = query(
            &deps,
            QueryMsg::ResolveRecord {
                name: name.to_string(),
            },
        )
        .unwrap();

        let value: ResolveRecordResponse = deserialize(&res).unwrap();
        assert_eq!(HumanAddr::from(owner), value.address);
    }

    fn mock_init_and_alice_registers_name(mut deps: &mut Extern<MockStorage, MockApi>) {
        let msg = InitMsg {};
        let params = mock_params(&deps.api, "creator", &coin("2", "token"), &[]);
        let _res = init(&mut deps, params, msg).unwrap();

        // anyone can register an available name
        let params = mock_params(&deps.api, "alice_key", &coin("2", "token"), &[]);
        let msg = HandleMsg::Register {
            name: "alice".to_string(),
        };
        let _res =
            handle(&mut deps, params, msg).expect("contract successfully handles Register message");
    }

    #[test]
    fn proper_initialization() {
        let mut deps = dependencies(20);

        let msg = InitMsg {};
        let params = mock_params(&deps.api, "creator", &coin("1000", "earth"), &[]);

        // we can just call .unwrap() to assert this was a success
        let res = init(&mut deps, params, msg).unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn register_available_name_and_query_works() {
        let mut deps = dependencies(20);
        mock_init_and_alice_registers_name(&mut deps);

        // querying for name resolves to correct address
        assert_name_owner(&mut deps, "alice", "alice_key");
    }

    #[test]
    fn fails_on_register_already_taken_name() {
        let mut deps = dependencies(20);
        mock_init_and_alice_registers_name(&mut deps);

        // bob can't register the same name
        let params = mock_params(&deps.api, "bob_key", &coin("2", "token"), &[]);
        let msg = HandleMsg::Register {
            name: "alice".to_string(),
        };
        let res = handle(&mut deps, params, msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "Name is already taken"),
            Err(_) => panic!("Unknown error"),
        }
        // alice can't register the same name again
        let params = mock_params(&deps.api, "alice_key", &coin("2", "token"), &[]);
        let msg = HandleMsg::Register {
            name: "alice".to_string(),
        };
        let res = handle(&mut deps, params, msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "Name is already taken"),
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn transfer_works() {
        let mut deps = dependencies(20);
        mock_init_and_alice_registers_name(&mut deps);

        // alice can transfer her name successfully to bob
        let params = mock_params(&deps.api, "alice_key", &coin("2", "token"), &[]);
        let msg = HandleMsg::Transfer {
            name: "alice".to_string(),
            to: HumanAddr::from("bob_key"),
        };

        let _res =
            handle(&mut deps, params, msg).expect("contract successfully handles Transfer message");
        // querying for name resolves to correct address (bob_key)
        assert_name_owner(&mut deps, "alice", "bob_key");
    }

    #[test]
    fn fails_on_transfer_from_nonowner() {
        let mut deps = dependencies(20);
        mock_init_and_alice_registers_name(&mut deps);

        // alice can transfer her name successfully to bob
        let params = mock_params(&deps.api, "frank_key", &coin("2", "token"), &[]);
        let msg = HandleMsg::Transfer {
            name: "alice".to_string(),
            to: HumanAddr::from("bob_key"),
        };

        let res = handle(&mut deps, params, msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(Error::Unauthorized { .. }) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }

        // querying for name resolves to correct address (alice_key)
        assert_name_owner(&mut deps, "alice", "alice_key");
    }

    #[test]
    fn fails_on_query_unregistered_name() {
        let mut deps = dependencies(20);

        let msg = InitMsg {};
        let params = mock_params(&deps.api, "creator", &coin("2", "token"), &[]);
        let _res = init(&mut deps, params, msg).unwrap();

        // querying for unregistered name results in NotFound error
        let res = query(
            &deps,
            QueryMsg::ResolveRecord {
                name: "alice".to_string(),
            },
        );

        match res {
            Ok(_) => panic!("Must return error"),
            Err(Error::NotFound { kind, .. }) => assert_eq!(kind, "NameRecord"),
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
}
