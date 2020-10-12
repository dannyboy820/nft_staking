#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{
        mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
    };
    use cosmwasm_std::{coin, coins, from_binary, Coin, Extern, HumanAddr};

    use crate::contract::{handle, init, query};
    use crate::error::ContractError;
    use crate::msg::{HandleMsg, InitMsg, QueryMsg, ResolveRecordResponse};
    use crate::state::Config;

    fn assert_name_owner(
        deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        name: &str,
        owner: &str,
    ) {
        let res = query(
            &deps,
            mock_env(),
            QueryMsg::ResolveRecord {
                name: name.to_string(),
            },
        )
        .unwrap();

        let value: ResolveRecordResponse = from_binary(&res).unwrap();
        assert_eq!(Some(HumanAddr::from(owner)), value.address);
    }

    fn assert_config_state(deps: &mut Extern<MockStorage, MockApi, MockQuerier>, expected: Config) {
        let res = query(&deps, mock_env(), QueryMsg::Config {}).unwrap();
        let value: Config = from_binary(&res).unwrap();
        assert_eq!(value, expected);
    }

    fn mock_init_with_price(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        purchase_price: Coin,
        transfer_price: Coin,
    ) {
        let msg = InitMsg {
            purchase_price: Some(purchase_price),
            transfer_price: Some(transfer_price),
        };

        let info = mock_info("creator", &coins(2, "token"));
        let _res =
            init(&mut deps, mock_env(), info, msg).expect("contract successfully handles InitMsg");
    }

    fn mock_init_no_price(mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>) {
        let msg = InitMsg {
            purchase_price: None,
            transfer_price: None,
        };

        let info = mock_info("creator", &coins(2, "token"));
        let _res =
            init(&mut deps, mock_env(), info, msg).expect("contract successfully handles InitMsg");
    }

    fn mock_alice_registers_name(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        sent: &[Coin],
    ) {
        // alice can register an available name
        let info = mock_info("alice_key", sent);
        let msg = HandleMsg::Register {
            name: "alice".to_string(),
        };
        let _res = handle(&mut deps, mock_env(), info, msg)
            .expect("contract successfully handles Register message");
    }

    #[test]
    fn proper_init_no_fees() {
        let mut deps = mock_dependencies(&[]);

        mock_init_no_price(&mut deps);

        assert_config_state(
            &mut deps,
            Config {
                purchase_price: None,
                transfer_price: None,
            },
        );
    }

    #[test]
    fn proper_init_with_fees() {
        let mut deps = mock_dependencies(&[]);

        mock_init_with_price(&mut deps, coin(3, "token"), coin(4, "token"));

        assert_config_state(
            &mut deps,
            Config {
                purchase_price: Some(coin(3, "token")),
                transfer_price: Some(coin(4, "token")),
            },
        );
    }

    #[test]
    fn register_available_name_and_query_works() {
        let mut deps = mock_dependencies(&[]);
        mock_init_no_price(&mut deps);
        mock_alice_registers_name(&mut deps, &[]);

        // querying for name resolves to correct address
        assert_name_owner(&mut deps, "alice", "alice_key");
    }

    #[test]
    fn register_available_name_and_query_works_with_fees() {
        let mut deps = mock_dependencies(&[]);
        mock_init_with_price(&mut deps, coin(2, "token"), coin(2, "token"));
        mock_alice_registers_name(&mut deps, &coins(2, "token"));

        // anyone can register an available name with more fees than needed
        let info = mock_info("bob_key", &coins(5, "token"));
        let msg = HandleMsg::Register {
            name: "bob".to_string(),
        };

        let _res = handle(&mut deps, mock_env(), info, msg)
            .expect("contract successfully handles Register message");

        // querying for name resolves to correct address
        assert_name_owner(&mut deps, "alice", "alice_key");
        assert_name_owner(&mut deps, "bob", "bob_key");
    }

    #[test]
    fn fails_on_register_already_taken_name() {
        let mut deps = mock_dependencies(&[]);
        mock_init_no_price(&mut deps);
        mock_alice_registers_name(&mut deps, &[]);

        // bob can't register the same name
        let info = mock_info("bob_key", &coins(2, "token"));
        let msg = HandleMsg::Register {
            name: "alice".to_string(),
        };
        let res = handle(&mut deps, mock_env(), info, msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::NameTaken { .. }) => {}
            Err(_) => panic!("Unknown error"),
        }
        // alice can't register the same name again
        let info = mock_info("alice_key", &coins(2, "token"));
        let msg = HandleMsg::Register {
            name: "alice".to_string(),
        };
        let res = handle(&mut deps, mock_env(), info, msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::NameTaken { .. }) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn register_available_name_fails_with_invalid_name() {
        let mut deps = mock_dependencies(&[]);
        mock_init_no_price(&mut deps);
        let info = mock_info("bob_key", &coins(2, "token"));

        // hi is too short
        let msg = HandleMsg::Register {
            name: "hi".to_string(),
        };
        match handle(&mut deps, mock_env(), info.clone(), msg) {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::NameTooShort { .. }) => {}
            Err(_) => panic!("Unknown error"),
        }

        // 65 chars is too long
        let msg = HandleMsg::Register {
            name: "01234567890123456789012345678901234567890123456789012345678901234".to_string(),
        };
        match handle(&mut deps, mock_env(), info.clone(), msg) {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::NameTooLong { .. }) => {}
            Err(_) => panic!("Unknown error"),
        }

        // no upper case...
        let msg = HandleMsg::Register {
            name: "LOUD".to_string(),
        };
        match handle(&mut deps, mock_env(), info.clone(), msg) {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::InvalidCharacter { c }) => assert_eq!(c, 'L'),
            Err(_) => panic!("Unknown error"),
        }
        // ... or spaces
        let msg = HandleMsg::Register {
            name: "two words".to_string(),
        };
        match handle(&mut deps, mock_env(), info, msg) {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::InvalidCharacter { .. }) => {}
            Err(_) => panic!("Unknown error"),
        }
    }

    #[test]
    fn fails_on_register_insufficient_fees() {
        let mut deps = mock_dependencies(&[]);
        mock_init_with_price(&mut deps, coin(2, "token"), coin(2, "token"));

        // anyone can register an available name with sufficient fees
        let info = mock_info("alice_key", &[]);
        let msg = HandleMsg::Register {
            name: "alice".to_string(),
        };

        let res = handle(&mut deps, mock_env(), info, msg);

        match res {
            Ok(_) => panic!("register call should fail with insufficient fees"),
            Err(ContractError::InsufficientFundsSend {}) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn fails_on_register_wrong_fee_denom() {
        let mut deps = mock_dependencies(&[]);
        mock_init_with_price(&mut deps, coin(2, "token"), coin(2, "token"));

        // anyone can register an available name with sufficient fees
        let info = mock_info("alice_key", &coins(2, "earth"));
        let msg = HandleMsg::Register {
            name: "alice".to_string(),
        };

        let res = handle(&mut deps, mock_env(), info, msg);

        match res {
            Ok(_) => panic!("register call should fail with insufficient fees"),
            Err(ContractError::InsufficientFundsSend {}) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn transfer_works() {
        let mut deps = mock_dependencies(&[]);
        mock_init_no_price(&mut deps);
        mock_alice_registers_name(&mut deps, &[]);

        // alice can transfer her name successfully to bob
        let info = mock_info("alice_key", &[]);
        let msg = HandleMsg::Transfer {
            name: "alice".to_string(),
            to: HumanAddr::from("bob_key"),
        };

        let _res = handle(&mut deps, mock_env(), info, msg)
            .expect("contract successfully handles Transfer message");
        // querying for name resolves to correct address (bob_key)
        assert_name_owner(&mut deps, "alice", "bob_key");
    }

    #[test]
    fn transfer_works_with_fees() {
        let mut deps = mock_dependencies(&[]);
        mock_init_with_price(&mut deps, coin(2, "token"), coin(2, "token"));
        mock_alice_registers_name(&mut deps, &coins(2, "token"));

        // alice can transfer her name successfully to bob
        let info = mock_info("alice_key", &vec![coin(1, "earth"), coin(2, "token")]);
        let msg = HandleMsg::Transfer {
            name: "alice".to_string(),
            to: HumanAddr::from("bob_key"),
        };

        let _res = handle(&mut deps, mock_env(), info, msg)
            .expect("contract successfully handles Transfer message");
        // querying for name resolves to correct address (bob_key)
        assert_name_owner(&mut deps, "alice", "bob_key");
    }

    #[test]
    fn fails_on_transfer_non_existent() {
        let mut deps = mock_dependencies(&[]);
        mock_init_no_price(&mut deps);
        mock_alice_registers_name(&mut deps, &[]);

        // alice can transfer her name successfully to bob
        let info = mock_info("frank_key", &coins(2, "token"));
        let msg = HandleMsg::Transfer {
            name: "alice42".to_string(),
            to: HumanAddr::from("bob_key"),
        };

        let res = handle(&mut deps, mock_env(), info, msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::NameNotExists { name }) => assert_eq!(name, "alice42"),
            Err(e) => panic!("Unexpected error: {:?}", e),
        }

        // querying for name resolves to correct address (alice_key)
        assert_name_owner(&mut deps, "alice", "alice_key");
    }

    #[test]
    fn fails_on_transfer_from_nonowner() {
        let mut deps = mock_dependencies(&[]);
        mock_init_no_price(&mut deps);
        mock_alice_registers_name(&mut deps, &[]);

        // alice can transfer her name successfully to bob
        let info = mock_info("frank_key", &coins(2, "token"));
        let msg = HandleMsg::Transfer {
            name: "alice".to_string(),
            to: HumanAddr::from("bob_key"),
        };

        let res = handle(&mut deps, mock_env(), info, msg);

        match res {
            Ok(_) => panic!("Must return error"),
            Err(ContractError::Unauthorized { .. }) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }

        // querying for name resolves to correct address (alice_key)
        assert_name_owner(&mut deps, "alice", "alice_key");
    }

    #[test]
    fn fails_on_transfer_insufficient_fees() {
        let mut deps = mock_dependencies(&[]);
        mock_init_with_price(&mut deps, coin(2, "token"), coin(5, "token"));
        mock_alice_registers_name(&mut deps, &coins(2, "token"));

        // alice can transfer her name successfully to bob
        let info = mock_info("alice_key", &vec![coin(1, "earth"), coin(2, "token")]);
        let msg = HandleMsg::Transfer {
            name: "alice".to_string(),
            to: HumanAddr::from("bob_key"),
        };

        let res = handle(&mut deps, mock_env(), info, msg);

        match res {
            Ok(_) => panic!("register call should fail with insufficient fees"),
            Err(ContractError::InsufficientFundsSend {}) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }

        // querying for name resolves to correct address (bob_key)
        assert_name_owner(&mut deps, "alice", "alice_key");
    }

    #[test]
    fn returns_empty_on_query_unregistered_name() {
        let mut deps = mock_dependencies(&[]);

        mock_init_no_price(&mut deps);

        // querying for unregistered name results in NotFound error
        let res = query(
            &deps,
            mock_env(),
            QueryMsg::ResolveRecord {
                name: "alice".to_string(),
            },
        )
        .unwrap();
        let value: ResolveRecordResponse = from_binary(&res).unwrap();
        assert_eq!(None, value.address);
    }
}
