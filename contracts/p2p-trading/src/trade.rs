use cosmwasm_std::{
    Addr, Api, BankMsg, Coin, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128,
};

use std::collections::HashSet;
use std::iter::FromIterator;

use cw1155::Cw1155ExecuteMsg;
use cw20::Cw20ExecuteMsg;
use cw721::Cw721ExecuteMsg;

use crate::error::ContractError;
use crate::query::query_all_trades;
use crate::state::{
    add_cw1155_coin, add_cw20_coin, add_cw721_coin, add_funds, is_trader, load_counter_trade,
    CONTRACT_INFO, COUNTER_TRADE_INFO, TRADE_INFO,
};

use p2p_trading_export::msg::{into_cosmos_msg, QueryFilters};
use p2p_trading_export::state::{AssetInfo, CounterTradeInfo, TradeInfo, TradeState};

pub fn get_last_trade_id_created(deps: Deps, by: String) -> StdResult<u64> {
    Ok(query_all_trades(
        deps,
        None,
        None,
        Some(QueryFilters {
            owner: Some(by),
            ..QueryFilters::default()
        }),
    )?
    .trades[0]
        .trade_id)
}

pub fn create_trade(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    whitelisted_users: Option<Vec<String>>,
) -> Result<Response, ContractError> {
    // We start by creating a new trade_id (simply incremented from the last id)
    let trade_id: u64 = CONTRACT_INFO
        .update(deps.storage, |mut c| -> StdResult<_> {
            if let Some(trade_id) = c.last_trade_id {
                c.last_trade_id = Some(trade_id + 1)
            } else {
                c.last_trade_id = Some(0);
            }
            Ok(c)
        })?
        .last_trade_id
        .unwrap(); // This is safe because of the function architecture just there

    // If the trade id already exists, the contract is faulty
    // Or an external error happened, or whatever...
    // In that case, we emit an error
    // The priority is : We do not want to overwrite existing data
    if TRADE_INFO.has(deps.storage, trade_id.into()) {
        return Err(ContractError::ExistsInTradeInfo {});
    } else {
        // We can safely create the TradeInfo
        TRADE_INFO.save(
            deps.storage,
            trade_id.into(),
            &TradeInfo {
                owner: info.sender.clone(),
                // We add the funds sent along with this transaction
                associated_funds: info.funds.clone(),
                ..Default::default()
            },
        )?;
    }

    if let Some(whitelist) = whitelisted_users {
        add_whitelisted_users(deps, env, info, trade_id, whitelist)?;
    }

    Ok(Response::new()
        .add_attribute("trade", "created")
        .add_attribute("trade_id", trade_id.to_string()))
}

pub fn add_funds_to_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: Option<u64>,
) -> Result<Response, ContractError> {
    let trade_id = match trade_id {
        Some(trade_id) => Ok(trade_id),
        None => get_last_trade_id_created(deps.as_ref(), info.sender.to_string()),
    }?;

    is_trader(deps.storage, &info.sender, trade_id)?;

    let trade_info = TRADE_INFO.load(deps.storage, trade_id.into())?;
    if trade_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: trade_info.state,
        });
    }

    TRADE_INFO.update(deps.storage, trade_id.into(), add_funds(info.funds))?;

    Ok(Response::new()
        .add_attribute("added funds", "trade")
        .add_attribute("trade_id", trade_id.to_string()))
}

