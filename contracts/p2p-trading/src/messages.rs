use crate::error::ContractError;
use crate::state::{is_trader, load_counter_trade, COUNTER_TRADE_INFO, TRADE_INFO};
use cosmwasm_std::{DepsMut, Env, MessageInfo, Order, Response};
use p2p_trading_export::state::{TradeInfo, TradeState};

pub fn review_counter_trade(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: u64,
    comment: Option<String>,
) -> Result<Response, ContractError> {
    // Only the initial trader can cancel the trade !
    is_trader(deps.storage, &info.sender, trade_id)?;

    // We check the counter trade exists !
    load_counter_trade(deps.storage, trade_id, counter_id)?;

    let mut trade_info = TRADE_INFO.load(deps.storage, &trade_id.to_be_bytes())?;
    if trade_info.state == TradeState::Accepted {
        return Err(ContractError::TradeAlreadyAccepted {});
    }
    if trade_info.state == TradeState::Cancelled {
        return Err(ContractError::TradeCancelled {});
    }

    // We go through all counter trades, to un-publish the counter_id and to update the current trade state
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
                            one.state = TradeState::Created;
                            one.comment = comment.clone();
                        } else if one.state == TradeState::Created {
                            is_acknowledged = true;
                        } else if one.state == TradeState::Published {
                            is_countered = true;
                        }
                        Ok(one)
                    }
                    None => Err(ContractError::NotFoundInCounterTradeInfo {}),
                }
            },
        )?;
    }

    if is_acknowledged {
        trade_info.state = TradeState::Acknowledged;
    }
    if is_countered {
        trade_info.state = TradeState::Countered;
    }

    // Then we need to change the trade status that we may have changed
    TRADE_INFO.save(deps.storage, &trade_id.to_be_bytes(), &trade_info)?;

    Ok(Response::new()
        .add_attribute("review", "counter")
        .add_attribute("trade", trade_id.to_string())
        .add_attribute("counter", counter_id.to_string()))
}
