use cosmwasm_std::{
    DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128
};

use crate::state::{ is_trader, 
    add_funds, add_cw20_coin, add_cw721_coin, 
    CONTRACT_INFO, TRADE_INFO
};
use p2p_trading_export::state::{TradeInfo, TradeState, FundsInfo};
use crate::error::ContractError;

pub fn create_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // We start by creating a new trade_id (simply incremented from the last id)
    let trade_id: u64 = CONTRACT_INFO
        .update(deps.storage, |mut c| -> StdResult<_> {
            c.last_trade_id += 1;
            Ok(c)
        })?
        .last_trade_id;

    // If the trade id already exists, we have a problem !!! (we do not want to overwrite existing data)
    if TRADE_INFO.has(deps.storage, &trade_id.to_be_bytes()) {
        return Err(ContractError::ExistsInTradeInfo {});
    } else {
        // We create the TradeInfo in advance to prevent other calls to front run this one
        TRADE_INFO.save(
            deps.storage,
            &trade_id.to_be_bytes(),
            &TradeInfo {
                owner: info.sender.clone(),
                // We add the funds sent along with this transaction
                associated_funds: info
                    .funds
                    .iter()
                    .map(|x| FundsInfo::Coin(x.clone()))
                    .collect(),
                state: TradeState::Created,
                last_counter_id:None
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
    if !is_trader(deps.storage, &info.sender, trade_id) {
        return Err(ContractError::TraderNotCreator {});
    }

    let update_result = TRADE_INFO.update(
        deps.storage,
        &trade_id.to_be_bytes(),
        add_funds(info.funds.clone()),
    );

    if let Some(confirmed) = confirm {
        if confirmed {
            confirm_trade(deps, env, info.clone(), trade_id)?;
        }
    }
    if update_result.is_err() {
        return Err(ContractError::NotFoundInTradeInfo {});
    }

    Ok(Response::new()
        .add_attribute("added funds", info.sender.to_string())
        .add_attribute("trade", trade_id.to_string()))
}

pub fn add_token_to_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trader: String,
    trade_id: u64,
    sent_amount: Uint128,
) -> Result<Response, ContractError> {
    if !is_trader(deps.storage, &deps.api.addr_validate(&trader)?, trade_id) {
        return Err(ContractError::TraderNotCreator {});
    }

    let update_result = TRADE_INFO.update(
        deps.storage,
        &trade_id.to_be_bytes(),
        add_cw20_coin(info.sender.clone(), sent_amount),
    );

    if update_result.is_err() {
        return Err(ContractError::NotFoundInTradeInfo {});
    }

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
    if !is_trader(deps.storage, &deps.api.addr_validate(&trader)?, trade_id) {
        return Err(ContractError::TraderNotCreator {});
    }

    let update_result = TRADE_INFO.update(
        deps.storage,
        &trade_id.to_be_bytes(),
        add_cw721_coin(info.sender.clone(), token_id.clone()),
    );

    if update_result.is_err() {
        return Err(ContractError::NotFoundInTradeInfo {});
    }

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
    if !is_trader(deps.storage, &info.sender, trade_id) {
        return Err(ContractError::TraderNotCreator {});
    }

    let update_result = TRADE_INFO.update(
        deps.storage,
        &trade_id.to_be_bytes(),
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
        return Err(ContractError::NotFoundInTradeInfo {});
    }

    Ok(Response::new()
        .add_attribute("confirmed","trade")
        .add_attribute("trade",trade_id.to_string())
    )
}