pub fn add_token_to_trade(
    deps: DepsMut,
    env: Env,
    trader: String,
    trade_id: Option<u64>,
    token: String,
    sent_amount: Uint128,
) -> Result<Response, ContractError> {
    let trade_id = match trade_id {
        Some(trade_id) => Ok(trade_id),
        None => get_last_trade_id_created(deps.as_ref(), trader.clone()),
    }?;

    let trade_info = is_trader(deps.storage, &deps.api.addr_validate(&trader)?, trade_id)?;

    if trade_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: trade_info.state,
        });
    }

    TRADE_INFO.update(
        deps.storage,
        trade_id.into(),
        add_cw20_coin(token.clone(), sent_amount),
    )?;

    // Now we need to transfer the token
    let message = Cw20ExecuteMsg::TransferFrom {
        owner: trader,
        recipient: env.contract.address.into(),
        amount: sent_amount,
    };

    Ok(Response::new()
        .add_message(into_cosmos_msg(message, token.clone())?)
        .add_attribute("added token", "trade")
        .add_attribute("token", token)
        .add_attribute("amount", sent_amount))
}

pub fn add_nft_to_trade(
    deps: DepsMut,
    env: Env,
    trader: String,
    trade_id: Option<u64>,
    token: String,
    token_id: String,
) -> Result<Response, ContractError> {
    let trade_id = match trade_id {
        Some(trade_id) => Ok(trade_id),
        None => get_last_trade_id_created(deps.as_ref(), trader.clone()),
    }?;
    let trade_info = is_trader(deps.storage, &deps.api.addr_validate(&trader)?, trade_id)?;

    if trade_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: trade_info.state,
        });
    }

    TRADE_INFO.update(
        deps.storage,
        trade_id.into(),
        add_cw721_coin(token.clone(), token_id.clone()),
    )?;

    // Now we need to transfer the nft
    let message = Cw721ExecuteMsg::TransferNft {
        recipient: env.contract.address.into(),
        token_id: token_id.clone(),
    };

    Ok(Response::new()
        .add_message(into_cosmos_msg(message, token.clone())?)
        .add_attribute("added token", "trade")
        .add_attribute("nft", token)
        .add_attribute("token_id", token_id))
}

pub fn add_cw1155_to_trade(
    deps: DepsMut,
    env: Env,
    trader: String,
    trade_id: Option<u64>,
    token: String,
    token_id: String,
    sent_amount: Uint128,
) -> Result<Response, ContractError> {
    let trade_id = match trade_id {
        Some(trade_id) => Ok(trade_id),
        None => get_last_trade_id_created(deps.as_ref(), trader.clone()),
    }?;
    let trade_info = is_trader(deps.storage, &deps.api.addr_validate(&trader)?, trade_id)?;

    if trade_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: trade_info.state,
        });
    }

    TRADE_INFO.update(
        deps.storage,
        trade_id.into(),
        add_cw1155_coin(token.clone(), token_id.clone(), sent_amount),
    )?;

    // Now we need to transfer the token
    let message = Cw1155ExecuteMsg::SendFrom {
        from: trader,
        to: env.contract.address.into(),
        token_id: token_id.clone(),
        value: sent_amount,
        msg: None,
    };

    Ok(Response::new()
        .add_message(into_cosmos_msg(message, token.clone())?)
        .add_attribute("added Cw1155", "trade")
        .add_attribute("token", token)
        .add_attribute("token_id", token_id)
        .add_attribute("amount", sent_amount))
}

pub fn validate_addresses(api: &dyn Api, whitelisted_users: &[String]) -> StdResult<Vec<Addr>> {
    whitelisted_users
        .iter()
        .map(|x| api.addr_validate(x))
        .collect()
}

pub fn add_whitelisted_users(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
    whitelisted_users: Vec<String>,
) -> Result<Response, ContractError> {
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;
    if trade_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: trade_info.state,
        });
    }

    let hash_set: HashSet<Addr> =
        HashSet::from_iter(validate_addresses(deps.api, &whitelisted_users)?);
    trade_info.whitelisted_users = trade_info
        .whitelisted_users
        .union(&hash_set)
        .cloned()
        .collect();

    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;

    Ok(Response::new().add_attribute("added", "whitelisted_users"))
}

