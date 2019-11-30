use super::*;
use cosmwasm::errors::Error;
use cosmwasm::mock::MockStorage;
use cosmwasm::types::mock_params;

fn mock_params_height(signer: &str, height: i64, time: i64) -> Params {
    let mut params = mock_params(signer, &[], &[]);
    params.block.height = height;
    params.block.time = time;
    params
}

fn get_name<T: Storage>(store: &T) -> String {
    let data = store.get(KEY_NAME).expect("no name data stored");
    let value = String::from_utf8(data).unwrap();
    return value;
}

fn get_symbol<T: Storage>(store: &T) -> String {
    let data = store.get(KEY_SYMBOL).expect("no symbol data stored");
    let value = String::from_utf8(data).unwrap();
    return value;
}

fn get_decimals<T: Storage>(store: &T) -> u8 {
    let data = store.get(KEY_DECIMALS).expect("no decimals data stored");
    let value = u8::from_be_bytes(data[0..1].try_into().unwrap());
    return value;
}

fn get_total_supply<T: Storage>(store: &T) -> u64 {
    let data = store
        .get(KEY_TOTAL_SUPPLY)
        .expect("no total_supply data stored");
    let value = u64::from_be_bytes(data[0..8].try_into().unwrap());
    return value;
}

fn get_balance<T: Storage>(store: &T, address: &str) -> u64 {
    let raw_address = parse_20bytes_from_hex(&address).unwrap();
    let data = store
        .get(&raw_address)
        .expect("no data stored for this address");
    let state: AddressState = from_slice(&data)
        .context(ParseErr {
            kind: "AddressState",
        })
        .unwrap();
    return state.balance;
}

fn get_allowance<T: Storage>(store: &T, owner: &str, spender: &str) -> u64 {
    let owner_raw_address = parse_20bytes_from_hex(owner).unwrap();
    let spender_raw_address = parse_20bytes_from_hex(spender).unwrap();
    let key = [&owner_raw_address[..], &spender_raw_address[..]].concat();
    return match store.get(&key) {
        Some(data) => u64::from_be_bytes(data[0..8].try_into().unwrap()),
        None => 0,
    };
}

mod init {
    use super::*;

    #[test]
    fn works() {
        let mut store = MockStorage::new();
        let msg = to_vec(&InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: [InitialBalance {
                address: "0000000000000000000000000000000000000000".to_string(),
                amount: 11223344,
            }]
            .to_vec(),
        })
        .unwrap();
        let params = mock_params_height("creator", 450, 550);
        let res = init(&mut store, params, msg).unwrap();
        assert_eq!(0, res.messages.len());

