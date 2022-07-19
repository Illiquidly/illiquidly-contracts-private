use anyhow::{Result, anyhow};
use cw_storage_plus::{Item, Map};

use cosmwasm_std::{Env, Addr, Coin, StdError, StdResult, Storage, Uint128};

use crate::error::ContractError;
use raffles_export::state::{
    AssetInfo, ContractInfo, Cw1155Coin, Cw20Coin, Cw721Coin, RaffleInfo, RaffleState,
};

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");

pub const RAFFLE_INFO: Map<u64, RaffleInfo> = Map::new("raffle_info");
pub const USER_TICKETS: Map<(&Addr, u64), u64> = Map::new("uset_tickets");

pub fn add_funds(
    fund: Coin,
    info_funds: Vec<Coin>,
) -> impl FnOnce(Option<TradeInfo>) -> Result<TradeInfo, ContractError> {
    move |d: Option<TradeInfo>| -> Result<TradeInfo, ContractError> {
        match d {
            Some(mut trade) => {
                // We check the sent funds are with the right format
                if info_funds.len() != 1 || fund != info_funds[0] {
                    return Err(ContractError::Std(StdError::generic_err(
                        "Funds sent do not match message AssetInfo",
                    )));
                }
                let existing_denom = trade.associated_assets.iter_mut().find(|c| match c {
                    AssetInfo::Coin(x) => x.denom == fund.denom,
                    _ => false,
                });

                if let Some(existing_fund) = existing_denom {
                    let current_amount = match existing_fund {
                        AssetInfo::Coin(x) => x.amount,
                        _ => Uint128::zero(),
                    };
                    *existing_fund = AssetInfo::Coin(Coin {
                        denom: fund.denom,
                        amount: current_amount + fund.amount,
                    });
                } else {
                    trade.associated_assets.push(AssetInfo::Coin(fund));
                }
                Ok(trade)
            }
            //TARPAULIN : Unreachable in current code state
            None => Err(ContractError::NotFoundInTradeInfo {}),
        }
    }
}

pub fn add_cw20_coin(
    address: String,
    sent_amount: Uint128,
) -> impl FnOnce(Option<TradeInfo>) -> Result<TradeInfo, ContractError> {
    move |d: Option<TradeInfo>| -> Result<TradeInfo, ContractError> {
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
            None => Err(ContractError::NotFoundInTradeInfo {}),
        }
    }
}

pub fn add_cw721_coin(
    address: String,
    token_id: String,
) -> impl FnOnce(Option<TradeInfo>) -> Result<TradeInfo, ContractError> {
    move |d: Option<TradeInfo>| -> Result<TradeInfo, ContractError> {
        match d {
            Some(mut one) => {
                one.associated_assets
                    .push(AssetInfo::Cw721Coin(Cw721Coin { address, token_id }));
                Ok(one)
            }
            //TARPAULIN : Unreachable in current code state
            None => Err(ContractError::NotFoundInTradeInfo {}),
        }
    }
}

pub fn add_cw1155_coin(
    address: String,
    token_id: String,
    value: Uint128,
) -> impl FnOnce(Option<TradeInfo>) -> Result<TradeInfo, ContractError> {
    move |d: Option<TradeInfo>| -> Result<TradeInfo, ContractError> {
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
            None => Err(ContractError::NotFoundInTradeInfo {}),
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

pub fn get_raffle_state(
    storage: &dyn Storage,
    env: Env,
    raffle_info: RaffleInfo,
) -> Result<RaffleState> {
    let state = 
    if env.block.time < raffle_info.raffle_start_timestamp{
        RaffleState::Created
    }else if env.block.time < raffle_info.raffle_start_timestamp.plus_seconds(raffle_info.raffle_duration){
        RaffleState::Started
    }else if env.block.time < raffle_info.raffle_start_timestamp.plus_seconds(raffle_info.raffle_duration).plus_seconds(raffle_info.raffle_timeout){
        RaffleState::Closed
    }else{
        RaffleState::Finished
    };
    Ok(state)
}

pub fn load_ticket_number(
    storage: &dyn Storage,
    raffle_id: u64,
    owner: Addr,
) -> Result<u64> {
    let raffle_info = RAFFLE_INFO
        .load(storage, raffle_id)
        .map_err(|_| ContractError::NotFoundInRaffleInfo {})?;

    Ok(raffle_info.tickets.iter().filter(|&ticket_owner| *ticket_owner == owner).count() as u64)
}

pub fn load_raffle(storage: &dyn Storage, raffle_id: u64) -> Result<RaffleInfo, ContractError> {
    RAFFLE_INFO
        .load(storage, raffle_id)
        .map_err(|_| ContractError::NotFoundInTradeInfo {})
}

pub fn can_buy_ticket(
    storage: &dyn Storage,
    raffle_info: RaffleInfo,
    env: Env,
) -> Result<()> {

    if get_raffle_state(storage, env, raffle_info)? == RaffleState::Started{
        Ok(())
    }else{
        Err(anyhow!(ContractError::CantBuyTickets {}))
    }
}