pub fn remove_whitelisted_users(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
    whitelisted_users: Vec<String>,
) -> Result<Response, ContractError> {
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;
    if trade_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: trade_info.state,
        });
    }

    let whitelisted_users = validate_addresses(deps.api, &whitelisted_users)?;

    for user in whitelisted_users {
        trade_info.whitelisted_users.remove(&user);
    }

    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;

    Ok(Response::new().add_attribute("removed", "whitelisted_users"))
}

pub fn add_nfts_wanted(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: Option<u64>,
    nfts_wanted: Vec<String>,
) -> Result<Response, ContractError> {
    let trade_id = match trade_id {
        Some(trade_id) => Ok(trade_id),
        None => get_last_trade_id_created(deps.as_ref(), info.sender.to_string()),
    }?;
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;
    if trade_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: trade_info.state,
        });
    }

    let hash_set: HashSet<Addr> = HashSet::from_iter(validate_addresses(deps.api, &nfts_wanted)?);
    trade_info.additionnal_info.nfts_wanted = trade_info
        .additionnal_info
        .nfts_wanted
        .union(&hash_set)
        .cloned()
        .collect();

    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;

    Ok(Response::new().add_attribute("added", "nfts_wanted"))
}

pub fn remove_nfts_wanted(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
    nfts_wanted: Vec<String>,
) -> Result<Response, ContractError> {
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;
    if trade_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: trade_info.state,
        });
    }

    let nfts_wanted = validate_addresses(deps.api, &nfts_wanted)?;

    for nft in nfts_wanted {
        trade_info.additionnal_info.nfts_wanted.remove(&nft);
    }

    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;

    Ok(Response::new().add_attribute("removed", "nfts_wanted"))
}

pub fn set_comment(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: Option<u64>,
    comment: String,
) -> Result<Response, ContractError> {
    let trade_id = match trade_id {
        Some(trade_id) => Ok(trade_id),
        None => get_last_trade_id_created(deps.as_ref(), info.sender.to_string()),
    }?;
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;
    trade_info.additionnal_info.comment = Some(comment.clone());
    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;
    Ok(Response::new()
        .add_attribute("set", "comment")
        .add_attribute("comment", comment))
}

pub fn confirm_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: Option<u64>,
) -> Result<Response, ContractError> {
    let trade_id = match trade_id {
        Some(trade_id) => Ok(trade_id),
        None => get_last_trade_id_created(deps.as_ref(), info.sender.to_string()),
    }?;
    is_trader(deps.storage, &info.sender, trade_id)?;

    TRADE_INFO.update(
        deps.storage,
        trade_id.into(),
        |d: Option<TradeInfo>| -> Result<TradeInfo, ContractError> {
            match d {
                Some(mut one) => {
                    if one.state != TradeState::Created {
                        return Err(ContractError::CantChangeTradeState {
                            from: one.state,
                            to: TradeState::Published,
                        });
                    }
                    one.state = TradeState::Published;
                    Ok(one)
                }
                // TARPAULIN : Unreachable code
                None => Err(ContractError::NotFoundInTradeInfo {}),
            }
        },
    )?;

    Ok(Response::new()
        .add_attribute("confirmed", "trade")
        .add_attribute("trade", trade_id.to_string()))
}

pub fn accept_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: u64,
) -> Result<Response, ContractError> {
    // Only the initial trader can accept a trade !
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;
    // We check the counter trade exists !
    let mut counter_info = load_counter_trade(deps.storage, trade_id, counter_id)?;

    // We check we can accept the trade
    if trade_info.state != TradeState::Countered {
        // TARPAULIN : This code does not seem to be reachable
        return Err(ContractError::CantChangeTradeState {
            from: trade_info.state,
            to: TradeState::Accepted,
        });
    }
    if counter_info.state != TradeState::Published {
        return Err(ContractError::CantAcceptNotPublishedCounter {});
    }

    // We accept the trade
    // An accepted trade whould contain additionnal info to make indexing more easy
    let accepted_info = CounterTradeInfo {
        trade_id,
        counter_id,
    };
    trade_info.state = TradeState::Accepted;
    trade_info.accepted_info = Some(accepted_info);

    counter_info.state = TradeState::Accepted;

    // And we save that to storage
    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;
    COUNTER_TRADE_INFO.save(
        deps.storage,
        (trade_id.into(), counter_id.into()),
        &counter_info,
    )?;

    Ok(Response::new()
        .add_attribute("accepted", "trade")
        .add_attribute("trade", trade_id.to_string())
        .add_attribute("counter", counter_id.to_string()))
}

