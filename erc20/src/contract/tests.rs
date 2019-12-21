use cosmwasm::errors::Error;
use cosmwasm::mock::{dependencies, mock_params};
use cosmwasm::serde::to_vec;
use cosmwasm::traits::{Api, Extern, Storage};
use cosmwasm::types::Params;
use std::convert::TryInto;

use super::{
    bytes_to_u128, handle, init, read_u128, HandleMsg, InitMsg, InitialBalance, KEY_DECIMALS,
    KEY_NAME, KEY_SYMBOL, KEY_TOTAL_SUPPLY, PREFIX_ALLOWANCES, PREFIX_BALANCES, PREFIX_CONFIG,
};

static CANONICAL_LENGTH: usize = 20;

fn mock_params_height<A: Api>(api: &A, signer: &str, height: i64, time: i64) -> Params {
    let mut params = mock_params(api, signer, &[], &[]);
    params.block.height = height;
    params.block.time = time;
    params
}

fn get_name<T: Storage>(store: &T) -> String {
    let key = [
        &[0u8],
        &[PREFIX_CONFIG.len() as u8] as &[u8],
        PREFIX_CONFIG,
        KEY_NAME,
    ]
    .concat();
    let data = store.get(&key).expect("no name data stored");
    return String::from_utf8(data).unwrap();
}

fn get_symbol<T: Storage>(store: &T) -> String {
    let key = [
        &[0u8],
        &[PREFIX_CONFIG.len() as u8] as &[u8],
        PREFIX_CONFIG,
        KEY_SYMBOL,
    ]
    .concat();
    let data = store.get(&key).expect("no symbol data stored");
    return String::from_utf8(data).unwrap();
}

fn get_decimals<T: Storage>(store: &T) -> u8 {
    let key = [
        &[0u8],
        &[PREFIX_CONFIG.len() as u8] as &[u8],
        PREFIX_CONFIG,
        KEY_DECIMALS,
    ]
    .concat();
    let data = store.get(&key).expect("no decimals data stored");
    return u8::from_be_bytes(data[0..1].try_into().unwrap());
}

fn get_total_supply<T: Storage>(store: &T) -> u128 {
    let key = [
        &[0u8],
        &[PREFIX_CONFIG.len() as u8] as &[u8],
        PREFIX_CONFIG,
        KEY_TOTAL_SUPPLY,
    ]
    .concat();
    let data = store.get(&key).expect("no decimals data stored");
    return bytes_to_u128(&data).unwrap();
}

fn get_balance<S: Storage, A: Api>(deps: &Extern<S, A>, address: &str) -> u128 {
    let address_key = deps
        .api
        .canonical_address(address)
        .expect("canonical_address failed");
    let key = [
        &[0u8],
        &[PREFIX_BALANCES.len() as u8] as &[u8],
        PREFIX_BALANCES,
        &address_key[..],
    ]
    .concat();
    return read_u128(&deps.storage, &key).unwrap();
}

fn get_allowance<S: Storage, A: Api>(deps: &Extern<S, A>, owner: &str, spender: &str) -> u128 {
    let owner_raw_address = deps
        .api
        .canonical_address(owner)
        .expect("canonical_address failed");
    let spender_raw_address = deps
        .api
        .canonical_address(spender)
        .expect("canonical_address failed");
    let key = [
        &[0u8],
        &[PREFIX_ALLOWANCES.len() as u8] as &[u8],
        PREFIX_ALLOWANCES,
        &owner_raw_address[..],
        &spender_raw_address[..],
    ]
    .concat();
    return read_u128(&deps.storage, &key).unwrap();
}

mod init {
    use super::*;

