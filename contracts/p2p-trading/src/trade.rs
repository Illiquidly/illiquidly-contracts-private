use cosmwasm_std::{
    Addr, Api, BankMsg, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Storage,
    Uint128,
};

use std::collections::HashSet;
use std::iter::FromIterator;

use cw1155::Cw1155ExecuteMsg;
use cw20::Cw20ExecuteMsg;
use cw721::Cw721ExecuteMsg;

use p2p_trading_export::msg::{into_cosmos_msg, QueryFilters};
use p2p_trading_export::state::{
    AdditionnalTradeInfo, AssetInfo, Comment, CounterTradeInfo, TradeInfo, TradeState,
};

use crate::error::ContractError;
use crate::messages::set_comment;
use crate::query::query_all_trades;
use crate::state::{
    add_cw1155_coin, add_cw20_coin, add_cw721_coin, add_funds, is_trader, load_counter_trade,
    CONTRACT_INFO, COUNTER_TRADE_INFO, TRADE_INFO,
};
pub fn get_last_trade_id_created(deps: Deps, by: String) -> StdResult<u64> {
    Ok(query_all_trades(
        deps,
        None,
        Some(1),
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
    comment: Option<String>,
) -> Result<Response, ContractError> {
    // We start by creating a new trade_id (simply incremented from the last id)
    let trade_id: u64 = CONTRACT_INFO
        .update(deps.storage, |mut c| -> StdResult<_> {
            c.last_trade_id = c.last_trade_id.map_or(Some(0), |id| Some(id + 1));
            Ok(c)
        })?
        .last_trade_id
        .unwrap(); // This is safe because of the function architecture just there

    // If the trade id already exists, the contract is faulty
    // Or an external error happened, or whatever...
    // In that case, we emit an error
    // The priority is : We do not want to overwrite existing data
    TRADE_INFO.update(deps.storage, trade_id.into(), |trade| match trade {
        Some(_) => Err(ContractError::ExistsInTradeInfo {}),
        None => Ok(TradeInfo {
            owner: info.sender.clone(),
            additionnal_info: AdditionnalTradeInfo {
                time: env.block.time,
                ..Default::default()
            },
            ..Default::default()
        }),
    })?;

    if let Some(whitelist) = whitelisted_users {
        add_whitelisted_users(
            deps.storage,
            deps.api,
            env.clone(),
            info.clone(),
            trade_id,
            whitelist,
        )?;
    }

    if let Some(comment) = comment {
        set_comment(deps, env, info, trade_id, None, comment)?;
    }

    Ok(Response::new()
        .add_attribute("trade", "created")
        .add_attribute("trade_id", trade_id.to_string()))
}

pub fn prepare_trade_asset_addition(
    deps: Deps,
    trader: Addr,
    trade_id: Option<u64>,
) -> Result<u64, ContractError> {
    let trade_id = match trade_id {
        Some(trade_id) => Ok(trade_id),
        None => get_last_trade_id_created(deps, trader.to_string()),
    }?;

    let trade_info = is_trader(deps.storage, &trader, trade_id)?;

    //let trade_info = TRADE_INFO.load(deps.storage, trade_id.into())?;
    if trade_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: trade_info.state,
        });
    }
    Ok(trade_id)
}

pub fn add_asset_to_trade(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trade_id: Option<u64>,
    asset: AssetInfo,
) -> Result<Response, ContractError> {
    let trade_id = prepare_trade_asset_addition(deps.as_ref(), info.sender.clone(), trade_id)?;

    match asset.clone() {
        AssetInfo::Coin(coin) => {
            TRADE_INFO.update(deps.storage, trade_id.into(), add_funds(coin, info.funds))
        }
        AssetInfo::Cw20Coin(token) => TRADE_INFO.update(
            deps.storage,
            trade_id.into(),
            add_cw20_coin(token.address.clone(), token.amount),
        ),
        AssetInfo::Cw721Coin(token) => TRADE_INFO.update(
            deps.storage,
            trade_id.into(),
            add_cw721_coin(token.address.clone(), token.token_id),
        ),
        AssetInfo::Cw1155Coin(token) => TRADE_INFO.update(
            deps.storage,
            trade_id.into(),
            add_cw1155_coin(
                token.address.clone(),
                token.token_id.clone(),
                token.value,
            ),
        ),
    }?;

    // Now we need to transfer the token
    Ok(match asset {
        AssetInfo::Coin(coin) => Response::new()
            .add_attribute("added funds", "trade")
            .add_attribute("denom", coin.denom)
            .add_attribute("amount", coin.amount),
        AssetInfo::Cw20Coin(token) => {
            let message = Cw20ExecuteMsg::TransferFrom {
                owner: info.sender.to_string(),
                recipient: env.contract.address.into(),
                amount: token.amount,
            };
            Response::new()
                .add_message(into_cosmos_msg(message, token.address.clone())?)
                .add_attribute("added token", "trade")
                .add_attribute("token", token.address)
                .add_attribute("amount", token.amount)
        }
        AssetInfo::Cw721Coin(token) => {
            let message = Cw721ExecuteMsg::TransferNft {
                recipient: env.contract.address.into(),
                token_id: token.token_id.clone(),
            };

            Response::new()
                .add_message(into_cosmos_msg(message, token.address.clone())?)
                .add_attribute("added token", "trade")
                .add_attribute("nft", token.address)
                .add_attribute("token_id", token.token_id)
        }
        AssetInfo::Cw1155Coin(token) => {
            let message = Cw1155ExecuteMsg::SendFrom {
                from: info.sender.to_string(),
                to: env.contract.address.into(),
                token_id: token.token_id.clone(),
                value: token.value,
                msg: None,
            };

            Response::new()
                .add_message(into_cosmos_msg(message, token.address.clone())?)
                .add_attribute("added Cw1155", "trade")
                .add_attribute("token", token.address)
                .add_attribute("token_id", token.token_id)
                .add_attribute("amount", token.value)
        }
    }
    .add_attribute("trade_id", trade_id.to_string()))
}

