use std::vec;

use cosmwasm_std::{
    entry_point, Addr, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128, StdError, from_binary, Storage, WasmMsg, to_binary, CosmosMsg
};
use cw721::Cw721ReceiveMsg;

use crate::error::ContractError;
use crate::msg::{ ExecuteMsg, InstantiateMsg, CreateCollectionPoolMsg, UpdateCollectionPoolMsg, UpdateContractInfoMsg, DepositeMsg};
use crate::state::{ContractInfo, CONTRACT_INFO, COLLECTION_POOL_INFO, STAKING_INFO, CollectionPoolInfo, StakerInfo, CollectionStakedTokenInfo};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let mut admin = info.sender.to_string();

    if msg.admin.is_some() {
        admin = msg.admin.unwrap()
    }

    let config = ContractInfo {
        source: info.sender,
        end_height: msg.end_height,
        end_time: msg.end_time,
        admin: Some(admin),
        nft_721_contract_addr_whitelist: msg.nft_721_contract_addr_whitelist,
    };

    if config.is_expired(&env) {
        return Err(ContractError::Expired {
            end_height: msg.end_height,
            end_time: msg.end_time,
        });
    }

    CONTRACT_INFO.save(deps.storage, &config)?;
    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateContractInfo ( msg ) => try_update_contract_info(deps, info, msg),
        ExecuteMsg::CreateCollectionPool(msg) => try_create_collection_pool_info(deps, env, info, msg),
        ExecuteMsg::UpdateCollectionPool(msg) => try_update_collection_pool_info(deps, info, msg),
        ExecuteMsg::ReceiveNft(receive_msg) => try_receive_721(deps, env, info, receive_msg),
        ExecuteMsg::Withdraw { collection_id, withdraw_rewards, withdraw_nft_ids } => try_withdraw(deps, env, info, collection_id, withdraw_rewards, withdraw_nft_ids),
        // ExecuteMsg::Claim { collection_id } => todo!(),
        // ExecuteMsg::Refund {  } => todo!(),
    }
}

fn try_withdraw(
    deps: DepsMut, 
    env: Env, 
    info: MessageInfo, 
    collection_id: String, 
    withdraw_rewards: bool, 
    withdraw_nft_ids: Vec<String>
) -> Result<Response, ContractError> {
    let staker_info = STAKING_INFO.load(deps.storage, &info.sender.clone().as_bytes())?;

    let collection_pool_info = update_collection_pool(deps.storage, env.clone(), collection_id.clone())?;
    let current_pending = staker_info.total_staked * collection_pool_info.acc_per_share - staker_info.reward_debt;

    if current_pending.gt(&Uint128::from(0u128)) {
        STAKING_INFO.update(
            deps.storage,
            &info.sender.clone().as_bytes(),
            |data| {
                if let Some(mut old_info) = data {
                    if withdraw_rewards {
                        old_info.total_earned += current_pending;
                        old_info.pending = Uint128::from(0u128)
                    } else {
                        old_info.pending = current_pending;
                    }
                    Ok(old_info)
                } else {
                    Err(ContractError::Std(StdError::generic_err("Invalid update staker")))
                }
            }
        )?;
    }

    let mut withdraw_nfts = vec![];
    let mut left_nfts = vec![];

    staker_info
        .clone()
        .staked_tokens
        .into_iter()
        .for_each(|token| {
            let res = withdraw_nft_ids
                .clone()
                .into_iter()
                .find(|n| n.eq(&token.token_id.clone()));
            match res {
                Some(..) => withdraw_nfts.push(token.clone()),
                None => left_nfts.push(token.clone()),
            }
        });

    if withdraw_nfts.len() != withdraw_nft_ids.len() {
        return Err(ContractError::Std(StdError::generic_err("Invalid withdraw:  You are trying to withdraw some nfts that you haven't staken!")))
    }

    let mut num_of_withdraw_edition = Uint128::from(0u128);

    let mut cosmos_msgs: Vec<CosmosMsg> = vec![];

    // Transfer nfts back to staker
    for nft in withdraw_nfts {
        num_of_withdraw_edition += Uint128::from(1u128);
        cosmos_msgs.push(
            WasmMsg::Execute { 
                contract_addr: nft.contract_addr.to_string(), 
                msg: to_binary(&cw721::Cw721ExecuteMsg::TransferNft { 
                    recipient: info.sender.to_string(), 
                    token_id: nft.token_id.clone(), 
                })?, 
                funds: vec![] 
            }.into()
        );
    }

    STAKING_INFO.update(
        deps.storage,
        &info.sender.clone().as_bytes(),
        |data| {
            if let Some(mut old_info) = data {
                old_info.total_staked = old_info.total_staked - num_of_withdraw_edition;
                old_info.reward_debt = old_info.total_staked * collection_pool_info.acc_per_share;
                old_info.staked_tokens = left_nfts;
                Ok(old_info)
            } else {
                Err(ContractError::Std(StdError::generic_err("Invalid update staker info")))
            }
        }
    )?;

    COLLECTION_POOL_INFO.update(
        deps.storage,
        collection_pool_info.collection_id.as_bytes(),
        |data| {
            if let Some(mut old_info) = data {
                old_info.total_nfts = old_info.total_nfts - num_of_withdraw_edition;
                Ok(old_info)
            } else {
                return Err(ContractError::Std(StdError::generic_err("Invalid update collection pool info")));
            }
        }
    )?;

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attribute("action", "stake nfts")
    )

    // match staker_info {
    //     Some(user_info) => {
    //         if user_info.total_staked.le(&Uint128::from(0u128)) {
    //             return Err(ContractError::Std(StdError::generic_err(
    //                 "You have not stake any nfts"
    //             )));
    //         }

    //         let 
    //     }
    // }
}

