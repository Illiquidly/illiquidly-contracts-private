use cosmwasm_std::{DepsMut, Env, MessageInfo, Order, Response, StdResult, Uint128};

use crate::error::ContractError;
use crate::state::{
    add_cw20_coin, add_cw721_coin, add_funds, is_trader, load_counter_trade, load_trade,
    CONTRACT_INFO,
    COUNTER_TRADE_INFO, TRADE_INFO,
};
use p2p_trading_export::state::{TradeInfo, TradeState, AcceptedTradeInfo};

pub fn create_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
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
    if TRADE_INFO.has(deps.storage, &trade_id.to_be_bytes()) {
        return Err(ContractError::ExistsInTradeInfo {});
    } else {
        // We can safely create the TradeInfo
        TRADE_INFO.save(
            deps.storage,
            &trade_id.to_be_bytes(),
            &TradeInfo {
                owner: info.sender.clone(),
                // We add the funds sent along with this transaction
                associated_funds: info.funds,
                associated_assets: vec![],
                state: TradeState::Created,
                last_counter_id: None,
                comment: None,
                accepted_info: None
            },
        )?;
    }

    Ok(Response::new()
        .add_attribute("trade", "created")
        .add_attribute("trade_id", trade_id.to_string()))
}

pub fn add_funds_to_trade(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trade_id: u64,
    confirm: Option<bool>,
) -> Result<Response, ContractError> {
    is_trader(deps.storage, &info.sender, trade_id)?;
    
    let trade_info = TRADE_INFO.load(deps.storage, &trade_id.to_be_bytes())?;
    if trade_info.state != TradeState::Created{
        return Err(ContractError::WrongTradeState{state: trade_info.state});
    }

    TRADE_INFO.update(
        deps.storage,
        &trade_id.to_be_bytes(),
        add_funds(info.funds.clone()),
    )?;

    if let Some(confirmed) = confirm {
        if confirmed {
            confirm_trade(deps, env, info, trade_id)?;
        }
    }

    Ok(Response::new()
        .add_attribute("added funds", "trade")
        .add_attribute("trade_id", trade_id.to_string()))
}

pub fn add_token_to_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trader: String,
    trade_id: u64,
    sent_amount: Uint128,
) -> Result<Response, ContractError> {
    is_trader(deps.storage, &deps.api.addr_validate(&trader)?, trade_id)?;
    
    let trade_info = TRADE_INFO.load(deps.storage, &trade_id.to_be_bytes())?;
    if trade_info.state != TradeState::Created{
        return Err(ContractError::WrongTradeState{state: trade_info.state});
    }

    TRADE_INFO.update(
        deps.storage,
        &trade_id.to_be_bytes(),
        add_cw20_coin(info.sender.clone(), sent_amount),
    )?;

    Ok(Response::new()
        .add_attribute("added token", "trade")
        .add_attribute("token", info.sender.to_string())
        .add_attribute("amount", sent_amount.to_string()))
}

pub fn add_nft_to_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trader: String,
    trade_id: u64,
    token_id: String,
) -> Result<Response, ContractError> {
    is_trader(deps.storage, &deps.api.addr_validate(&trader)?, trade_id)?;

    let trade_info = TRADE_INFO.load(deps.storage, &trade_id.to_be_bytes())?;
    if trade_info.state != TradeState::Created{
        return Err(ContractError::WrongTradeState{state: trade_info.state});
    }

    TRADE_INFO.update(
        deps.storage,
        &trade_id.to_be_bytes(),
        add_cw721_coin(info.sender.clone(), token_id.clone()),
    )?;

    Ok(Response::new()
        .add_attribute("added token", "trade")
        .add_attribute("nft", info.sender.to_string())
        .add_attribute("token_id", token_id))
}

