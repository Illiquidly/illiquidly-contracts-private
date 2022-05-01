use cosmwasm_std::{Addr, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult};

use cw1155::Cw1155ExecuteMsg;
use cw20::Cw20ExecuteMsg;
use cw721::Cw721ExecuteMsg;

use p2p_trading_export::msg::{into_cosmos_msg, QueryFilters};
use p2p_trading_export::state::{AdditionnalTradeInfo, AssetInfo, TradeInfo, TradeState};

use crate::error::ContractError;
use crate::messages::set_comment;
use crate::query::query_counter_trades;
use crate::state::{
    add_cw1155_coin, add_cw20_coin, add_cw721_coin, add_funds, can_suggest_counter_trade,
    is_counter_trader, load_trade, COUNTER_TRADE_INFO, TRADE_INFO,
};
use crate::trade::{are_assets_in_trade, create_withdraw_messages, try_withdraw_assets_unsafe};

pub fn get_last_counter_id_created(deps: Deps, by: String, trade_id: u64) -> StdResult<u64> {
    let counter_trade = &query_counter_trades(
        deps,
        trade_id,
        None,
        Some(1),
        Some(QueryFilters {
            owner: Some(by),
            ..QueryFilters::default()
        }),
    )?
    .counter_trades[0];
    if counter_trade.trade_id != trade_id {
        Err(StdError::generic_err(
            "Wrong trade id for the last counter trade",
        ))
    } else {
        Ok(counter_trade.counter_id.unwrap())
    }
}
pub fn suggest_counter_trade(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trade_id: u64,
    comment: Option<String>,
) -> Result<Response, ContractError> {
    // We start by verifying it is possible to suggest a counter trade to that trade
    // It also checks if the trade exists
    // And that the sender is whitelisted (in case the trade is private)
    let mut trade_info = can_suggest_counter_trade(deps.storage, trade_id, &info.sender)?;

    // We start by creating a new trade_id (simply incremented from the last id)
    trade_info.last_counter_id = trade_info
        .last_counter_id
        .map_or(Some(0), |id| Some(id + 1));

    if trade_info.state == TradeState::Published {
        trade_info.state = TradeState::Countered;
    }

    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;

    let counter_id = trade_info.last_counter_id.unwrap(); // This is safe, as per the statement above.

    // If the trade id already exists, the contract is faulty
    // Or an external error happened, or whatever...
    // In that case, we emit an error
    // The priority is : We do not want to overwrite existing data
    COUNTER_TRADE_INFO.update(
        deps.storage,
        (trade_id.into(), counter_id.into()),
        |counter| match counter {
            Some(_) => Err(ContractError::ExistsInCounterTradeInfo {}),
            None => Ok(TradeInfo {
                owner: info.sender.clone(),
                additionnal_info: AdditionnalTradeInfo {
                    time: env.block.time,
                    ..Default::default()
                },
                ..Default::default()
            }),
        },
    )?;

    if let Some(comment) = comment {
        set_comment(deps, env, info, trade_id, Some(counter_id), comment)?;
    }

    Ok(Response::new()
        .add_attribute("counter", "created")
        .add_attribute("trade_id", trade_id.to_string())
        .add_attribute("counter_id", counter_id.to_string()))
}

pub fn prepare_counter_asset_addition(
    deps: Deps,
    trader: Addr,
    trade_id: u64,
    counter_id: Option<u64>,
) -> Result<u64, ContractError> {
    let counter_id = match counter_id {
        Some(counter_id) => Ok(counter_id),
        None => get_last_counter_id_created(deps, trader.to_string(), trade_id),
    }?;

    let counter_info = is_counter_trader(deps.storage, &trader, trade_id, counter_id)?;

    if counter_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: counter_info.state,
        });
    }
    Ok(counter_id)
}

