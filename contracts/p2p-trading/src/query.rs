use cosmwasm_std::{Api, Pair};
#[cfg(not(feature = "library"))]
use cosmwasm_std::{Deps, Order, StdResult};

use cw_storage_plus::{Bound, PrimaryKey, U64Key};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;

use crate::state::{CONTRACT_INFO, COUNTER_TRADE_INFO, TRADE_INFO};
use p2p_trading_export::msg::QueryFilters;
use p2p_trading_export::state::{AssetInfo, ContractInfo, CounterTradeInfo, TradeInfo};

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct TradeResponse {
    pub trade_id: u64,
    pub counter_id: Option<u64>,
    pub trade_info: TradeInfo,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct AllTradesResponse {
    pub trades: Vec<TradeResponse>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct AllCounterTradesResponse {
    pub counter_trades: Vec<TradeResponse>,
}

pub fn query_contract_info(deps: Deps) -> StdResult<ContractInfo> {
    CONTRACT_INFO.load(deps.storage)
}

// parse trades to human readable format
fn parse_trades(_: &dyn Api, item: StdResult<Pair<TradeInfo>>) -> StdResult<TradeResponse> {
    item.map(|(k, trade)| {
        let trade_id = k.try_into().unwrap();
        TradeResponse {
            trade_id: u64::from_be_bytes(trade_id),
            counter_id: None,
            trade_info: trade,
        }
    })
}

pub fn trade_filter(
    api: &dyn Api,
    trade_info: &StdResult<TradeResponse>,
    filters: &Option<QueryFilters>,
) -> bool {
    if let Some(filters) = filters {
        let trade = trade_info.as_ref().unwrap();

        (match &filters.states {
            Some(state) => state.contains(&trade.trade_info.state.to_string()),
            None => true,
        } && match &filters.owner {
            Some(owner) => trade.trade_info.owner == owner.clone(),
            None => true,
        } && match &filters.whitelisted_user {
            Some(whitelisted_user) => trade
                .trade_info
                .whitelisted_users
                .contains(&api.addr_validate(whitelisted_user).unwrap()),
            None => true,
        } && match &filters.wanted_nft {
            Some(wanted_nft) => trade
                .trade_info
                .additionnal_info
                .nfts_wanted
                .contains(&api.addr_validate(wanted_nft).unwrap()),
            None => true,
        } && match &filters.contains_token {
            Some(token) => trade
                .trade_info
                .associated_assets
                .iter()
                .any(|asset| match asset {
                    AssetInfo::Cw20Coin(x) => x.address == token.as_ref(),
                    AssetInfo::Cw721Coin(x) => x.address == token.as_ref(),
                }),
            None => true,
        })
    } else {
        true
    }
}

pub fn query_all_trades(
    deps: Deps,
    start_after: Option<u64>,
    limit: Option<u32>,
    filters: Option<QueryFilters>,
) -> StdResult<AllTradesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(|s| Bound::Exclusive(U64Key::new(s).joined_key()));

    let trades: StdResult<Vec<TradeResponse>> = TRADE_INFO
        .range(deps.storage, None, start, Order::Descending)
        .map(|kv_item| parse_trades(deps.api, kv_item))
        .filter(|response| trade_filter(deps.api, response, &filters))
        .take(limit)
        .collect();

    Ok(AllTradesResponse { trades: trades? })
}

// parse counter trades to human readable format
fn parse_all_counter_trades(
    _: &dyn Api,
    item: StdResult<Pair<TradeInfo>>,
) -> StdResult<TradeResponse> {
    item.map(|(ck, trade)| {
        // First two bytes define size [0,8] since we know it's u64 skip it.
        let (trade_id, counter_id) = (&ck[2..10], &ck[10..]);
        let trade_id = trade_id.try_into().unwrap();
        let counter_id = counter_id.try_into().unwrap();

        TradeResponse {
            trade_id: u64::from_be_bytes(trade_id),
            counter_id: Some(u64::from_be_bytes(counter_id)),
            trade_info: trade,
        }
    })
}

pub fn query_all_counter_trades(
    deps: Deps,
    start_after: Option<CounterTradeInfo>,
    limit: Option<u32>,
    filters: Option<QueryFilters>,
) -> StdResult<AllCounterTradesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let start = start_after.map(|s| {
        Bound::Exclusive((U64Key::new(s.trade_id), U64Key::new(s.counter_id)).joined_key())
    });

    let counter_trades: StdResult<Vec<TradeResponse>> = COUNTER_TRADE_INFO
        .range(deps.storage, None, start, Order::Descending)
        .map(|kv_item| parse_all_counter_trades(deps.api, kv_item))
        .filter(|response| trade_filter(deps.api, response, &filters))
        .take(limit)
        .collect();

    Ok(AllCounterTradesResponse {
        counter_trades: counter_trades?,
    })
}

// parse counter trades to human readable format
fn parse_counter_trades(
    _: &dyn Api,
    item: StdResult<Pair<TradeInfo>>,
    trade_id: Vec<u8>,
) -> StdResult<TradeResponse> {
    item.map(|(counter_id, trade)| {
        let trade_id = trade_id.try_into().unwrap();
        let counter_id = counter_id.try_into().unwrap();

        TradeResponse {
            trade_id: u64::from_be_bytes(trade_id),
            counter_id: Some(u64::from_be_bytes(counter_id)),
            trade_info: trade,
        }
    })
}

pub fn query_counter_trades(deps: Deps, trade_id: u64) -> StdResult<AllCounterTradesResponse> {
    let counter_trades: StdResult<Vec<TradeResponse>> = COUNTER_TRADE_INFO
        .prefix(trade_id.into())
        .range(deps.storage, None, None, Order::Descending)
        .map(|kv_item| parse_counter_trades(deps.api, kv_item, U64Key::new(trade_id).joined_key()))
        .collect();

    Ok(AllCounterTradesResponse {
        counter_trades: counter_trades?,
    })
}
