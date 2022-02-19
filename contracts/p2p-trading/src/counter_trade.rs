use cosmwasm_std::{DepsMut, Env, MessageInfo, Response, StdError, Uint128};

use crate::error::ContractError;
use crate::state::{
    add_cw20_coin, add_cw721_coin, add_funds, can_suggest_counter_trade, is_counter_trader,
    COUNTER_TRADE_INFO, TRADE_INFO,
};
use p2p_trading_export::state::{TradeInfo, TradeState};

pub fn suggest_counter_trade(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trade_id: u64,
    confirm: Option<bool>,
) -> Result<Response, ContractError> {
    // We start by verifying it is possible to suggest a counter trade to that trade
    // It also checks if the trade exists
    can_suggest_counter_trade(deps.storage, trade_id)?;

    // We start by creating a new trade_id (simply incremented from the last id)
    let new_trade_info = TRADE_INFO.update(
        deps.storage,
        &trade_id.to_be_bytes(),
        |c| -> Result<TradeInfo, ContractError> {
            match c {
                Some(mut trade_info) => {
                    match trade_info.last_counter_id {
                        Some(last_counter_id) => {
                            trade_info.last_counter_id = Some(last_counter_id + 1)
                        }
                        None => trade_info.last_counter_id = Some(0),
                    }
                    if trade_info.state != TradeState::Published
                        && trade_info.state != TradeState::Countered
                        && trade_info.state != TradeState::Acknowledged
                    {
                        return Err(ContractError::CantChangeTradeState {
                            from: trade_info.state,
                            to: TradeState::Acknowledged,
                        });
                    }
                    if trade_info.state == TradeState::Published {
                        trade_info.state = TradeState::Acknowledged;
                    }
                    Ok(trade_info)
                }
                _ => Err(ContractError::Std(StdError::generic_err(
                    "Error not reachable (in suggest counter trade)",
                ))),
            }
        },
    )?;

    let counter_id = new_trade_info.last_counter_id.unwrap(); // This is safe, as per the statement above.

    // If the counter trade id already exists, we have a problem !!!
    // (we do not want to overwrite existing data)
    if COUNTER_TRADE_INFO.has(
        deps.storage,
        (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()),
    ) {
        return Err(ContractError::ExistsInCounterTradeInfo {});
    } else {
        COUNTER_TRADE_INFO.save(
            deps.storage,
            (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()),
            &TradeInfo {
                owner: info.sender.clone(),
                // We add the funds sent along with this transaction
                associated_funds: info.funds.clone(),
                associated_assets: vec![],
                state: TradeState::Created,
                last_counter_id: None,
                comment: None,
                accepted_info:None
            },
        )?;
    }

    if let Some(confirmed) = confirm {
        if confirmed {
            confirm_counter_trade(deps, env, info, trade_id, counter_id)?;
        }
    }

    Ok(Response::new()
        .add_attribute("counter", "created")
        .add_attribute("trade_id", trade_id.to_string())
        .add_attribute("counter_id", counter_id.to_string()))
}

pub fn add_funds_to_counter_trade(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: u64,
    confirm: Option<bool>,
) -> Result<Response, ContractError> {
    is_counter_trader(deps.storage, &info.sender, trade_id, counter_id)?;

    COUNTER_TRADE_INFO.update(
        deps.storage,
        (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()),
        add_funds(info.funds.clone()),
    )?;

    if let Some(confirmed) = confirm {
        if confirmed {
            confirm_counter_trade(deps, env, info, trade_id, counter_id)?;
        }
    }

    Ok(Response::new()
        .add_attribute("added funds", "counter")
        .add_attribute("trade_id", trade_id.to_string())
        .add_attribute("counter_id", counter_id.to_string()))
}

pub fn add_token_to_counter_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trader: String,
    trade_id: u64,
    counter_id: u64,
    sent_amount: Uint128,
) -> Result<Response, ContractError> {
    is_counter_trader(
        deps.storage,
        &deps.api.addr_validate(&trader)?,
        trade_id,
        counter_id,
    )?;

    COUNTER_TRADE_INFO.update(
        deps.storage,
        (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()),
        add_cw20_coin(info.sender.clone(), sent_amount),
    )?;

    Ok(Response::new()
        .add_attribute("added token", "counter")
        .add_attribute("token", info.sender.to_string())
        .add_attribute("amount", sent_amount.to_string()))
}

pub fn add_nft_to_counter_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trader: String,
    trade_id: u64,
    counter_id: u64,
    token_id: String,
) -> Result<Response, ContractError> {
    is_counter_trader(
        deps.storage,
        &deps.api.addr_validate(&trader)?,
        trade_id,
        counter_id,
    )?;

    COUNTER_TRADE_INFO.update(
        deps.storage,
        (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()),
        add_cw721_coin(info.sender.clone(), token_id.clone()),
    )?;

    Ok(Response::new()
        .add_attribute("added token", "counter")
        .add_attribute("nft", info.sender.to_string())
        .add_attribute("token_id", token_id))
}

pub fn confirm_counter_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: u64,
) -> Result<Response, ContractError> {
    is_counter_trader(deps.storage, &info.sender, trade_id, counter_id)?;

    // We update the trade_info to show there are some suggested counters
    TRADE_INFO.update(
        deps.storage,
        &trade_id.to_be_bytes(),
        |d: Option<TradeInfo>| -> Result<TradeInfo, ContractError> {
            match d {
                Some(mut one) => {
                    if one.state != TradeState::Acknowledged && one.state != TradeState::Countered {
                        return Err(ContractError::CantChangeTradeState {
                            from: one.state,
                            to: TradeState::Countered,
                        });
                    }
                    one.state = TradeState::Countered;
                    Ok(one)
                }
                None => Err(ContractError::NotFoundInTradeInfo {}),
            }
        },
    )?;

    // We update the counter_trade_info to indicate it is published and ready to be accepted
    COUNTER_TRADE_INFO.update(
        deps.storage,
        (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()),
        |d: Option<TradeInfo>| -> Result<TradeInfo, ContractError> {
            match d {
                Some(mut one) => {
                    if one.state != TradeState::Created {
                        return Err(ContractError::CantChangeCounterTradeState {
                            from: one.state,
                            to: TradeState::Countered,
                        });
                    }
                    one.state = TradeState::Published;
                    Ok(one)
                }
                None => Err(ContractError::NotFoundInCounterTradeInfo {}),
            }
        },
    )?;

    Ok(Response::new()
        .add_attribute("confirmed", "counter")
        .add_attribute("trade", trade_id.to_string())
        .add_attribute("counter", counter_id.to_string()))
}
