use cosmwasm_std::{Coin, DepsMut, Env, MessageInfo, Response, Uint128};

use cw20::Cw20ExecuteMsg;
use cw721::Cw721ExecuteMsg;
use cw1155::Cw1155ExecuteMsg;

use crate::error::ContractError;
use crate::state::{
    add_cw20_coin, add_cw721_coin, add_cw1155_coin, add_funds, can_suggest_counter_trade, is_counter_trader,
    load_trade, COUNTER_TRADE_INFO, TRADE_INFO,
};
use p2p_trading_export::msg::into_cosmos_msg;
use p2p_trading_export::state::{AssetInfo, TradeInfo, TradeState};

use crate::trade::{are_assets_in_trade, create_withdraw_messages, try_withdraw_assets_unsafe};

pub fn suggest_counter_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
) -> Result<Response, ContractError> {
    // We start by verifying it is possible to suggest a counter trade to that trade
    // It also checks if the trade exists
    // And that the sender is whitelisted (in case the trade is private)
    can_suggest_counter_trade(deps.storage, trade_id, &info.sender)?;

    // We start by creating a new trade_id (simply incremented from the last id)
    let new_trade_info = TRADE_INFO.update(
        deps.storage,
        trade_id.into(),
        |c| -> Result<TradeInfo, ContractError> {
            match c {
                Some(mut trade_info) => {
                    match trade_info.last_counter_id {
                        Some(last_counter_id) => {
                            trade_info.last_counter_id = Some(last_counter_id + 1)
                        }
                        None => trade_info.last_counter_id = Some(0),
                    }
                    if trade_info.state == TradeState::Published {
                        trade_info.state = TradeState::Countered;
                    }
                    Ok(trade_info)
                }
                //TARPAULIN : Unreachable
                None => Err(ContractError::NotFoundInTradeInfo {}),
            }
        },
    )?;

    let counter_id = new_trade_info.last_counter_id.unwrap(); // This is safe, as per the statement above.

    // If the trade id already exists, the contract is faulty
    // Or an external error happened, or whatever...
    // In that case, we emit an error
    // The priority is : We do not want to overwrite existing data
    if COUNTER_TRADE_INFO.has(deps.storage, (trade_id.into(), counter_id.into())) {
        return Err(ContractError::ExistsInCounterTradeInfo {});
    } else {
        COUNTER_TRADE_INFO.save(
            deps.storage,
            (trade_id.into(), counter_id.into()),
            &TradeInfo {
                owner: info.sender.clone(),
                // We add the funds sent along with this transaction
                associated_funds: info.funds,
                ..Default::default()
            },
        )?;
    }

    Ok(Response::new()
        .add_attribute("counter", "created")
        .add_attribute("trade_id", trade_id.to_string())
        .add_attribute("counter_id", counter_id.to_string()))
}

pub fn add_funds_to_counter_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: u64,
) -> Result<Response, ContractError> {
    let counter_info = is_counter_trader(deps.storage, &info.sender, trade_id, counter_id)?;
    if counter_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: counter_info.state,
        });
    }

    COUNTER_TRADE_INFO.update(
        deps.storage,
        (trade_id.into(), counter_id.into()),
        add_funds(info.funds),
    )?;

    Ok(Response::new()
        .add_attribute("added funds", "counter")
        .add_attribute("trade_id", trade_id.to_string())
        .add_attribute("counter_id", counter_id.to_string()))
}

pub fn add_token_to_counter_trade(
    deps: DepsMut,
    env: Env,
    trader: String,
    trade_id: u64,
    counter_id: u64,
    token: String,
    sent_amount: Uint128,
) -> Result<Response, ContractError> {
    let counter_info = is_counter_trader(
        deps.storage,
        &deps.api.addr_validate(&trader)?,
        trade_id,
        counter_id,
    )?;
    if counter_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: counter_info.state,
        });
    }

    COUNTER_TRADE_INFO.update(
        deps.storage,
        (trade_id.into(), counter_id.into()),
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
        .add_attribute("added token", "counter")
        .add_attribute("token", token)
        .add_attribute("amount", sent_amount))
}

