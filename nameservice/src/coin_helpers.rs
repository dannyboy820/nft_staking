use cosmwasm::errors::{contract_err, Result};
pub use cosmwasm::types::coin as coin_vec;
use cosmwasm::types::Coin;

fn parse_u128(source: &str) -> Result<u128> {
    match source.parse::<u128>() {
        Ok(value) => Ok(value),
        Err(_) => contract_err("Error while parsing string to u128"),
    }
}

pub fn assert_sent_sufficient_coin(sent: &Option<Vec<Coin>>, required: Option<Coin>) -> Result<()> {
    if let Some(required_coin) = required {
        let required_amount = parse_u128(&required_coin.amount)?;
        if required_amount > 0 {
            if let Some(coins) = sent {
                if coins.iter().any(|coin| {
                    // check if a given sent coin matches denom
                    // and has sufficient amount
                    let amount = parse_u128(&coin.amount).unwrap_or(0);
                    coin.denom == required_coin.denom && amount >= required_amount
                }) {
                    return Ok(());
                }
            }
            return contract_err("Insufficient funds sent");
        }
    }
    Ok(())
}

pub fn coin(amount: &str, denom: &str) -> Coin {
    Coin {
        amount: amount.to_string(),
        denom: denom.to_string(),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm::errors::Error;

    #[test]
    fn assert_sent_sufficient_coin_works() {
        match assert_sent_sufficient_coin(&None, Some(coin("0", "token"))) {
            Ok(()) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        };

        match assert_sent_sufficient_coin(&None, Some(coin("5", "token"))) {
            Ok(()) => panic!("Should have raised insufficient funds error"),
            Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "Insufficient funds sent"),
            Err(e) => panic!("Unexpected error: {:?}", e),
        };

        match assert_sent_sufficient_coin(&Some(coin_vec("10", "smokin")), Some(coin("5", "token")))
        {
            Ok(()) => panic!("Should have raised insufficient funds error"),
            Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "Insufficient funds sent"),
            Err(e) => panic!("Unexpected error: {:?}", e),
        };

        match assert_sent_sufficient_coin(&Some(coin_vec("10", "token")), Some(coin("5", "token")))
        {
            Ok(()) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        };

        let sent_coins = Some(vec![
            coin("2", "smokin"),
            coin("5", "token"),
            coin("1", "earth"),
        ]);

        match assert_sent_sufficient_coin(&sent_coins, Some(coin("5", "token"))) {
            Ok(()) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        };
    }

    #[test]
    fn assert_sent_sufficient_coin_handles_parse_failure() {
        match assert_sent_sufficient_coin(&None, Some(coin("ff", "token"))) {
            Ok(()) => panic!("Should have raised parse error"),
            Err(Error::ContractErr { msg, .. }) => {
                assert_eq!(msg, "Error while parsing string to u128")
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        };

        let sent_coins = Some(vec![coin("abcd", "smokin"), coin("5", "token")]);

        match assert_sent_sufficient_coin(&sent_coins, Some(coin("5", "token"))) {
            Ok(()) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        };

        let sent_coins = Some(vec![coin("abcd", "smokin"), coin("efg", "token")]);

        match assert_sent_sufficient_coin(&sent_coins, Some(coin("5", "token"))) {
            Ok(()) => panic!("Should have raised parse error"),
            Err(Error::ContractErr { msg, .. }) => assert_eq!(msg, "Insufficient funds sent"),
            Err(e) => panic!("Unexpected error: {:?}", e),
        };
    }
}