fn try_receive_721(
    deps: DepsMut, 
    env: Env, 
    info: MessageInfo, 
    receive_msg: Cw721ReceiveMsg
) -> Result<Response, ContractError> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    let result = contract_info
        .nft_721_contract_addr_whitelist
        .into_iter()
        .find(|addr| addr.eq(&info.sender.to_string()));

    if result.is_none() {
        return Err(ContractError::Unauthorized { sender: info.sender.to_string() });
    }

    let deposit_msg = from_binary::<DepositeMsg>(&receive_msg.msg)?;

    let collection_pool_info 
        = COLLECTION_POOL_INFO.may_load(deps.storage, deposit_msg.collection_id.clone().as_bytes()).unwrap();
    
    if collection_pool_info.is_none() {
        return Err(ContractError::InvalidCollection {});
    }

    check_collection_is_expired(env.clone(), &collection_pool_info.clone().unwrap())?;


    // staking process...
    let mut collection_pool_info = update_collection_pool(deps.storage, env.clone(), deposit_msg.collection_id.clone())?;
    
    let staker_info = STAKING_INFO.may_load(deps.storage, &receive_msg.sender.clone().as_bytes())?;

    if let Some(staking_info) = staker_info {
        if staking_info.total_staked.gt(&Uint128::from(0u128)) {
            let pending = staking_info.total_staked * collection_pool_info.acc_per_share - staking_info.reward_debt + staking_info.pending;
            if pending.gt(&Uint128::from(0u128)) {
                STAKING_INFO.update(
                    deps.storage, 
                    &receive_msg.sender.clone().as_bytes(), 
                    |data| {
                        if let Some(mut info) = data {
                            if deposit_msg.withdraw_rewards {
                                info.total_earned += pending;
                                info.pending = Uint128::from(0u128);
                            } else {
                                info.pending = pending;
                            }
                            Ok(info)
                        } else {
                            return Err(StdError::generic_err("Invalid update collection staker info"));
                         }
                    })?;
            }
        }
    } else {
       let user_info  = StakerInfo{
            total_staked: Uint128::from(0u128),
            reward_debt: Uint128::from(0u128),
            pending: Uint128::from(0u128),
            total_earned: Uint128::from(0u128),
            staked_tokens: vec![],
        };

        STAKING_INFO.save(deps.storage, &receive_msg.sender.clone().as_bytes(), &user_info)?;        
    }

    // Update the total_staked_nft_editions for collection pool
    collection_pool_info = COLLECTION_POOL_INFO.update(
        deps.storage, 
        deposit_msg.collection_id.clone().as_bytes(),
        |data| {
            if let Some(mut collection_info) = data {
                collection_info.total_nfts += Uint128::from(1u128);
                Ok(collection_info)
            } else {
                return Err(StdError::generic_err("Invalid update collection info"));
            }
        })?;

    //4. Update staker's total_staked_nft_editions and reward debt and staked_nft
    STAKING_INFO.update(
        deps.storage, 
        &receive_msg.sender.clone().as_bytes(),
        |data| {
            if let Some(mut user_info) = data {
                user_info.total_staked += Uint128::from(1u128);
                user_info.reward_debt = user_info.total_staked * collection_pool_info.acc_per_share.clone();
                let nft = CollectionStakedTokenInfo{
                    token_id: receive_msg.token_id,
                    contract_addr: info.sender.clone()
                };
                user_info.staked_tokens.push(nft.clone());
                Ok(user_info)
            } else {
                return Err(StdError::generic_err("Invalid update4 collection staker info"));
            }
        }
    )?;

    // let collection_staker_info_response = 
    Ok(Response::default())
}

fn try_update_collection_pool_info(
    deps: DepsMut, 
    info: MessageInfo, 
    msg: UpdateCollectionPoolMsg
) -> Result<Response, ContractError> {
    check_admin_permission(deps.as_ref(), &info.sender)?;

    COLLECTION_POOL_INFO.update(
        deps.storage, 
        msg.collection_id.clone().as_bytes(), 
        | data | {
            if let Some(mut collection_pool_info) = data {
                if let Some(reward_per_block) = msg.reward_per_block.clone() {
                    if reward_per_block.le(&Uint128::from(0u128)) {
                        return Err(ContractError::InvalidRewardPerBlock{});
                    }
                    collection_pool_info.reward_per_block = reward_per_block;
                }

                return Ok(collection_pool_info);
            } else {
                Err(ContractError::Std(StdError::generic_err("invalid update empty!")))
            }
        })?;
        
    Ok(Response::new()
        .add_attribute("action", "update_collection_pool_info")
        .add_attribute("collection_id", msg.collection_id)
    )
}