    #[test]
    fn works() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: [InitialBalance {
                address: "addr0000".to_string(),
                amount: "11223344".to_string(),
            }]
            .to_vec(),
        })
        .unwrap();
        let params = mock_params_height(&deps.api, "creator", 450, 550);
        let res = init(&mut deps, params, msg).unwrap();
        assert_eq!(0, res.messages.len());

        assert_eq!(get_name(&deps.storage), "Cash Token");
        assert_eq!(get_symbol(&deps.storage), "CASH");
        assert_eq!(get_decimals(&deps.storage), 9);
        assert_eq!(get_balance(&deps, "addr0000"), 11223344);
        assert_eq!(get_total_supply(&deps.storage), 11223344);
    }

    #[test]
    fn works_with_empty_balance() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: [].to_vec(),
        })
        .unwrap();
        let params = mock_params_height(&deps.api, "creator", 450, 550);
        let res = init(&mut deps, params, msg).unwrap();
        assert_eq!(0, res.messages.len());

        assert_eq!(get_total_supply(&deps.storage), 0);
    }

    #[test]
    fn works_with_multiple_balances() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: [
                InitialBalance {
                    address: "addr0000".to_string(),
                    amount: "11".to_string(),
                },
                InitialBalance {
                    address: "addr1111".to_string(),
                    amount: "22".to_string(),
                },
                InitialBalance {
                    address: "addrbbbb".to_string(),
                    amount: "33".to_string(),
                },
            ]
            .to_vec(),
        })
        .unwrap();
        let params = mock_params_height(&deps.api, "creator", 450, 550);
        let res = init(&mut deps, params, msg).unwrap();
        assert_eq!(0, res.messages.len());

        assert_eq!(get_balance(&deps, "addr0000"), 11);
        assert_eq!(get_balance(&deps, "addr1111"), 22);
        assert_eq!(get_balance(&deps, "addrbbbb"), 33);
        assert_eq!(get_total_supply(&deps.storage), 66);
    }

    #[test]
    fn works_with_balance_larger_than_53_bit() {
        let mut deps = dependencies(CANONICAL_LENGTH);

        // This value cannot be represented precisely in JavaScript and jq. Both
        //   node -e "console.log(9007199254740993)"
        //   echo '{ "value": 9007199254740993 }' | jq
        // return 9007199254740992
        let msg = to_vec(&InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: [InitialBalance {
                address: "addr0000".to_string(),
                amount: "9007199254740993".to_string(),
            }]
            .to_vec(),
        })
        .unwrap();
        let params = mock_params_height(&deps.api, "creator", 450, 550);
        let res = init(&mut deps, params, msg).unwrap();
        assert_eq!(0, res.messages.len());

        assert_eq!(get_name(&deps.storage), "Cash Token");
        assert_eq!(get_symbol(&deps.storage), "CASH");
        assert_eq!(get_decimals(&deps.storage), 9);
        assert_eq!(get_balance(&deps, "addr0000"), 9007199254740993);
        assert_eq!(get_total_supply(&deps.storage), 9007199254740993);
    }

    #[test]
    // Typical supply like 100 million tokens with 18 decimals exceeds the 64 bit range
    fn works_with_balance_larger_than_64_bit() {
        let mut deps = dependencies(CANONICAL_LENGTH);

        let msg = to_vec(&InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: [InitialBalance {
                address: "addr0000".to_string(),
                amount: "100000000000000000000000000".to_string(),
            }]
            .to_vec(),
        })
        .unwrap();
        let params = mock_params_height(&deps.api, "creator", 450, 550);
        let res = init(&mut deps, params, msg).unwrap();
        assert_eq!(0, res.messages.len());

        assert_eq!(get_name(&deps.storage), "Cash Token");
        assert_eq!(get_symbol(&deps.storage), "CASH");
        assert_eq!(get_decimals(&deps.storage), 9);
        assert_eq!(get_balance(&deps, "addr0000"), 100000000000000000000000000);
        assert_eq!(get_total_supply(&deps.storage), 100000000000000000000000000);
    }

    #[test]
    fn fails_for_balance_larger_than_max_u128() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: [InitialBalance {
                address: "addr0000".to_string(),
                // 2**128 = 340282366920938463463374607431768211456
                amount: "340282366920938463463374607431768211456".to_string(),
            }]
            .to_vec(),
        })
        .unwrap();
        let params = mock_params_height(&deps.api, "creator", 450, 550);
        let result = init(&mut deps, params, msg);
        match result {
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => {
                assert_eq!(msg, "Error while parsing decimal string to u128")
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn fails_for_large_decimals() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 42,
            initial_balances: [].to_vec(),
        })
        .unwrap();
        let params = mock_params_height(&deps.api, "creator", 450, 550);
        let result = init(&mut deps, params, msg);
        match result {
            Ok(_) => panic!("expected error"),
            Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "Decimals must not exceed 18"),
            Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn fails_for_name_too_short() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&InitMsg {
            name: "CC".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: [].to_vec(),
        })
        .unwrap();
        let params = mock_params_height(&deps.api, "creator", 450, 550);
        let result = init(&mut deps, params, msg);
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
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&InitMsg {
            name: "Cash coin. Cash coin. Cash coin. Cash coin.".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            initial_balances: [].to_vec(),
        })
        .unwrap();
        let params = mock_params_height(&deps.api, "creator", 450, 550);
        let result = init(&mut deps, params, msg);
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
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&InitMsg {
            name: "De De".to_string(),
            symbol: "DD".to_string(),
            decimals: 9,
            initial_balances: [].to_vec(),
        })
        .unwrap();
        let params = mock_params_height(&deps.api, "creator", 450, 550);
        let result = init(&mut deps, params, msg);
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
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&InitMsg {
            name: "Super Coin".to_string(),
            symbol: "SUPERCOIN".to_string(),
            decimals: 9,
            initial_balances: [].to_vec(),
        })
        .unwrap();
        let params = mock_params_height(&deps.api, "creator", 450, 550);
        let result = init(&mut deps, params, msg);
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
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&InitMsg {
            name: "Cash Token".to_string(),
            symbol: "CaSH".to_string(),
            decimals: 9,
            initial_balances: [].to_vec(),
        })
        .unwrap();
        let params = mock_params_height(&deps.api, "creator", 450, 550);
        let result = init(&mut deps, params, msg);
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
                    address: "addr0000".to_string(),
                    amount: "11".to_string(),
                },
                InitialBalance {
                    address: "addr1111".to_string(),
                    amount: "22".to_string(),
                },
                InitialBalance {
                    address: "addrbbbb".to_string(),
                    amount: "33".to_string(),
                },
            ],
        };
    }

    #[test]
    fn can_send_to_existing_recipient() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height(&deps.api, "addr0000", 450, 550);
        let res = init(&mut deps, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Initial state
        assert_eq!(get_balance(&deps, "addr0000"), 11);
        assert_eq!(get_balance(&deps, "addr1111"), 22);
        assert_eq!(get_balance(&deps, "addrbbbb"), 33);
        assert_eq!(get_total_supply(&deps.storage), 66);

        // Transfer
        let transfer_msg = to_vec(&HandleMsg::Transfer {
            recipient: "addr1111".to_string(),
            amount: "1".to_string(),
        })
        .unwrap();
        let params2 = mock_params_height(&deps.api, "addr0000", 450, 550);
        let transfer_result = handle(&mut deps, params2, transfer_msg).unwrap();
        assert_eq!(transfer_result.messages.len(), 0);
        assert_eq!(transfer_result.log, Some("transfer successful".to_string()));

        // New state
        assert_eq!(get_balance(&deps, "addr0000"), 10); // -1
        assert_eq!(get_balance(&deps, "addr1111"), 23); // +1
        assert_eq!(get_balance(&deps, "addrbbbb"), 33);
        assert_eq!(get_total_supply(&deps.storage), 66);
    }

    #[test]
    fn can_send_to_non_existent_recipient() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height(&deps.api, "addr0000", 450, 550);
        let res = init(&mut deps, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Initial state
        assert_eq!(get_balance(&deps, "addr0000"), 11);
        assert_eq!(get_balance(&deps, "addr1111"), 22);
        assert_eq!(get_balance(&deps, "addrbbbb"), 33);
        assert_eq!(get_total_supply(&deps.storage), 66);

        // Transfer
        let transfer_msg = to_vec(&HandleMsg::Transfer {
            recipient: "addr2323".to_string(),
            amount: "1".to_string(),
        })
        .unwrap();
        let params2 = mock_params_height(&deps.api, "addr0000", 450, 550);
        let transfer_result = handle(&mut deps, params2, transfer_msg).unwrap();
        assert_eq!(transfer_result.messages.len(), 0);
        assert_eq!(transfer_result.log, Some("transfer successful".to_string()));

        // New state
        assert_eq!(get_balance(&deps, "addr0000"), 10);
        assert_eq!(get_balance(&deps, "addr1111"), 22);
        assert_eq!(get_balance(&deps, "addr2323"), 1);
        assert_eq!(get_balance(&deps, "addrbbbb"), 33);
        assert_eq!(get_total_supply(&deps.storage), 66);
    }

    #[test]
    fn can_send_zero_amount() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height(&deps.api, "addr0000", 450, 550);
        let res = init(&mut deps, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Initial state
        assert_eq!(get_balance(&deps, "addr0000"), 11);
        assert_eq!(get_balance(&deps, "addr1111"), 22);
        assert_eq!(get_balance(&deps, "addrbbbb"), 33);
        assert_eq!(get_total_supply(&deps.storage), 66);

        // Transfer
        let transfer_msg = to_vec(&HandleMsg::Transfer {
            recipient: "addr1111".to_string(),
            amount: "0".to_string(),
        })
        .unwrap();
        let params2 = mock_params_height(&deps.api, "addr0000", 450, 550);
        let transfer_result = handle(&mut deps, params2, transfer_msg).unwrap();
        assert_eq!(transfer_result.messages.len(), 0);
        assert_eq!(transfer_result.log, Some("transfer successful".to_string()));

        // New state (unchanged)
        assert_eq!(get_balance(&deps, "addr0000"), 11);
        assert_eq!(get_balance(&deps, "addr1111"), 22);
        assert_eq!(get_balance(&deps, "addrbbbb"), 33);
        assert_eq!(get_total_supply(&deps.storage), 66);
    }

    #[test]
    fn fails_on_insufficient_balance() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height(&deps.api, "addr0000", 450, 550);
        let res = init(&mut deps, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Initial state
        assert_eq!(get_balance(&deps, "addr0000"), 11);
        assert_eq!(get_balance(&deps, "addr1111"), 22);
        assert_eq!(get_balance(&deps, "addrbbbb"), 33);
        assert_eq!(get_total_supply(&deps.storage), 66);

        // Transfer
        let transfer_msg = to_vec(&HandleMsg::Transfer {
            recipient: "addr1111".to_string(),
            amount: "12".to_string(),
        })
        .unwrap();
        let params2 = mock_params_height(&deps.api, "addr0000", 450, 550);
        let transfer_result = handle(&mut deps, params2, transfer_msg);
        match transfer_result {
            Ok(_) => panic!("expected error"),
            Err(Error::DynContractErr { msg, .. }) => {
                assert_eq!(msg, "Insufficient funds: balance=11, required=12")
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }

        // New state (unchanged)
        assert_eq!(get_balance(&deps, "addr0000"), 11);
        assert_eq!(get_balance(&deps, "addr1111"), 22);
        assert_eq!(get_balance(&deps, "addrbbbb"), 33);
        assert_eq!(get_total_supply(&deps.storage), 66);
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
                    address: "addr0000".to_string(),
                    amount: "11".to_string(),
                },
                InitialBalance {
                    address: "addr1111".to_string(),
                    amount: "22".to_string(),
                },
                InitialBalance {
                    address: "addrbbbb".to_string(),
                    amount: "33".to_string(),
                },
            ],
        };
    }

    fn make_spender() -> String {
        "dadadadadadadada".to_string()
    }

    #[test]
    fn has_zero_allowance_by_default() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height(&deps.api, "addr0000", 450, 550);
        let res = init(&mut deps, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Existing owner
        assert_eq!(get_allowance(&deps, "addr0000", &make_spender()), 0);

        // Non-existing owner
        assert_eq!(get_allowance(&deps, "addr4567", &make_spender()), 0);
    }

    #[test]
    fn can_set_allowance() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height(&deps.api, "addr0000", 450, 550);
        let res = init(&mut deps, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        assert_eq!(get_allowance(&deps, "addr7654", &make_spender()), 0);

        // First approval
        let approve_msg1 = to_vec(&HandleMsg::Approve {
            spender: make_spender(),
            amount: "334422".to_string(),
        })
        .unwrap();
        let params2 = mock_params_height(&deps.api, "addr7654", 450, 550);
        let transfer_result = handle(&mut deps, params2, approve_msg1).unwrap();
        assert_eq!(transfer_result.messages.len(), 0);
        assert_eq!(transfer_result.log, Some("approve successful".to_string()));

        assert_eq!(get_allowance(&deps, "addr7654", &make_spender()), 334422);

        // Updated approval
        let approve_msg2 = to_vec(&HandleMsg::Approve {
            spender: make_spender(),
            amount: "777888".to_string(),
        })
        .unwrap();
        let params3 = mock_params_height(&deps.api, "addr7654", 450, 550);
        let transfer_result = handle(&mut deps, params3, approve_msg2).unwrap();
        assert_eq!(transfer_result.messages.len(), 0);
        assert_eq!(transfer_result.log, Some("approve successful".to_string()));

        assert_eq!(get_allowance(&deps, "addr7654", &make_spender()), 777888);
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
                    address: "addr0000".to_string(),
                    amount: "11".to_string(),
                },
                InitialBalance {
                    address: "addr1111".to_string(),
                    amount: "22".to_string(),
                },
                InitialBalance {
                    address: "addrbbbb".to_string(),
                    amount: "33".to_string(),
                },
            ],
        };
    }

    fn make_spender() -> String {
        "dadadadadadadada".to_string()
    }

    #[test]
    fn works() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height(&deps.api, "addr0000", 450, 550);
        let res = init(&mut deps, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let owner = "addr0000";
        let spender = &make_spender();
        let recipient = "addr1212";

        // Set approval
        let approve_msg = to_vec(&HandleMsg::Approve {
            spender: make_spender(),
            amount: "4".to_string(),
        })
        .unwrap();
        let params2 = mock_params_height(&deps.api, owner, 450, 550);
        let approve_result = handle(&mut deps, params2, approve_msg).unwrap();
        assert_eq!(approve_result.messages.len(), 0);
        assert_eq!(approve_result.log, Some("approve successful".to_string()));

        assert_eq!(get_balance(&deps, owner), 11);
        assert_eq!(get_allowance(&deps, owner, spender), 4);

        // Transfer less than allowance but more than balance
        let fransfer_from_msg = to_vec(&HandleMsg::TransferFrom {
            owner: owner.to_string(),
            recipient: recipient.to_string(),
            amount: "3".to_string(),
        })
        .unwrap();
        let params3 = mock_params_height(&deps.api, spender, 450, 550);
        let transfer_result = handle(&mut deps, params3, fransfer_from_msg).unwrap();
        assert_eq!(transfer_result.messages.len(), 0);
        assert_eq!(
            transfer_result.log,
            Some("transfer from successful".to_string())
        );

        // State changed
        assert_eq!(get_balance(&deps, owner), 8);
        assert_eq!(get_allowance(&deps, owner, spender), 1);
    }

    #[test]
    fn fails_when_allowance_too_low() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height(&deps.api, "addr0000", 450, 550);
        let res = init(&mut deps, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let owner = "addr0000";
        let spender = &make_spender();
        let recipient = "addr1212";

        // Set approval
        let approve_msg = to_vec(&HandleMsg::Approve {
            spender: make_spender(),
            amount: "2".to_string(),
        })
        .unwrap();
        let params2 = mock_params_height(&deps.api, owner, 450, 550);
        let approve_result = handle(&mut deps, params2, approve_msg).unwrap();
        assert_eq!(approve_result.messages.len(), 0);
        assert_eq!(approve_result.log, Some("approve successful".to_string()));

        assert_eq!(get_balance(&deps, owner), 11);
        assert_eq!(get_allowance(&deps, owner, spender), 2);

        // Transfer less than allowance but more than balance
        let fransfer_from_msg = to_vec(&HandleMsg::TransferFrom {
            owner: owner.to_string(),
            recipient: recipient.to_string(),
            amount: "3".to_string(),
        })
        .unwrap();
        let params3 = mock_params_height(&deps.api, spender, 450, 550);
        let transfer_result = handle(&mut deps, params3, fransfer_from_msg);
        match transfer_result {
            Ok(_) => panic!("expected error"),
            Err(Error::DynContractErr { msg, .. }) => {
                assert_eq!(msg, "Insufficient allowance: allowance=2, required=3")
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }

        // TOOD: is it in scope to test state after execution errors?
        // State unchanged
        // assert_eq!(get_balance(&deps, owner), 11);
        // assert_eq!(get_allowance(&deps, owner, spender), 2);
    }

    #[test]
    fn fails_when_allowance_is_set_but_balance_too_low() {
        let mut deps = dependencies(CANONICAL_LENGTH);
        let msg = to_vec(&make_init_msg()).unwrap();
        let params1 = mock_params_height(&deps.api, "addr0000", 450, 550);
        let res = init(&mut deps, params1, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let owner = "addr0000";
        let spender = &make_spender();
        let recipient = "addr1212";

        // Set approval
        let approve_msg = to_vec(&HandleMsg::Approve {
            spender: make_spender(),
            amount: "20".to_string(),
        })
        .unwrap();
        let params2 = mock_params_height(&deps.api, owner, 450, 550);
        let approve_result = handle(&mut deps, params2, approve_msg).unwrap();
        assert_eq!(approve_result.messages.len(), 0);
        assert_eq!(approve_result.log, Some("approve successful".to_string()));

        assert_eq!(get_balance(&deps, owner), 11);
        assert_eq!(get_allowance(&deps, owner, spender), 20);

        // Transfer less than allowance but more than balance
        let fransfer_from_msg = to_vec(&HandleMsg::TransferFrom {
            owner: owner.to_string(),
            recipient: recipient.to_string(),
            amount: "15".to_string(),
        })
        .unwrap();
        let params3 = mock_params_height(&deps.api, spender, 450, 550);
        let transfer_result = handle(&mut deps, params3, fransfer_from_msg);
        match transfer_result {
            Ok(_) => panic!("expected error"),
            Err(Error::DynContractErr { msg, .. }) => {
                assert_eq!(msg, "Insufficient funds: balance=11, required=15")
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }

        // TOOD: is it in scope to test state after execution errors?
        // State unchanged
        // assert_eq!(get_balance(&deps, owner), 11);
        // assert_eq!(get_allowance(&deps, owner, spender), 20);
    }
}
