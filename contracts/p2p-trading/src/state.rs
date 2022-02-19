use cw_storage_plus::{Item, Map};

use cosmwasm_std::{Addr, Coin, StdError, StdResult, Storage, Uint128};
use cw20::Cw20Coin;

use crate::error::ContractError;
use p2p_trading_export::msg::Cw721Coin;
use p2p_trading_export::state::{ContractInfo, AssetInfo, TradeInfo, TradeState};

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");

pub const TRADE_INFO: Map<&[u8], TradeInfo> = Map::new("trade_info");

pub const COUNTER_TRADE_INFO: Map<(&[u8], &[u8]), TradeInfo> = Map::new("counter_trade_info");

pub fn add_funds(funds: Vec<Coin>) -> impl FnOnce(Option<TradeInfo>) -> StdResult<TradeInfo> {
    move |d: Option<TradeInfo>| -> StdResult<TradeInfo> {
        match d {
            Some(mut one) => {
                one.associated_funds.extend(
                    funds
                );
                Ok(one)
            }
            None => Err(StdError::GenericErr {
                msg: "Trade Id not found !".to_string(),
            }),
        }
    }
}

pub fn add_cw20_coin(
    address: Addr,
    sent_amount: Uint128,
) -> impl FnOnce(Option<TradeInfo>) -> StdResult<TradeInfo> {
    move |d: Option<TradeInfo>| -> StdResult<TradeInfo> {
        match d {
            Some(mut one) => {
                one.associated_assets.push(AssetInfo::Cw20Coin(Cw20Coin {
                    address: address.into(),
                    amount: sent_amount,
                }));
                Ok(one)
            }
            None => Err(StdError::GenericErr {
                msg: "Trade Id not found !".to_string(),
            }),
        }
    }
}

pub fn add_cw721_coin(
    address: Addr,
    token_id: String,
) -> impl FnOnce(Option<TradeInfo>) -> StdResult<TradeInfo> {
    move |d: Option<TradeInfo>| -> StdResult<TradeInfo> {
        match d {
            Some(mut one) => {
                one.associated_assets.push(AssetInfo::Cw721Coin(Cw721Coin {
                    address: address.into(),
                    token_id,
                }));
                Ok(one)
            }
            None => Err(StdError::GenericErr {
                msg: "Trade Id not found !".to_string(),
            }),
        }
    }
}

pub fn is_trader(storage: &dyn Storage, sender: &Addr, trade_id: u64) -> Result<(), ContractError> {
    if let Ok(Some(trade)) = TRADE_INFO.may_load(storage, &trade_id.to_be_bytes()) {
        if trade.owner == sender.clone() {
            Ok(())
        } else {
            Err(ContractError::TraderNotCreator {})
        }
    } else {
        Err(ContractError::NotFoundInTradeInfo {})
    }
}

pub fn is_counter_trader(
    storage: &dyn Storage,
    sender: &Addr,
    trade_id: u64,
    counter_id: u64,
) -> Result<(), ContractError> {
    if let Ok(Some(trade)) = COUNTER_TRADE_INFO.may_load(
        storage,
        (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()),
    ) {
        if trade.owner == sender.clone() {
            Ok(())
        } else {
            Err(ContractError::CounterTraderNotCreator {})
        }
    } else {
        Err(ContractError::NotFoundInCounterTradeInfo {})
    }
}

pub fn load_counter_trade(
    storage: &dyn Storage,
    trade_id: u64,
    counter_id: u64,
) -> Result<TradeInfo, ContractError> {
    COUNTER_TRADE_INFO
        .load(
            storage,
            (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()),
        )
        .map_err(|_| ContractError::NotFoundInCounterTradeInfo {})
}

pub fn can_suggest_counter_trade(
    storage: &dyn Storage,
    trade_id: u64,
) -> Result<(), ContractError> {
    if let Ok(Some(trade)) = TRADE_INFO.may_load(storage, &trade_id.to_be_bytes()) {
        if (trade.state == TradeState::Published)
            | (trade.state == TradeState::Acknowledged)
            | (trade.state == TradeState::Countered)
        {
            Ok(())
        } else {
            Err(ContractError::NotCounterable {})
        }
    } else {
        Err(ContractError::NotFoundInTradeInfo {})
    }
}