pub fn refuse_counter_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: u64,
) -> Result<Response, ContractError> {
    // Only the initial trader can refuse a trade
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;
    // We check the counter trade exists
    load_counter_trade(deps.storage, trade_id, counter_id)?;

    if trade_info.state == TradeState::Accepted {
        return Err(ContractError::TradeAlreadyAccepted {});
    }
    if trade_info.state == TradeState::Cancelled {
        return Err(ContractError::TradeCancelled {});
    }

    trade_info.state = TradeState::Refused;

    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;

    /*
    // We go through all counter trades, to cancel the counter_id and to update the current trade state
    let mut is_countered = false;
    // We get all the counter trades for this trade
    let counter_trade_keys: Vec<Vec<u8>> = COUNTER_TRADE_INFO
        .prefix(trade_id.into())
        .keys(deps.storage, None, None, Order::Ascending)
        .collect();

    // We go through all of them and change their status
    for key in counter_trade_keys {
        COUNTER_TRADE_INFO.update(
            deps.storage,
            (trade_id.into(), &key),
            |d: Option<TradeInfo>| -> Result<TradeInfo, ContractError> {
                match d {
                    Some(mut one) => {
                        let id: &[u8] = &key;
                        if id == counter_id.to_be_bytes() {
                            one.state =
                        }else if one.state == TradeState::Published {
                            is_countered = true;
                        }
                        Ok(one)
                    }
                    // TARPAULIN : Unreachable
                    None => Err(ContractError::NotFoundInCounterTradeInfo {}),
                }
            },
        )?;
    }
    */

    Ok(Response::new()
        .add_attribute("refuse", "counter")
        .add_attribute("trade", trade_id.to_string())
        .add_attribute("counter", counter_id.to_string()))
}

pub fn cancel_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
) -> Result<Response, ContractError> {
    // Only the initial trader can cancel the trade
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;

    if trade_info.state == TradeState::Accepted {
        return Err(ContractError::CantChangeTradeState {
            from: trade_info.state,
            to: TradeState::Cancelled,
        });
    }
    trade_info.state = TradeState::Cancelled;

    // We store the new trade status
    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;

    Ok(Response::new()
        .add_attribute("cancelled", "trade")
        .add_attribute("trade", trade_id.to_string()))
}

pub fn withdraw_trade_assets_while_creating(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trade_id: u64,
    assets: Vec<(u16, AssetInfo)>,
    funds: Vec<(u16, Coin)>,
) -> Result<Response, ContractError> {
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;
    if trade_info.state != TradeState::Created {
        return Err(ContractError::TradeAlreadyPublished {});
    }

    are_assets_in_trade(&trade_info, &assets, &funds)?;

    try_withdraw_assets_unsafe(&mut trade_info, &assets, &funds)?;

    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;

    let res = create_withdraw_messages(
        &env.contract.address,
        &info.sender,
        &assets.iter().map(|x| x.1.clone()).collect(),
        &funds.iter().map(|x| x.1.clone()).collect(),
    )?;
    Ok(res.add_attribute("remove from", "trade"))
}