        assert_eq!(get_name(&store), "Cash Token");
        assert_eq!(get_symbol(&store), "CASH");
        assert_eq!(get_decimals(&store), 9);
        assert_eq!(
            get_balance(&store, "0000000000000000000000000000000000000000"),
            11223344
        );
        assert_eq!(get_total_supply(&store), 11223344);
    }

    #[test]
    fn works_with_empty_balance() {
        let mut store = MockStorage::new();
        let msg = to_vec(&InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: [].to_vec(),
        })
        .unwrap();
        let params = mock_params_height("creator", 450, 550);
        let res = init(&mut store, params, msg).unwrap();
        assert_eq!(0, res.messages.len());

        assert_eq!(get_total_supply(&store), 0);
    }

    #[test]
    fn works_with_multiple_balances() {
        let mut store = MockStorage::new();
        let msg = to_vec(&InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: [
                InitialBalance {
                    address: "0000000000000000000000000000000000000000".to_string(),
                    amount: 11,
                },
                InitialBalance {
                    address: "1111111111111111111111111111111111111111".to_string(),
                    amount: 22,
                },
                InitialBalance {
                    address: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                    amount: 33,
                },
            ]
            .to_vec(),
        })
        .unwrap();
        let params = mock_params_height("creator", 450, 550);
        let res = init(&mut store, params, msg).unwrap();
        assert_eq!(0, res.messages.len());

        assert_eq!(
            get_balance(&store, "0000000000000000000000000000000000000000"),
            11
        );
        assert_eq!(
            get_balance(&store, "1111111111111111111111111111111111111111"),
            22
        );
        assert_eq!(
            get_balance(&store, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            33
        );
        assert_eq!(get_total_supply(&store), 66);
    }

    #[test]
    fn fails_for_large_decimals() {
        let mut store = MockStorage::new();
        let msg = to_vec(&InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 42,
            initial_balances: [].to_vec(),
        })
        .unwrap();
        let params = mock_params_height("creator", 450, 550);
        let result = init(&mut store, params, msg);
        match result {
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "Decimals must not exceed 18"),
            Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn fails_for_name_too_short() {
        let mut store = MockStorage::new();
        let msg = to_vec(&InitMsg {
            name: "CC".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: [].to_vec(),
        })
        .unwrap();
        let params = mock_params_height("creator", 450, 550);
        let result = init(&mut store, params, msg);
        match result {
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => {
                assert_eq!(msg, "Name is not in the expected format (3-30 UTF-8 bytes)")
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn fails_for_name_too_long() {
        let mut store = MockStorage::new();
        let msg = to_vec(&InitMsg {
            name: "Cash coin. Cash coin. Cash coin. Cash coin.".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: [].to_vec(),
        })
        .unwrap();
        let params = mock_params_height("creator", 450, 550);
        let result = init(&mut store, params, msg);
        match result {
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => {
                assert_eq!(msg, "Name is not in the expected format (3-30 UTF-8 bytes)")
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn fails_for_symbol_too_short() {
        let mut store = MockStorage::new();
        let msg = to_vec(&InitMsg {
            name: "De De".to_string(),
            symbol: "DD".to_string(),
            decimals: 9,
            initial_balances: [].to_vec(),
        })
        .unwrap();
        let params = mock_params_height("creator", 450, 550);
        let result = init(&mut store, params, msg);
        match result {
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => {
                assert_eq!(msg, "Ticker symbol is not in expected format [A-Z]{3,6}")
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn fails_for_symbol_too_long() {
        let mut store = MockStorage::new();
        let msg = to_vec(&InitMsg {
            name: "Super Coin".to_string(),
            symbol: "SUPERCOIN".to_string(),
            decimals: 9,
            initial_balances: [].to_vec(),
        })
        .unwrap();
        let params = mock_params_height("creator", 450, 550);
        let result = init(&mut store, params, msg);
        match result {
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => {
                assert_eq!(msg, "Ticker symbol is not in expected format [A-Z]{3,6}")
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn fails_for_symbol_lowercase() {
        let mut store = MockStorage::new();
        let msg = to_vec(&InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CaSH".to_string(),
            decimals: 9,
            initial_balances: [].to_vec(),
        })
        .unwrap();
        let params = mock_params_height("creator", 450, 550);
        let result = init(&mut store, params, msg);
        match result {
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => {
                assert_eq!(msg, "Ticker symbol is not in expected format [A-Z]{3,6}")
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }
    }
}

mod transfer {
    use super::*;

    fn make_init_msg() -> InitMsg {
        return InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: vec![
                InitialBalance {
                    address: "0000000000000000000000000000000000000000".to_string(),
                    amount: 11,
                },
                InitialBalance {
                    address: "1111111111111111111111111111111111111111".to_string(),
                    amount: 22,
                },
                InitialBalance {
                    address: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                    amount: 33,
                },
            ],
        };
    }

    #[test]
    fn can_send_to_existing_recipient() {
        let mut store = MockStorage::new();
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height("0000000000000000000000000000000000000000", 450, 550);
        let res = init(&mut store, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Initial state
        assert_eq!(
            get_balance(&store, "0000000000000000000000000000000000000000"),
            11
        );
        assert_eq!(
            get_balance(&store, "1111111111111111111111111111111111111111"),
            22
        );
        assert_eq!(
            get_balance(&store, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            33
        );
        assert_eq!(get_total_supply(&store), 66);

        // Transfer
        let transfer_msg = to_vec(&HandleMsg::Transfer {
            recipient: "1111111111111111111111111111111111111111".to_string(),
            amount: 1,
        })
        .unwrap();
        let params2 = mock_params_height("0000000000000000000000000000000000000000", 450, 550);
        let transfer_result = handle(&mut store, params2, transfer_msg).unwrap();
        assert_eq!(transfer_result.messages.len(), 0);
        assert_eq!(transfer_result.log, Some("transfer successful".to_string()));

        // New state
        assert_eq!(
            get_balance(&store, "0000000000000000000000000000000000000000"),
            10
        ); // -1
        assert_eq!(
            get_balance(&store, "1111111111111111111111111111111111111111"),
            23
        ); // +1
        assert_eq!(
            get_balance(&store, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            33
        );
        assert_eq!(get_total_supply(&store), 66);
    }

    #[test]
    fn can_send_to_non_existent_recipient() {
        let mut store = MockStorage::new();
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height("0000000000000000000000000000000000000000", 450, 550);
        let res = init(&mut store, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Initial state
        assert_eq!(
            get_balance(&store, "0000000000000000000000000000000000000000"),
            11
        );
        assert_eq!(
            get_balance(&store, "1111111111111111111111111111111111111111"),
            22
        );
        assert_eq!(
            get_balance(&store, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            33
        );
        assert_eq!(get_total_supply(&store), 66);

        // Transfer
        let transfer_msg = to_vec(&HandleMsg::Transfer {
            recipient: "2323232323232323232323232323232323232323".to_string(),
            amount: 1,
        })
        .unwrap();
        let params2 = mock_params_height("0000000000000000000000000000000000000000", 450, 550);
        let transfer_result = handle(&mut store, params2, transfer_msg).unwrap();
        assert_eq!(transfer_result.messages.len(), 0);
        assert_eq!(transfer_result.log, Some("transfer successful".to_string()));

        // New state
        assert_eq!(
            get_balance(&store, "0000000000000000000000000000000000000000"),
            10
        );
        assert_eq!(
            get_balance(&store, "1111111111111111111111111111111111111111"),
            22
        );
        assert_eq!(
            get_balance(&store, "2323232323232323232323232323232323232323"),
            1
        );
        assert_eq!(
            get_balance(&store, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            33
        );
        assert_eq!(get_total_supply(&store), 66);
    }

    #[test]
    fn can_send_zero_amount() {
        let mut store = MockStorage::new();
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height("0000000000000000000000000000000000000000", 450, 550);
        let res = init(&mut store, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Initial state
        assert_eq!(
            get_balance(&store, "0000000000000000000000000000000000000000"),
            11
        );
        assert_eq!(
            get_balance(&store, "1111111111111111111111111111111111111111"),
            22
        );
        assert_eq!(
            get_balance(&store, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            33
        );
        assert_eq!(get_total_supply(&store), 66);

        // Transfer
        let transfer_msg = to_vec(&HandleMsg::Transfer {
            recipient: "1111111111111111111111111111111111111111".to_string(),
            amount: 0,
        })
        .unwrap();
        let params2 = mock_params_height("0000000000000000000000000000000000000000", 450, 550);
        let transfer_result = handle(&mut store, params2, transfer_msg).unwrap();
        assert_eq!(transfer_result.messages.len(), 0);
        assert_eq!(transfer_result.log, Some("transfer successful".to_string()));

        // New state (unchanged)
        assert_eq!(
            get_balance(&store, "0000000000000000000000000000000000000000"),
            11
        );
        assert_eq!(
            get_balance(&store, "1111111111111111111111111111111111111111"),
            22
        );
        assert_eq!(
            get_balance(&store, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            33
        );
        assert_eq!(get_total_supply(&store), 66);
    }

    #[test]
    fn fails_on_insufficient_balance() {
        let mut store = MockStorage::new();
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height("0000000000000000000000000000000000000000", 450, 550);
        let res = init(&mut store, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Initial state
        assert_eq!(
            get_balance(&store, "0000000000000000000000000000000000000000"),
            11
        );
        assert_eq!(
            get_balance(&store, "1111111111111111111111111111111111111111"),
            22
        );
        assert_eq!(
            get_balance(&store, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            33
        );
        assert_eq!(get_total_supply(&store), 66);

        // Transfer
        let transfer_msg = to_vec(&HandleMsg::Transfer {
            recipient: "1111111111111111111111111111111111111111".to_string(),
            amount: 12,
        })
        .unwrap();
        let params2 = mock_params_height("0000000000000000000000000000000000000000", 450, 550);
        let transfer_result = handle(&mut store, params2, transfer_msg);
        match transfer_result {
            Ok(_) => panic!("expected error"),
            Err(Error::DynContractErr { msg, .. }) => {
                assert_eq!(msg, "Insufficient funds: balance=11, required=12")
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }

        // New state (unchanged)
        assert_eq!(
            get_balance(&store, "0000000000000000000000000000000000000000"),
            11
        );
        assert_eq!(
            get_balance(&store, "1111111111111111111111111111111111111111"),
            22
        );
        assert_eq!(
            get_balance(&store, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            33
        );
        assert_eq!(get_total_supply(&store), 66);
    }
}

mod approve {
    use super::*;

    fn make_init_msg() -> InitMsg {
        return InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: vec![
                InitialBalance {
                    address: "0000000000000000000000000000000000000000".to_string(),
                    amount: 11,
                },
                InitialBalance {
                    address: "1111111111111111111111111111111111111111".to_string(),
                    amount: 22,
                },
                InitialBalance {
                    address: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                    amount: 33,
                },
            ],
        };
    }

    fn make_spender() -> String {
        "dadadadadadadadadadadadadadadadadadadada".to_string()
    }

    #[test]
    fn has_zero_allowance_by_default() {
        let mut store = MockStorage::new();
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height("0000000000000000000000000000000000000000", 450, 550);
        let res = init(&mut store, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Existing owner
        assert_eq!(
            get_allowance(
                &store,
                "0000000000000000000000000000000000000000",
                &make_spender()
            ),
            0
        );

        // Non-existing owner
        assert_eq!(
            get_allowance(
                &store,
                "ab41312435245436564767134131243524565647",
                &make_spender()
            ),
            0
        );
    }

    #[test]
    fn can_set_allowance() {
        let mut store = MockStorage::new();
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height("0000000000000000000000000000000000000000", 450, 550);
        let res = init(&mut store, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        assert_eq!(
            get_allowance(
                &store,
                "aaccdd2323232332aaccdd2323232332ddff3322",
                &make_spender()
            ),
            0
        );

        // First approval
        let approve_msg1 = to_vec(&HandleMsg::Approve {
            spender: make_spender(),
            amount: 334422,
        })
        .unwrap();
        let params2 = mock_params_height("aaccdd2323232332aaccdd2323232332ddff3322", 450, 550);
        let transfer_result = handle(&mut store, params2, approve_msg1).unwrap();
        assert_eq!(transfer_result.messages.len(), 0);
        assert_eq!(transfer_result.log, Some("approve successful".to_string()));

        assert_eq!(
            get_allowance(
                &store,
                "aaccdd2323232332aaccdd2323232332ddff3322",
                &make_spender()
            ),
            334422
        );

        // Updated approval
        let approve_msg2 = to_vec(&HandleMsg::Approve {
            spender: make_spender(),
            amount: 777888,
        })
        .unwrap();
        let params3 = mock_params_height("aaccdd2323232332aaccdd2323232332ddff3322", 450, 550);
        let transfer_result = handle(&mut store, params3, approve_msg2).unwrap();
        assert_eq!(transfer_result.messages.len(), 0);
        assert_eq!(transfer_result.log, Some("approve successful".to_string()));

        assert_eq!(
            get_allowance(
                &store,
                "aaccdd2323232332aaccdd2323232332ddff3322",
                &make_spender()
            ),
            777888
        );
    }
}

mod transfer_from {
    use super::*;

    fn make_init_msg() -> InitMsg {
        return InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: vec![
                InitialBalance {
                    address: "0000000000000000000000000000000000000000".to_string(),
                    amount: 11,
                },
                InitialBalance {
                    address: "1111111111111111111111111111111111111111".to_string(),
                    amount: 22,
                },
                InitialBalance {
                    address: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                    amount: 33,
                },
            ],
        };
    }

    fn make_spender() -> String {
        "dadadadadadadadadadadadadadadadadadadada".to_string()
    }

    #[test]
    fn works() {
        let mut store = MockStorage::new();
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height("0000000000000000000000000000000000000000", 450, 550);
        let res = init(&mut store, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let owner = "0000000000000000000000000000000000000000";
        let spender = &make_spender();
        let recipient = "1212121212121212121212121212121212121212";

        // Set approval
        let approve_msg = to_vec(&HandleMsg::Approve {
            spender: make_spender(),
            amount: 4,
        })
        .unwrap();
        let params2 = mock_params_height(owner, 450, 550);
        let approve_result = handle(&mut store, params2, approve_msg).unwrap();
        assert_eq!(approve_result.messages.len(), 0);
        assert_eq!(approve_result.log, Some("approve successful".to_string()));

        assert_eq!(get_balance(&store, owner), 11);
        assert_eq!(get_allowance(&store, owner, spender), 4);

        // Transfer less than allowance but more than balance
        let fransfer_from_msg = to_vec(&HandleMsg::TransferFrom {
            owner: owner.to_string(),
            recipient: recipient.to_string(),
            amount: 3,
        })
        .unwrap();
        let params3 = mock_params_height(spender, 450, 550);
        let transfer_result = handle(&mut store, params3, fransfer_from_msg).unwrap();
        assert_eq!(transfer_result.messages.len(), 0);
        assert_eq!(
            transfer_result.log,
            Some("transfer from successful".to_string())
        );

        // State changed
        assert_eq!(get_balance(&store, owner), 8);
        assert_eq!(get_allowance(&store, owner, spender), 1);
    }

    #[test]
    fn fails_when_allowance_too_low() {
        let mut store = MockStorage::new();
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height("0000000000000000000000000000000000000000", 450, 550);
        let res = init(&mut store, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let owner = "0000000000000000000000000000000000000000";
        let spender = &make_spender();
        let recipient = "1212121212121212121212121212121212121212";

        // Set approval
        let approve_msg = to_vec(&HandleMsg::Approve {
            spender: make_spender(),
            amount: 2,
        })
        .unwrap();
        let params2 = mock_params_height(owner, 450, 550);
        let approve_result = handle(&mut store, params2, approve_msg).unwrap();
        assert_eq!(approve_result.messages.len(), 0);
        assert_eq!(approve_result.log, Some("approve successful".to_string()));

        assert_eq!(get_balance(&store, owner), 11);
        assert_eq!(get_allowance(&store, owner, spender), 2);

        // Transfer less than allowance but more than balance
        let fransfer_from_msg = to_vec(&HandleMsg::TransferFrom {
            owner: owner.to_string(),
            recipient: recipient.to_string(),
            amount: 3,
        })
        .unwrap();
        let params3 = mock_params_height(spender, 450, 550);
        let transfer_result = handle(&mut store, params3, fransfer_from_msg);
        match transfer_result {
            Ok(_) => panic!("expected error"),
            Err(Error::DynContractErr { msg, .. }) => {
                assert_eq!(msg, "Insufficient allowance: allowance=2, required=3")
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }

        // TOOD: is it in scope to test state after execution errors?
        // State unchanged
        // assert_eq!(get_balance(&store, owner), 11);
        // assert_eq!(get_allowance(&store, owner, spender), 2);
    }

    #[test]
    fn fails_when_allowance_is_set_but_balance_too_low() {
        let mut store = MockStorage::new();
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height("0000000000000000000000000000000000000000", 450, 550);
        let res = init(&mut store, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let owner = "0000000000000000000000000000000000000000";
        let spender = &make_spender();
        let recipient = "1212121212121212121212121212121212121212";

        // Set approval
        let approve_msg = to_vec(&HandleMsg::Approve {
            spender: make_spender(),
            amount: 20,
        })
        .unwrap();
        let params2 = mock_params_height(owner, 450, 550);
        let approve_result = handle(&mut store, params2, approve_msg).unwrap();
        assert_eq!(approve_result.messages.len(), 0);
        assert_eq!(approve_result.log, Some("approve successful".to_string()));

        assert_eq!(get_balance(&store, owner), 11);
        assert_eq!(get_allowance(&store, owner, spender), 20);

        // Transfer less than allowance but more than balance
        let fransfer_from_msg = to_vec(&HandleMsg::TransferFrom {
            owner: owner.to_string(),
            recipient: recipient.to_string(),
            amount: 15,
        })
        .unwrap();
        let params3 = mock_params_height(spender, 450, 550);
        let transfer_result = handle(&mut store, params3, fransfer_from_msg);
        match transfer_result {
            Ok(_) => panic!("expected error"),
            Err(Error::DynContractErr { msg, .. }) => {
                assert_eq!(msg, "Insufficient funds: balance=11, required=15")
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }

        // TOOD: is it in scope to test state after execution errors?
        // State unchanged
        // assert_eq!(get_balance(&store, owner), 11);
        // assert_eq!(get_allowance(&store, owner, spender), 20);
    }
}
