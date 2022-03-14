use cw_storage_plus::{Item, Map, U64Key};

use cosmwasm_std::{Addr, Coin, StdError, StdResult, Storage, Uint128};

use crate::error::ContractError;
use p2p_trading_export::state::{
    AssetInfo, ContractInfo, Cw1155Coin, Cw20Coin, Cw721Coin, TradeInfo, TradeState,
};

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");

pub const TRADE_INFO: Map<U64Key, TradeInfo> = Map::new("trade_info");

pub const COUNTER_TRADE_INFO: Map<(U64Key, U64Key), TradeInfo> = Map::new("counter_trade_info");

pub const USER_COUNTERED_TRADES: Map<Addr, Vec<u64>> = Map::new("user_countered_trades");

pub fn add_funds(funds: Vec<Coin>) -> impl FnOnce(Option<TradeInfo>) -> StdResult<TradeInfo> {
    move |d: Option<TradeInfo>| -> StdResult<TradeInfo> {
        match d {
            Some(mut trade) => {
                for fund in funds {
                    let existing_denom = trade
                        .associated_funds
                        .iter_mut()
                        .find(|c| c.denom == fund.denom);
                    if let Some(existing_fund) = existing_denom {
                        existing_fund.amount += fund.amount
                    } else {
                        trade.associated_funds.push(fund)
                    }
                }
                Ok(trade)
            }
            //TARPAULIN : Unreachable in current code state
            None => Err(StdError::GenericErr {
                msg: "Trade Id not found !".to_string(),
            }),
        }
    }
}

pub fn add_cw20_coin(
    address: String,
    sent_amount: Uint128,
) -> impl FnOnce(Option<TradeInfo>) -> StdResult<TradeInfo> {
    move |d: Option<TradeInfo>| -> StdResult<TradeInfo> {
        match d {
            Some(mut trade) => {
                let existing_token = trade.associated_assets.iter_mut().find(|c| match c {
                    AssetInfo::Cw20Coin(x) => x.address == address,
                    _ => false,
                });
                if let Some(existing_token) = existing_token {
                    let current_amount = match existing_token {
                        AssetInfo::Cw20Coin(x) => x.amount,
                        _ => Uint128::zero(),
                    };
                    *existing_token = AssetInfo::Cw20Coin(Cw20Coin {
                        address,
                        amount: current_amount + sent_amount,
                    })
                } else {
                    trade.associated_assets.push(AssetInfo::Cw20Coin(Cw20Coin {
                        address,
                        amount: sent_amount,
                    }))
                }

                Ok(trade)
            }
            //TARPAULIN : Unreachable in current code state
            None => Err(StdError::GenericErr {
                msg: "Trade Id not found !".to_string(),
            }),
        }
    }
}

pub fn add_cw721_coin(
    address: String,
    token_id: String,
) -> impl FnOnce(Option<TradeInfo>) -> StdResult<TradeInfo> {
    move |d: Option<TradeInfo>| -> StdResult<TradeInfo> {
        match d {
            Some(mut one) => {
                one.associated_assets
                    .push(AssetInfo::Cw721Coin(Cw721Coin { address, token_id }));
                Ok(one)
            }
            //TARPAULIN : Unreachable in current code state
            None => Err(StdError::GenericErr {
                msg: "Trade Id not found !".to_string(),
            }),
        }
    }
}

pub fn add_cw1155_coin(
    address: String,
    token_id: String,
    value: Uint128,
) -> impl FnOnce(Option<TradeInfo>) -> StdResult<TradeInfo> {
    move |d: Option<TradeInfo>| -> StdResult<TradeInfo> {
        match d {
            Some(mut trade) => {
                let existing_token = trade.associated_assets.iter_mut().find(|c| match c {
                    AssetInfo::Cw1155Coin(x) => x.address == address && x.token_id == token_id,
                    _ => false,
                });
                if let Some(existing_token) = existing_token {
                    let current_value = match existing_token {
                        AssetInfo::Cw1155Coin(x) => x.value,
                        _ => Uint128::zero(),
                    };
                    *existing_token = AssetInfo::Cw1155Coin(Cw1155Coin {
                        address,
                        token_id,
                        value: current_value + value,
                    })
                } else {
                    trade
                        .associated_assets
                        .push(AssetInfo::Cw1155Coin(Cw1155Coin {
                            address,
                            token_id,
                            value,
                        }))
                }

                Ok(trade)
            }
            //TARPAULIN : Unreachable in current code state
            None => Err(StdError::GenericErr {
                msg: "Trade Id not found !".to_string(),
            }),
        }
    }
}

pub fn is_owner(storage: &dyn Storage, sender: Addr) -> Result<ContractInfo, ContractError> {
    let contract_info = CONTRACT_INFO.load(storage)?;
    if sender == contract_info.owner {
        Ok(contract_info)
    } else {
        Err(ContractError::Unauthorized {})
    }
}

pub fn is_fee_contract(storage: &dyn Storage, sender: Addr) -> Result<(), ContractError> {
    let contract_info = CONTRACT_INFO.load(storage)?;
    if let Some(fee_contract) = contract_info.fee_contract {
        if sender == fee_contract {
            Ok(())
        } else {
            Err(ContractError::Unauthorized {})
        }
    } else {
        Err(ContractError::Unauthorized {})
    }
}

pub fn is_trader(
    storage: &dyn Storage,
    sender: &Addr,
    trade_id: u64,
) -> Result<TradeInfo, ContractError> {
    let trade = load_trade(storage, trade_id)?;

    if trade.owner == sender.clone() {
        Ok(trade)
    } else {
        Err(ContractError::TraderNotCreator {})
    }
}

pub fn is_counter_trader(
    storage: &dyn Storage,
    sender: &Addr,
    trade_id: u64,
    counter_id: u64,
) -> Result<TradeInfo, ContractError> {
    let trade = load_counter_trade(storage, trade_id, counter_id)?;

    if trade.owner == sender.clone() {
        Ok(trade)
    } else {
        Err(ContractError::CounterTraderNotCreator {})
    }
}

pub fn load_counter_trade(
    storage: &dyn Storage,
    trade_id: u64,
    counter_id: u64,
) -> Result<TradeInfo, ContractError> {
    COUNTER_TRADE_INFO
        .load(storage, (trade_id.into(), counter_id.into()))
        .map_err(|_| ContractError::NotFoundInCounterTradeInfo {})
}

pub fn load_trade(storage: &dyn Storage, trade_id: u64) -> Result<TradeInfo, ContractError> {
    TRADE_INFO
        .load(storage, trade_id.into())
        .map_err(|_| ContractError::NotFoundInTradeInfo {})
}

pub fn can_suggest_counter_trade(
    storage: &dyn Storage,
    trade_id: u64,
    sender: &Addr,
) -> Result<(), ContractError> {
    if let Ok(Some(trade)) = TRADE_INFO.may_load(storage, trade_id.into()) {
        if (trade.state == TradeState::Published) | (trade.state == TradeState::Countered) {
            if !trade.whitelisted_users.is_empty() {
                if !trade.whitelisted_users.contains(sender) {
                    Err(ContractError::AddressNotWhitelisted {})
                } else {
                    Ok(())
                }
            } else {
                Ok(())
            }
        } else {
            Err(ContractError::NotCounterable {})
        }
    } else {
        Err(ContractError::NotFoundInTradeInfo {})
    }
}