pub fn add_asset_to_counter_trade(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: Option<u64>,
    asset: AssetInfo,
) -> Result<Response, ContractError> {
    let counter_id =
        prepare_counter_asset_addition(deps.as_ref(), info.sender.clone(), trade_id, counter_id)?;

    match asset.clone() {
        AssetInfo::Coin(coin) => COUNTER_TRADE_INFO.update(
            deps.storage,
            (trade_id.into(), counter_id.into()),
            add_funds(coin, info.funds),
        ),
        AssetInfo::Cw20Coin(token) => COUNTER_TRADE_INFO.update(
            deps.storage,
            (trade_id.into(), counter_id.into()),
            add_cw20_coin(token.address.clone(), token.amount),
        ),
        AssetInfo::Cw721Coin(token) => COUNTER_TRADE_INFO.update(
            deps.storage,
            (trade_id.into(), counter_id.into()),
            add_cw721_coin(token.address.clone(), token.token_id),
        ),
        AssetInfo::Cw1155Coin(token) => COUNTER_TRADE_INFO.update(
            deps.storage,
            (trade_id.into(), counter_id.into()),
            add_cw1155_coin(token.address.clone(), token.token_id.clone(), token.value),
        ),
    }?;

    // Now we need to transfer the token
    Ok(match asset {
        AssetInfo::Coin(coin) => Response::new()
            .add_attribute("added funds", "counter")
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
                .add_attribute("added token", "counter")
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
                .add_attribute("added token", "counter")
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
                .add_attribute("added Cw1155", "counter")
                .add_attribute("token", token.address)
                .add_attribute("token_id", token.token_id)
                .add_attribute("amount", token.value)
        }
    }
    .add_attribute("trade_id", trade_id.to_string())
    .add_attribute("counter_id", counter_id.to_string()))
}

pub fn confirm_counter_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: Option<u64>,
) -> Result<Response, ContractError> {
    let trade_info = load_trade(deps.storage, trade_id)?;

    let counter_id = match counter_id {
        Some(counter_id) => Ok(counter_id),
        None => get_last_counter_id_created(deps.as_ref(), info.sender.to_string(), trade_id),
    }?;

    let mut counter = is_counter_trader(deps.storage, &info.sender, trade_id, counter_id)?;

    if trade_info.state != TradeState::Countered {
        return Err(ContractError::CantChangeTradeState {
            from: trade_info.state,
            to: TradeState::Countered,
        });
    }

    // We update the trade_info to show there are some suggested counters
    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;

    // We update the counter_trade_info to indicate it is published and ready to be accepted
    if counter.state != TradeState::Created {
        return Err(ContractError::CantChangeCounterTradeState {
            from: counter.state,
            to: TradeState::Published,
        });
    }
    counter.state = TradeState::Published;

    COUNTER_TRADE_INFO.save(deps.storage, (trade_id.into(), counter_id.into()), &counter)?;

    Ok(Response::new()
        .add_attribute("confirmed", "counter")
        .add_attribute("trade", trade_id.to_string())
        .add_attribute("counter", counter_id.to_string()))
}

pub fn cancel_counter_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: u64,
) -> Result<Response, ContractError> {
    // Only the initial trader can cancel the trade
    let mut counter_info = is_counter_trader(deps.storage, &info.sender, trade_id, counter_id)?;

    if counter_info.state == TradeState::Accepted {
        return Err(ContractError::CantChangeCounterTradeState {
            from: counter_info.state,
            to: TradeState::Cancelled,
        });
    }
    counter_info.state = TradeState::Cancelled;

    // We store the new trade status
    COUNTER_TRADE_INFO.save(
        deps.storage,
        (trade_id.into(), counter_id.into()),
        &counter_info,
    )?;

    Ok(Response::new()
        .add_attribute("cancelled", "counter")
        .add_attribute("trade", trade_id.to_string())
        .add_attribute("counter", counter_id.to_string()))
}

pub fn withdraw_counter_trade_assets_while_creating(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: u64,
    assets: Vec<(u16, AssetInfo)>,
) -> Result<Response, ContractError> {
    let mut counter_info = is_counter_trader(deps.storage, &info.sender, trade_id, counter_id)?;

    if counter_info.state != TradeState::Created && counter_info.state != TradeState::Cancelled {
        return Err(ContractError::CounterTradeAlreadyPublished {});
    }

    are_assets_in_trade(&counter_info, &assets)?;

    try_withdraw_assets_unsafe(&mut counter_info, &assets)?;

    COUNTER_TRADE_INFO.save(
        deps.storage,
        (trade_id.into(), counter_id.into()),
        &counter_info,
    )?;

    let res = create_withdraw_messages(
        &env.contract.address,
        &info.sender,
        &assets.iter().map(|x| x.1.clone()).collect(),
    )?;
    Ok(res.add_attribute("remove from", "counter"))
}