fn try_create_collection_pool_info(
    deps: DepsMut, 
    env: Env, 
    info: MessageInfo, 
    msg: CreateCollectionPoolMsg
) -> Result<Response, ContractError> {
    check_admin_permission(deps.as_ref(), &info.sender)?;

    if msg.reward_per_block.le(&Uint128::from(0u128)) {
        return Err(ContractError::InvalidRewardPerBlock {});
    }

    let existed_collection_info = COLLECTION_POOL_INFO.may_load(deps.storage, &msg.collection_id.clone().as_bytes())?;

    if existed_collection_info.is_some() {
        return Err(ContractError::Std(StdError::generic_err(
            "Collection info already existed",
        )));
    }

    let mut new_collection_info = CollectionPoolInfo {
        collection_id: msg.collection_id.clone(),
        reward_per_block: msg.reward_per_block.clone(),
        total_nfts: Uint128::from(0u128),
        acc_per_share: Uint128::from(0u128),
        last_reward_block: 0u64,
        expired_block: None
    };

    if let Some(expired_after) = msg.expired_after {
        new_collection_info.expired_block = Some(env.block.height + expired_after);
    }

    COLLECTION_POOL_INFO.save(
        deps.storage, 
        msg.collection_id.clone().as_bytes(), 
        &new_collection_info,
    )?;

    Ok(Response::new()
        .add_attribute("action", "create_collection_pool")
        .add_attribute("collection_id", msg.collection_id)
        .add_attribute("reward_per_block", msg.reward_per_block)
    )


}


pub fn try_update_contract_info(
    deps: DepsMut,
    info: MessageInfo,
    msg: UpdateContractInfoMsg,
) -> Result<Response, ContractError>  {
    check_admin_permission(deps.as_ref(), &info.sender)?;

    CONTRACT_INFO.update (
        deps.storage,
        |mut old_info| -> Result<ContractInfo, ContractError> {
            if let Some(admin) = msg.admin {
                old_info.admin = Some(admin);
            }
            if let Some(whitelist) = msg.nft_721_contract_addr_whitelist {
                for addr in whitelist.into_iter() {
                    let existed = old_info
                        .nft_721_contract_addr_whitelist
                        .iter()
                        .find(|a| a.eq(&&addr));
                    if existed.is_none() {
                        old_info.nft_721_contract_addr_whitelist.push(addr);
                    }
                }
            }
            Ok(old_info)
        }
    )?;

    Ok(Response::new()
        .add_attribute("action", "update_info")
    )
}


fn check_admin_permission(deps: Deps, address: &Addr) -> Result<(), ContractError> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    let admin = contract_info.admin.unwrap();
    if !admin.eq(address) {
        return Err(ContractError::Unauthorized {
            sender: address.to_string(),
        });
    } else {
        Ok(())
    }
}

fn check_collection_is_expired(
    env: Env,
    collection_pool_info: &CollectionPoolInfo,
) -> Result<bool, ContractError> {
    //let collection_pool_info = COLLECTION_POOL_INFO.load(store, k)
    match collection_pool_info.expired_block {
        Some(expired_block) => {
            if env.block.height >= expired_block {
                return Err(ContractError::ExpiredCollection {});
            }
            Ok(true)
        }
        None => Ok(true),
    }
}

fn update_collection_pool(
    storage: &mut dyn Storage,
    env: Env,
    collection_id: String
) -> StdResult<CollectionPoolInfo> {
    let collection_pool_info = COLLECTION_POOL_INFO
        .load(storage, collection_id.clone().as_bytes())
        .unwrap();
    
    if collection_pool_info.last_reward_block > 0 && env.block.height <= collection_pool_info.last_reward_block {
        return Ok(collection_pool_info);
    }

    if collection_pool_info.total_nfts.eq(&Uint128::from(0u128)) {
        let update_collection_pool_info = 
            COLLECTION_POOL_INFO.update(storage, collection_id.clone().as_bytes(), | data | {
                if let Some(mut old_info) = data {
                    old_info.last_reward_block = env.block.height;
                    return Ok(old_info);
                } else {
                    return Err(StdError::generic_err("Invalid update collection info"));
                }
            })?;
            return Ok(update_collection_pool_info);
    } else {
        // Update accumulate_per_share and last_block_reward
        let multiplier = env.block.height - collection_pool_info.last_reward_block;
        let reward = collection_pool_info.reward_per_block * Uint128::from(multiplier);

        let update_collection_pool_info = 
            COLLECTION_POOL_INFO.update(storage, collection_id.clone().as_bytes(), | data | {
                if let Some(mut old_info) = data {
                    old_info.acc_per_share = old_info.acc_per_share + reward / collection_pool_info.total_nfts;
                    old_info.last_reward_block = env.block.height;
                    return Ok(old_info);
                } else {
                    return Err(StdError::generic_err("Invalid update collection info"));
                };
            })?;
            Ok(update_collection_pool_info)

    }
}