pub fn validate_addresses(api: &dyn Api, whitelisted_users: &[String]) -> StdResult<Vec<Addr>> {
    whitelisted_users
        .iter()
        .map(|x| api.addr_validate(x))
        .collect()
}

pub fn add_whitelisted_users(
    storage: &mut dyn Storage,
    api: &dyn Api,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
    whitelisted_users: Vec<String>,
) -> Result<Response, ContractError> {
    let mut trade_info = is_trader(storage, &info.sender, trade_id)?;
    if trade_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: trade_info.state,
        });
    }

    let hash_set: HashSet<Addr> = HashSet::from_iter(validate_addresses(api, &whitelisted_users)?);
    trade_info.whitelisted_users = trade_info
        .whitelisted_users
        .union(&hash_set)
        .cloned()
        .collect();

    TRADE_INFO.save(storage, trade_id.into(), &trade_info)?;

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
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;

    if trade_info.state != TradeState::Created {
        return Err(ContractError::CantChangeTradeState {
            from: trade_info.state,
            to: TradeState::Published,
        });
    }
    trade_info.state = TradeState::Published;

    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;

    Ok(Response::new()
        .add_attribute("confirmed", "trade")
        .add_attribute("trade", trade_id.to_string()))
}

pub fn accept_trade(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: u64,
    comment: Option<String>,
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
    // We check this specific counter trade can be accepted
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

    counter_info.additionnal_info.trader_comment = comment.map(|comment| Comment {
        time: env.block.time,
        comment,
    });
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
    let trade_info = is_trader(deps.storage, &info.sender, trade_id)?;
    // We check the counter trade exists
    let mut counter_info = load_counter_trade(deps.storage, trade_id, counter_id)?;

    if trade_info.state == TradeState::Accepted {
        return Err(ContractError::TradeAlreadyAccepted {});
    }
    if trade_info.state == TradeState::Cancelled {
        return Err(ContractError::TradeCancelled {});
    }

    counter_info.state = TradeState::Refused;

    COUNTER_TRADE_INFO.save(
        deps.storage,
        (trade_id.into(), counter_id.into()),
        &counter_info,
    )?;

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
) -> Result<Response, ContractError> {
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;
    if trade_info.state != TradeState::Created && trade_info.state != TradeState::Cancelled {
        return Err(ContractError::TradeAlreadyPublished {});
    }

    are_assets_in_trade(&trade_info, &assets)?;

    try_withdraw_assets_unsafe(&mut trade_info, &assets)?;

    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;

    let res = create_withdraw_messages(
        &env.contract.address,
        &info.sender,
        &assets.iter().map(|x| x.1.clone()).collect(),
    )?;
    Ok(res.add_attribute("remove from", "trade"))
}

pub fn are_assets_in_trade(
    trade_info: &TradeInfo,
    assets: &[(u16, AssetInfo)],
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
            AssetInfo::Coin(fund_info) => {
                // We check the fund is the one we want
                if let AssetInfo::Coin(fund) = asset {
                    // We verify the sent information matches the saved fund
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
            }

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

    Ok(())
}

pub fn try_withdraw_assets_unsafe(
    trade_info: &mut TradeInfo,
    assets: &[(u16, AssetInfo)],
) -> Result<(), ContractError> {
    for (position, asset) in assets {
        let position: usize = (*position).into();
        let asset_info = trade_info.associated_assets[position].clone();
        match asset_info {
            AssetInfo::Coin(mut fund_info) => {
                if let AssetInfo::Coin(fund) = asset {
                    // If everything is in order, we remove the coin from the trade
                    fund_info.amount -= fund.amount;
                    trade_info.associated_assets[position] = AssetInfo::Coin(fund_info);
                }
            }
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

    // Then we remove empty assets from the trade
    trade_info.associated_assets.retain(|asset| match asset {
        AssetInfo::Coin(fund) => fund.amount != Uint128::zero(),
        AssetInfo::Cw20Coin(token) => token.amount != Uint128::zero(),
        AssetInfo::Cw721Coin(nft) => !nft.address.is_empty(),
        AssetInfo::Cw1155Coin(cw1155) => cw1155.value != Uint128::zero(),
    });

    Ok(())
}

#[allow(clippy::ptr_arg)]
pub fn create_withdraw_messages(
    contract_address: &Addr,
    recipient: &Addr,
    assets: &Vec<AssetInfo>,
) -> Result<Response, ContractError> {
    let mut res = Response::new();

    // First the assets
    for asset in assets {
        match asset {
            AssetInfo::Coin(fund) => {
                let message = BankMsg::Send {
                    to_address: recipient.to_string(),
                    amount: vec![fund.clone()],
                };
                res = res.add_message(message);
            }
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

    Ok(res)
}