pub fn are_assets_in_trade(
    trade_info: &TradeInfo,
    assets: &[(u16, AssetInfo)],
    funds: &[(u16, Coin)],
) -> Result<(), ContractError> {
    // We first treat the assets
    for (position, asset) in assets {
        let position: usize = (*position).into();

        if position >= trade_info.associated_assets.len() {
            return Err(ContractError::Std(StdError::generic_err(
                "assets position does not exist in array",
            )));
        }
        let asset_info: AssetInfo = trade_info.associated_assets[position].clone();
        match asset_info {
            AssetInfo::Cw20Coin(token_info) => {
                // We check the token is the one we want
                if let AssetInfo::Cw20Coin(token) = asset {
                    // We verify the sent information matches the saved token
                    if token_info.address != token.address {
                        return Err(ContractError::Std(StdError::generic_err(format!(
                            "Wrong token address at position {position}",
                            position = position
                        ))));
                    }
                    if token_info.amount < token.amount {
                        return Err(ContractError::Std(StdError::generic_err(format!(
                            "You can't withdraw that much {address}, \
                                wanted: {wanted}, \
                                available: {available}",
                            address = token_info.address,
                            wanted = token.amount,
                            available = token_info.amount
                        ))));
                    }
                } else {
                    return Err(ContractError::Std(StdError::generic_err(format!(
                        "Wrong token type at position {position}",
                        position = position
                    ))));
                }
            }
            AssetInfo::Cw721Coin(nft_info) => {
                // We check the token is the one we want
                if let AssetInfo::Cw721Coin(nft) = asset {
                    // We verify the sent information matches the saved nft
                    if nft_info.address != nft.address {
                        return Err(ContractError::Std(StdError::generic_err(format!(
                            "Wrong nft address at position {position}",
                            position = position
                        ))));
                    }
                    if nft_info.token_id != nft.token_id {
                        return Err(ContractError::Std(StdError::generic_err(format!(
                            "Wrong nft id at position {position}, \
                                wanted: {wanted}, \
                                found: {found}",
                            position = position,
                            wanted = nft.token_id,
                            found = nft_info.token_id
                        ))));
                    }
                } else {
                    return Err(ContractError::Std(StdError::generic_err(format!(
                        "Wrong token type at position {position}",
                        position = position
                    ))));
                }
            }
            AssetInfo::Cw1155Coin(cw1155_info) => {
                // We check the token is the one we want
                if let AssetInfo::Cw1155Coin(cw1155) = asset {
                    // We verify the sent information matches the saved nft
                    if cw1155_info.address != cw1155.address {
                        return Err(ContractError::Std(StdError::generic_err(format!(
                            "Wrong nft address at position {position}",
                            position = position
                        ))));
                    }
                    if cw1155_info.token_id != cw1155.token_id {
                        return Err(ContractError::Std(StdError::generic_err(format!(
                            "Wrong cw1155 id at position {position}, \
                                wanted: {wanted}, \
                                found: {found}",
                            position = position,
                            wanted = cw1155.token_id,
                            found = cw1155_info.token_id
                        ))));
                    }
                    if cw1155_info.value < cw1155.value {
                        return Err(ContractError::Std(StdError::generic_err(format!(
                            "You can't withdraw that much {address}, \
                                wanted: {wanted}, \
                                available: {available}",
                            address = cw1155_info.address,
                            wanted = cw1155.value,
                            available = cw1155_info.value
                        ))));
                    }
                } else {
                    return Err(ContractError::Std(StdError::generic_err(format!(
                        "Wrong token type at position {position}",
                        position = position
                    ))));
                }
            }
        }
    }

    // Then we take care of the funds
    for (position, fund) in funds {
        let position: usize = (*position).into();
        if position >= trade_info.associated_funds.len() {
            return Err(ContractError::Std(StdError::generic_err(
                "assets position does not exist in array",
            )));
        }
        let fund_info = trade_info.associated_funds[position].clone();
        if fund_info.denom != fund.denom {
            return Err(ContractError::Std(StdError::generic_err(format!(
                "Wrong fund denom at position {position}",
                position = position
            ))));
        }
        if fund_info.amount < fund.amount {
            return Err(ContractError::Std(StdError::generic_err(format!(
                "You can't withdraw that much {address}, \
                    wanted: {wanted}, \
                    available: {available}",
                address = fund_info.denom,
                wanted = fund.amount,
                available = fund_info.amount
            ))));
        }
    }
    Ok(())
}

