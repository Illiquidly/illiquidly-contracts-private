use cosmwasm_std::{
    BankMsg, Coin, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Uint128,
};

use crate::error::ContractError;
use crate::state::{
    add_cw20_coin, add_cw721_coin, add_funds, is_trader, load_counter_trade, CONTRACT_INFO,
    COUNTER_TRADE_INFO, TRADE_INFO,
};
use cw20::Cw20ExecuteMsg;
use cw721::Cw721ExecuteMsg;
use p2p_trading_export::msg::into_cosmos_msg;
use p2p_trading_export::state::{AcceptedTradeInfo, AssetInfo, TradeInfo, TradeState};

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
                accepted_info: None,
                assets_withdrawn: false,
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
    if trade_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: trade_info.state,
        });
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
    let trade_info = is_trader(deps.storage, &deps.api.addr_validate(&trader)?, trade_id)?;

    if trade_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: trade_info.state,
        });
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
    let trade_info = is_trader(deps.storage, &deps.api.addr_validate(&trader)?, trade_id)?;

    if trade_info.state != TradeState::Created {
        return Err(ContractError::WrongTradeState {
            state: trade_info.state,
        });
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
    let accepted_info = AcceptedTradeInfo {
        trade_id,
        counter_id,
    };
    trade_info.state = TradeState::Accepted;
    trade_info.accepted_info = Some(accepted_info);

    counter_info.state = TradeState::Accepted;

    // And we save that to storage
    TRADE_INFO.save(deps.storage, &trade_id.to_be_bytes(), &trade_info)?;
    COUNTER_TRADE_INFO.save(deps.storage, (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()), &counter_info)?;






    /*
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
    */


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

    TRADE_INFO.save(deps.storage, &trade_id.to_be_bytes(),&trade_info)?;


    /*
    // We go through all counter trades, to cancel the counter_id and to update the current trade state
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
    TRADE_INFO.save(deps.storage, &trade_id.to_be_bytes(), &trade_info)?;

    Ok(Response::new()
        .add_attribute("cancelled", "trade")
        .add_attribute("trade", trade_id.to_string()))
}

pub fn withdraw_trade_assets_while_creating(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
    assets: Vec<(usize, AssetInfo)>,
    funds: Vec<(usize, Coin)>,
) -> Result<Response, ContractError> {
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;
    if trade_info.state != TradeState::Created {
        return Err(ContractError::TradeAlreadyPublished {});
    }

    are_assets_in_trade(&trade_info, &assets, &funds)?;

    try_withdraw_assets_unsafe(&mut trade_info, &assets, &funds)?;

    TRADE_INFO.save(deps.storage, &trade_id.to_be_bytes(), &trade_info)?;

    let res = create_withdraw_messages(
        info.clone(),
        &assets.iter().map(|x| x.1.clone()).collect(),
        &funds.iter().map(|x| x.1.clone()).collect(),
    )?;
    Ok(res.add_attribute("remove from", "trade"))
}

pub fn are_assets_in_trade(
    trade_info: &TradeInfo,
    assets: &Vec<(usize, AssetInfo)>,
    funds: &Vec<(usize, Coin)>,
) -> Result<(), ContractError> {
    // We first treat the assets
    for (position, asset) in assets {
        if *position >= trade_info.associated_assets.len() {
            return Err(ContractError::Std(StdError::generic_err(
                "assets position does not exist in array",
            )));
        }
        let asset_info: AssetInfo = trade_info.associated_assets[*position].clone();
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
        }
    }

    // Then we take care of the funds
    for (position, fund) in funds {
        if *position >= trade_info.associated_funds.len() {
            return Err(ContractError::Std(StdError::generic_err(
                "assets position does not exist in array",
            )));
        }
        let fund_info = trade_info.associated_funds[*position].clone();
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
    assets: &Vec<(usize, AssetInfo)>,
    funds: &Vec<(usize, Coin)>,
) -> Result<(), ContractError> {
    for (position, asset) in assets {
        let asset_info = trade_info.associated_assets[*position].clone();
        match asset_info {
            AssetInfo::Cw20Coin(mut token_info) => {
                // We check the token is the one we want
                if let AssetInfo::Cw20Coin(token) = asset {
                    token_info.amount -= token.amount;
                    trade_info.associated_assets[*position] = AssetInfo::Cw20Coin(token_info);
                }
            }
            AssetInfo::Cw721Coin(mut nft_info) => {
                // We check the token is the one we want
                if let AssetInfo::Cw721Coin(_) = asset {
                    nft_info.address = "".to_string();
                    trade_info.associated_assets[*position] = AssetInfo::Cw721Coin(nft_info);
                }
            }
        }
    }

    // Then we remove empty funds from the trade
    trade_info.associated_assets.retain(|asset| match asset {
        AssetInfo::Cw20Coin(token) => token.amount != Uint128::zero(),
        AssetInfo::Cw721Coin(nft) => nft.address != "",
    });

    // Then we take care of the wanted funds
    // First, we check funds availability and update their state
    for (position, fund) in funds {
        let mut fund_info = trade_info.associated_funds[*position].clone();

        // If everything is in order, we remove the coin from the trade
        fund_info.amount -= fund.amount;
        trade_info.associated_funds[*position] = fund_info;
    }

    // Then we remove empty funds
    trade_info
        .associated_funds
        .retain(|fund| fund.amount != Uint128::zero());

    Ok(())
}

pub fn create_withdraw_messages(
    info: MessageInfo,
    assets: &Vec<AssetInfo>,
    funds: &Vec<Coin>,
) -> Result<Response, ContractError> {
    let mut res = Response::new();

    // First the assets
    for asset in assets {
        match asset {
            AssetInfo::Cw20Coin(token) => {
                let message = Cw20ExecuteMsg::Transfer {
                    recipient: info.sender.to_string(),
                    amount: token.amount,
                };
                res = res.add_message(into_cosmos_msg(message, token.address.clone())?);
            }
            AssetInfo::Cw721Coin(nft) => {
                let message = Cw721ExecuteMsg::TransferNft {
                    recipient: info.sender.to_string(),
                    token_id: nft.token_id.clone(),
                };
                res = res.add_message(into_cosmos_msg(message, nft.address.clone())?);
            }
        }
    }

    // Then the funds
    if !funds.is_empty() {
        res = res.add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: funds.iter().map(|x| x.clone()).collect(),
        });
    };

    Ok(res)
}