pub fn confirm_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
) -> Result<Response, ContractError> {
    is_trader(deps.storage, &info.sender, trade_id)?;

    TRADE_INFO.update(
        deps.storage,
        &trade_id.to_be_bytes(),
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
    is_trader(deps.storage, &info.sender, trade_id)?;
    // We check the counter trade exists !
    load_counter_trade(deps.storage, trade_id, counter_id)?;

    // We get all the counter trades for this trade
    let counter_trade_keys: Vec<Vec<u8>> = COUNTER_TRADE_INFO
        .prefix(&trade_id.to_be_bytes())
        .keys(deps.storage, None, None, Order::Ascending)
        .collect();


    // An accepted trade whould contain additionnal info to make indexing more easy
    let accepted_info = AcceptedTradeInfo{
        trade_id,
        counter_id
    };

    // We go through all of them and change their status
    for key in counter_trade_keys {
        COUNTER_TRADE_INFO.update(
            deps.storage,
            (&trade_id.to_be_bytes(), &key),
            |d: Option<TradeInfo>| -> Result<TradeInfo, ContractError> {
                match d {
                    Some(mut one) => {
                        let id: &[u8] = &key;
                        if id == counter_id.to_be_bytes() {
                            if one.state != TradeState::Published {
                                return Err(ContractError::CantAcceptNotPublishedCounter {});
                            }
                            one.state = TradeState::Accepted;
                        } else {
                            one.state = TradeState::Refused;
                        }
                        Ok(one)
                    }
                    // TARPAULIN : Unreachable code
                    None => Err(ContractError::NotFoundInCounterTradeInfo {}), 
                }
            },
        )?;
    }

    // Then we need to change the trade status
    TRADE_INFO.update(
        deps.storage,
        &trade_id.to_be_bytes(),
        |d: Option<TradeInfo>| -> Result<TradeInfo, ContractError> {
            match d {
                Some(mut one) => {
                    // The Trade has to be countered in order to be accepted of course !
                    if one.state != TradeState::Countered {
                        // TARPAULIN : This code does not seem to be reachable
                        return Err(ContractError::CantChangeTradeState {
                            from: one.state,
                            to: TradeState::Accepted,
                        });
                    }
                    one.state = TradeState::Accepted;
                    one.accepted_info = Some(accepted_info);
                    Ok(one)
                }
                // TARPAULIN : Unreachable code
                None => Err(ContractError::NotFoundInTradeInfo {}),
            }
        },
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
    is_trader(deps.storage, &info.sender, trade_id)?;
    // We check the counter trade exists 
    load_counter_trade(deps.storage, trade_id, counter_id)?;

    let mut trade_info = TRADE_INFO.load(deps.storage, &trade_id.to_be_bytes())?;
    if trade_info.state == TradeState::Accepted {
        return Err(ContractError::TradeAlreadyAccepted {});
    }
    if trade_info.state == TradeState::Cancelled {
        return Err(ContractError::TradeCancelled {});
    }

    // We go through all counter trades, to cancel the counter_id and to update the current trade state
    let mut is_acknowledged = false;
    let mut is_countered = false;
    // We get all the counter trades for this trade
    let counter_trade_keys: Vec<Vec<u8>> = COUNTER_TRADE_INFO
        .prefix(&trade_id.to_be_bytes())
        .keys(deps.storage, None, None, Order::Ascending)
        .collect();

    // We go through all of them and change their status
    for key in counter_trade_keys {
        COUNTER_TRADE_INFO.update(
            deps.storage,
            (&trade_id.to_be_bytes(), &key),
            |d: Option<TradeInfo>| -> Result<TradeInfo, ContractError> {
                match d {
                    Some(mut one) => {
                        let id: &[u8] = &key;
                        if id == counter_id.to_be_bytes() {
                            one.state = TradeState::Refused;
                        } else if one.state == TradeState::Created {
                            is_acknowledged = true;
                        } else if one.state == TradeState::Published {
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

    // As we removed a counter offer, we need to update the trade state accordingly
    if is_acknowledged {
        trade_info.state = TradeState::Acknowledged;
    }
    if is_countered {
        trade_info.state = TradeState::Countered;
    }
    TRADE_INFO.save(deps.storage, &trade_id.to_be_bytes(), &trade_info)?;

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
    is_trader(deps.storage, &info.sender, trade_id)?;

    let mut trade_info = load_trade(deps.storage, trade_id)?;
    if trade_info.state == TradeState::Accepted {
        return Err(ContractError::CantChangeTradeState {
            from: trade_info.state,
            to: TradeState::Cancelled,
        });
    }
    trade_info.state = TradeState::Cancelled;

    // We store the new trade status
    TRADE_INFO.save(
        deps.storage,
        &trade_id.to_be_bytes(),
        &trade_info
    )?;

    // We get all the counter trades for this trade
    let counter_trade_keys: Vec<Vec<u8>> = COUNTER_TRADE_INFO
        .prefix(&trade_id.to_be_bytes())
        .keys(deps.storage, None, None, Order::Ascending)
        .collect();

    // We go through all of them and change their status
    for key in counter_trade_keys {
        COUNTER_TRADE_INFO.update(
            deps.storage,
            (&trade_id.to_be_bytes(), &key),
            |d: Option<TradeInfo>| -> Result<TradeInfo, ContractError> {
                match d {
                    Some(mut one) => {
                        one.state = TradeState::Refused;
                        Ok(one)
                    }
                    None => Err(ContractError::NotFoundInCounterTradeInfo {}),
                }
            },
        )?;
    }


    // TODO We need to make the funds available for withdrawal somehow ?!?!

    Ok(Response::new()
        .add_attribute("cancelled", "trade")
        .add_attribute("trade", trade_id.to_string()))
}