pub fn add_nft_to_counter_trade(
    deps: DepsMut,
    env: Env,
    trader: String,
    trade_id: u64,
    counter_id: u64,
    token: String,
    token_id: String,
) -> Result<Response, ContractError> {
    let counter_info = is_counter_trader(
        deps.storage,
        &deps.api.addr_validate(&trader)?,
        trade_id,
        counter_id,
    )?;
    if counter_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: counter_info.state,
        });
    }

    COUNTER_TRADE_INFO.update(
        deps.storage,
        (trade_id.into(), counter_id.into()),
        add_cw721_coin(token.clone(), token_id.clone()),
    )?;

    // Now we need to transfer the nft
    let message = Cw721ExecuteMsg::TransferNft {
        recipient: env.contract.address.into(),
        token_id: token_id.clone(),
    };

    Ok(Response::new()
        .add_message(into_cosmos_msg(message, token.clone())?)
        .add_attribute("added token", "counter")
        .add_attribute("nft", token)
        .add_attribute("token_id", token_id))
}

pub fn add_cw1155_to_counter_trade(
    deps: DepsMut,
    env: Env,
    trader: String,
    trade_id: u64,
    counter_id: u64,
    token: String,
    token_id: String, 
    sent_amount: Uint128,
) -> Result<Response, ContractError> {
    let counter_info = is_counter_trader(
        deps.storage,
        &deps.api.addr_validate(&trader)?,
        trade_id,
        counter_id,
    )?;
    if counter_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: counter_info.state,
        });
    }

    COUNTER_TRADE_INFO.update(
        deps.storage,
        (trade_id.into(), counter_id.into()),
        add_cw1155_coin(token.clone(), token_id.clone(), sent_amount),
    )?;

    // Now we need to transfer the token
    let message = Cw1155ExecuteMsg::SendFrom {
        from: trader,
        to: env.contract.address.into(),
        token_id: token_id.clone(),
        value: sent_amount,
        msg:None
    };

    Ok(Response::new()
        .add_message(into_cosmos_msg(message, token.clone())?)
        .add_attribute("added Cw1155", "trade")
        .add_attribute("token", token)
        .add_attribute("token_id", token_id)
        .add_attribute("amount", sent_amount))
}

pub fn confirm_counter_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: u64,
) -> Result<Response, ContractError> {
    let mut trade_info = load_trade(deps.storage, trade_id)?;

    is_counter_trader(deps.storage, &info.sender, trade_id, counter_id)?;

    if trade_info.state != TradeState::Countered {
        return Err(ContractError::CantChangeTradeState {
            from: trade_info.state,
            to: TradeState::Countered,
        });
    }
    trade_info.state = TradeState::Countered;

    // We update the trade_info to show there are some suggested counters
    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;

    // We update the counter_trade_info to indicate it is published and ready to be accepted
    COUNTER_TRADE_INFO.update(
        deps.storage,
        (trade_id.into(), counter_id.into()),
        |d: Option<TradeInfo>| -> Result<TradeInfo, ContractError> {
            match d {
                Some(mut one) => {
                    if one.state != TradeState::Created {
                        return Err(ContractError::CantChangeCounterTradeState {
                            from: one.state,
                            to: TradeState::Published,
                        });
                    }
                    one.state = TradeState::Published;
                    Ok(one)
                }
                //TARPAULIN : Unreachable
                None => Err(ContractError::NotFoundInCounterTradeInfo {}),
            }
        },
    )?;

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
        .add_attribute("trade", trade_id.to_string()))
}

pub fn withdraw_counter_trade_assets_while_creating(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: u64,
    assets: Vec<(u16, AssetInfo)>,
    funds: Vec<(u16, Coin)>,
) -> Result<Response, ContractError> {
    let mut counter_info = is_counter_trader(deps.storage, &info.sender, trade_id, counter_id)?;

    if counter_info.state != TradeState::Created {
        return Err(ContractError::CounterTradeAlreadyPublished {});
    }

    are_assets_in_trade(&counter_info, &assets, &funds)?;

    try_withdraw_assets_unsafe(&mut counter_info, &assets, &funds)?;

    COUNTER_TRADE_INFO.save(
        deps.storage,
        (trade_id.into(), counter_id.into()),
        &counter_info,
    )?;

    let res = create_withdraw_messages(
        &env.contract.address,
        &info.sender,
        &assets.iter().map(|x| x.1.clone()).collect(),
        &funds.iter().map(|x| x.1.clone()).collect(),
    )?;
    Ok(res.add_attribute("remove from", "counter"))
}
