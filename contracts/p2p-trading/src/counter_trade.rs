use cosmwasm_std::{
    DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128
};

use crate::state::{ 
	is_counter_trader, 
	add_funds, add_cw20_coin, add_cw721_coin, 
	COUNTER_TRADE_INFO, TRADE_INFO
};
use p2p_trading_export::state::{TradeInfo, TradeState, FundsInfo};
use crate::error::ContractError;


pub fn suggest_counter_trade(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trade_id: u64,
    confirm: Option<bool>
) -> Result<Response, ContractError> {
    // We start by creating a new trade_id (simply incremented from the last id)
    let new_trade_info = TRADE_INFO
        .update(deps.storage, &trade_id.to_be_bytes(),|c| -> StdResult<_> {
            match c {
                Some(mut trade_info) => {
                    match trade_info.last_counter_id {
                        Some(last_counter_id) => trade_info.last_counter_id = Some(last_counter_id + 1),
                        None => trade_info.last_counter_id = Some(0),
                    }
                    trade_info.state = TradeState::Acknowledged;
                    Ok(trade_info)
                },
                None => Err(StdError::GenericErr {
                    msg: "Trade Id not found !".to_string(),
                }),
            }
        })?;

    let counter_id = new_trade_info.last_counter_id.unwrap(); // This is safe, as per the statement above.

    // If the counter trade id already exists, we have a problem !!! 
    // (we do not want to overwrite existing data)
    if COUNTER_TRADE_INFO.has(deps.storage, (&trade_id.to_be_bytes(),&counter_id.to_be_bytes())) {
        return Err(ContractError::ExistsInCounterTradeInfo {});
    } else {
        // We create the TradeInfo in advance to prevent other calls to front run this one
        COUNTER_TRADE_INFO.save(
            deps.storage,
            (&trade_id.to_be_bytes(),&counter_id.to_be_bytes()),
            &TradeInfo {
                owner: info.sender.clone(),
                // We add the funds sent along with this transaction
                associated_funds: info
                    .funds
                    .iter()
                    .map(|x| FundsInfo::Coin(x.clone()))
                    .collect(),
                state: TradeState::Created,
                last_counter_id:None,
            },
        )?;
    }


    if let Some(confirmed) = confirm {
        if confirmed {
            confirm_counter_trade(deps, env, info.clone(), trade_id, counter_id)?;
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
    if !is_counter_trader(deps.storage, &info.sender, trade_id, counter_id) {
        return Err(ContractError::CounterTraderNotCreator {});
    }

    let update_result = COUNTER_TRADE_INFO.update(
        deps.storage,
        (&trade_id.to_be_bytes(),&counter_id.to_be_bytes()),
        add_funds(info.funds.clone())
    );

    if let Some(confirmed) = confirm {
        if confirmed {
            confirm_counter_trade(deps, env, info.clone(), trade_id, counter_id)?;
        }
    }
    if update_result.is_err() {
        return Err(ContractError::NotFoundInCounterTradeInfo{});
    }

    Ok(Response::new()
        .add_attribute("added funds", info.sender.to_string())
        .add_attribute("trade", trade_id.to_string())
        .add_attribute("counter", counter_id.to_string())
    )
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
    if !is_counter_trader(deps.storage, &deps.api.addr_validate(&trader)?, trade_id, counter_id) {
        return Err(ContractError::TraderNotCreator {});
    }

    let update_result = COUNTER_TRADE_INFO.update(
        deps.storage,
        (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()),
        add_cw20_coin(info.sender.clone(), sent_amount),
    );

    if update_result.is_err() {
        return Err(ContractError::NotFoundInCounterTradeInfo {});
    }

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

    if !is_counter_trader(deps.storage, &deps.api.addr_validate(&trader)?, trade_id, counter_id) {
        return Err(ContractError::TraderNotCreator {});
    }

    let update_result = COUNTER_TRADE_INFO.update(
        deps.storage,
        (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()),
        add_cw721_coin(info.sender.clone(), token_id.clone()),
    );

    if update_result.is_err() {
        return Err(ContractError::NotFoundInCounterTradeInfo {});
    }

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

    if !is_counter_trader(deps.storage, &info.sender, trade_id, counter_id) {
        return Err(ContractError::TraderNotCreator {});
    }

    // We update the trade_info to show there are some suggested counters
    let update_result = TRADE_INFO.update(
        deps.storage,
        &trade_id.to_be_bytes(),
        |d: Option<TradeInfo>| -> StdResult<TradeInfo> {
            match d {
                Some(mut one) => {
                    one.state = TradeState::Countered;
                    Ok(one)
                }
                None => Err(StdError::GenericErr {
                    msg: "Id not found !".to_string(),
                }),
            }
        },
    );
    if update_result.is_err() {
        return Err(ContractError::NotFoundInTradeInfo {});
    }

    // We update the trade_info to show there are some suggested counters
    let update_result = COUNTER_TRADE_INFO.update(
        deps.storage,
        (&trade_id.to_be_bytes(),&counter_id.to_be_bytes()),
        |d: Option<TradeInfo>| -> StdResult<TradeInfo> {
            match d {
                Some(mut one) => {
                    one.state = TradeState::Published;
                    Ok(one)
                }
                None => Err(StdError::GenericErr {
                    msg: "Id not found !".to_string(),
                }),
            }
        },
    );
    if update_result.is_err() {
        return Err(ContractError::NotFoundInCounterTradeInfo {});
    }


    Ok(Response::new()
        .add_attribute("confirmed","counter")
        .add_attribute("trade",trade_id.to_string())
        .add_attribute("counter",counter_id.to_string())
    )
}