pub fn try_withdraw_assets_unsafe(
    trade_info: &mut TradeInfo,
    assets: &[(u16, AssetInfo)],
    funds: &[(u16, Coin)],
) -> Result<(), ContractError> {
    for (position, asset) in assets {
        let position: usize = (*position).into();
        let asset_info = trade_info.associated_assets[position].clone();
        match asset_info {
            AssetInfo::Cw20Coin(mut token_info) => {
                if let AssetInfo::Cw20Coin(token) = asset {
                    token_info.amount -= token.amount;
                    trade_info.associated_assets[position] = AssetInfo::Cw20Coin(token_info);
                }
            }
            AssetInfo::Cw721Coin(mut nft_info) => {
                if let AssetInfo::Cw721Coin(_) = asset {
                    nft_info.address = "".to_string();
                    trade_info.associated_assets[position] = AssetInfo::Cw721Coin(nft_info);
                }
            }
            AssetInfo::Cw1155Coin(mut cw1155_info) => {
                if let AssetInfo::Cw1155Coin(cw1155) = asset {
                    cw1155_info.value -= cw1155.value;
                    trade_info.associated_assets[position] = AssetInfo::Cw1155Coin(cw1155_info);
                }
            }
        }
    }

    // Then we remove empty funds from the trade
    trade_info.associated_assets.retain(|asset| match asset {
        AssetInfo::Cw20Coin(token) => token.amount != Uint128::zero(),
        AssetInfo::Cw721Coin(nft) => !nft.address.is_empty(),
        AssetInfo::Cw1155Coin(cw1155) => cw1155.value != Uint128::zero(),
    });

    // Then we take care of the wanted funds
    // First, we check funds availability and update their state
    for (position, fund) in funds {
        let position: usize = (*position).into();
        let mut fund_info = trade_info.associated_funds[position].clone();

        // If everything is in order, we remove the coin from the trade
        fund_info.amount -= fund.amount;
        trade_info.associated_funds[position] = fund_info;
    }

    // Then we remove empty funds
    trade_info
        .associated_funds
        .retain(|fund| fund.amount != Uint128::zero());

    Ok(())
}

#[allow(clippy::ptr_arg)]
pub fn create_withdraw_messages(
    contract_address: &Addr,
    recipient: &Addr,
    assets: &Vec<AssetInfo>,
    funds: &Vec<Coin>,
) -> Result<Response, ContractError> {
    let mut res = Response::new();

    // First the assets
    for asset in assets {
        match asset {
            AssetInfo::Cw20Coin(token) => {
                let message = Cw20ExecuteMsg::Transfer {
                    recipient: recipient.to_string(),
                    amount: token.amount,
                };
                res = res.add_message(into_cosmos_msg(message, token.address.clone())?);
            }
            AssetInfo::Cw721Coin(nft) => {
                let message = Cw721ExecuteMsg::TransferNft {
                    recipient: recipient.to_string(),
                    token_id: nft.token_id.clone(),
                };
                res = res.add_message(into_cosmos_msg(message, nft.address.clone())?);
            }
            AssetInfo::Cw1155Coin(cw1155) => {
                let message = Cw1155ExecuteMsg::SendFrom {
                    from: contract_address.to_string(),
                    to: recipient.to_string(),
                    token_id: cw1155.token_id.clone(),
                    value: cw1155.value,
                    msg: None,
                };
                res = res.add_message(into_cosmos_msg(message, cw1155.address.clone())?);
            }
        }
    }

    // Then the funds
    if !funds.is_empty() {
        res = res.add_message(BankMsg::Send {
            to_address: recipient.to_string(),
            amount: funds.to_vec(),
        });
    };

    Ok(res)
}
