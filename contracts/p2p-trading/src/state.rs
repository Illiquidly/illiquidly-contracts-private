use cw_storage_plus::{Item, Map};

use cosmwasm_std::{Storage, Addr, StdError, Coin, StdResult, Uint128};
use cw20::{Cw20Coin};

use p2p_trading_export::state::{ContractInfo, TradeInfo, FundsInfo};
use p2p_trading_export::msg::{
    Cw721Coin
};

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");

pub const TRADE_INFO: Map<&[u8], TradeInfo> = Map::new("trade_info");

pub const COUNTER_TRADE_INFO: Map<(&[u8], &[u8]), TradeInfo> = Map::new("counter_trade_info");


pub fn add_funds(
    funds: Vec<Coin>
) -> impl FnOnce(Option<TradeInfo>) -> StdResult<TradeInfo> {

    move |d: Option<TradeInfo>| -> StdResult<TradeInfo> {
        match d {
            Some(mut one) => {
                one.associated_funds.extend(
                    funds
                        .iter()
                        .map(|x| FundsInfo::Coin(x.clone()))
                        .collect::<Vec<FundsInfo>>(),
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
                one.associated_funds.push(FundsInfo::Cw20Coin(Cw20Coin {
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
                one.associated_funds.push(FundsInfo::Cw721Coin(Cw721Coin {
                    address: address.into(),
                    token_id: token_id,
                }));
                Ok(one)
            }
            None => Err(StdError::GenericErr {
                msg: "Trade Id not found !".to_string(),
            }),
        }
    }
}

pub fn is_trader(storage: &dyn Storage, sender: &Addr, trade_id: u64) -> bool {
    if let Ok(Some(trade)) = TRADE_INFO.may_load(storage, &trade_id.to_be_bytes()) {
        if trade.owner == sender.clone() {
            return true;
        }
    }
    false
}

pub fn is_counter_trader(storage: &dyn Storage, sender: &Addr, trade_id: u64, counter_id: u64) -> bool {
    if let Ok(Some(trade)) = COUNTER_TRADE_INFO.may_load(storage, (&trade_id.to_be_bytes(),&counter_id.to_be_bytes())) {
        if trade.owner == sender.clone() {
            return true;
        }
    }
    false
}

