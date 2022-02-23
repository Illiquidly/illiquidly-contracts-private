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
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;

    // We check the counter trade exists !
    let mut counter_info = load_counter_trade(deps.storage, trade_id, counter_id)?;

    if trade_info.state == TradeState::Accepted {
        return Err(ContractError::TradeAlreadyAccepted {});
    }
    if trade_info.state == TradeState::Cancelled {
        return Err(ContractError::TradeCancelled {});
    }

    // Only a published counter trade can be reviewed
    if counter_info.state != TradeState::Published {
        return Err(ContractError::CantChangeCounterTradeState {
            from: counter_info.state,
            to: TradeState::Created,
        });
    }

    counter_info.state = TradeState::Created;
    counter_info.comment = comment.clone();

    // Then we need to change the trade status that we may have changed
    TRADE_INFO.save(deps.storage, &trade_id.to_be_bytes(), &trade_info)?; 
    COUNTER_TRADE_INFO.save(deps.storage, (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()), &counter_info)?;


    Ok(Response::new()
        .add_attribute("review", "counter")
        .add_attribute("trade", trade_id.to_string())
        .add_attribute("counter", counter_id.to_string()))
}
