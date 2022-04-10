use cosmwasm_std::{Api, Pair};
#[cfg(not(feature = "library"))]
use cosmwasm_std::{Deps, Order, StdResult, Storage};

use cw_storage_plus::{Bound, PrimaryKey, U64Key};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;

use crate::state::{get_actual_counter_state, CONTRACT_INFO, COUNTER_TRADE_INFO, TRADE_INFO};
use p2p_trading_export::msg::QueryFilters;
use p2p_trading_export::state::{AssetInfo, ContractInfo, CounterTradeInfo, TradeInfo};

use itertools::Itertools;
// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;
const BASE_LIMIT: usize = 100;

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct TradeResponse {
    pub trade_id: u64,
    pub counter_id: Option<u64>,
    pub trade_info: Option<TradeInfo>,
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
            trade_info: Some(trade),
        }
    })
}

fn joined_to_key(ck: Vec<u8>) -> (u64, u64) {
    let (trade_id, counter_id) = (&ck[2..10], &ck[10..]);

    (
        u64::from_be_bytes(trade_id.try_into().unwrap()),
        u64::from_be_bytes(counter_id.try_into().unwrap()),
    )
}

// parse counter trades to human readable format
fn parse_all_counter_trades(
    _: &dyn Api,
    storage: &dyn Storage,
    item: StdResult<Pair<TradeInfo>>,
) -> StdResult<TradeResponse> {
    item.map(|(ck, mut counter)| {
        // First two bytes define size [0,8] since we know it's u64 skip it.
        let (trade_id, counter_id) = joined_to_key(ck);
        get_actual_counter_state(storage, trade_id, &mut counter)?;
        Ok(TradeResponse {
            trade_id,
            counter_id: Some(counter_id),
            trade_info: Some(counter),
        })
    })?
}

// parse counter trades to human readable format
fn parse_counter_trades(
    _: &dyn Api,
    storage: &dyn Storage,
    item: StdResult<Pair<TradeInfo>>,
    trade_id: u64,
) -> StdResult<TradeResponse> {
    item.map(|(counter_id, mut counter)| {
        let counter_id = counter_id.try_into().unwrap();
        get_actual_counter_state(storage, trade_id, &mut counter)?;
        Ok(TradeResponse {
            trade_id,
            counter_id: Some(u64::from_be_bytes(counter_id)),
            trade_info: Some(counter),
        })
    })?
}

pub fn trade_filter(
    api: &dyn Api,
    trade_info: &StdResult<TradeResponse>,
    filters: &Option<QueryFilters>,
) -> bool {
    if let Some(filters) = filters {
        let trade = trade_info.as_ref().unwrap();

        (match &filters.states {
            Some(state) => state.contains(&trade.trade_info.as_ref().unwrap().state.to_string()),
            None => true,
        } && match &filters.owner {
            Some(owner) => trade.trade_info.as_ref().unwrap().owner == owner.clone(),
            None => true,
        } && match &filters.has_whitelist {
            Some(has_whitelist) => {
                &trade
                    .trade_info
                    .as_ref()
                    .unwrap()
                    .whitelisted_users
                    .is_empty()
                    != has_whitelist
            }
            None => true,
        } && match &filters.whitelisted_user {
            Some(whitelisted_user) => trade
                .trade_info
                .as_ref()
                .unwrap()
                .whitelisted_users
                .contains(&api.addr_validate(whitelisted_user).unwrap()),
            None => true,
        } && match &filters.wanted_nft {
            Some(wanted_nft) => trade
                .trade_info
                .as_ref()
                .unwrap()
                .additionnal_info
                .nfts_wanted
                .contains(&api.addr_validate(wanted_nft).unwrap()),
            None => true,
        } && match &filters.contains_token {
            Some(token) => trade
                .trade_info
                .as_ref()
                .unwrap()
                .associated_assets
                .iter()
                .any(|asset| match asset {
                    AssetInfo::Coin(x) => x.denom == token.as_ref(),
                    AssetInfo::Cw20Coin(x) => x.address == token.as_ref(),
                    AssetInfo::Cw721Coin(x) => x.address == token.as_ref(),
                    AssetInfo::Cw1155Coin(x) => x.address == token.as_ref(),
                }),
            None => true,
        } && match &filters.assets_withdrawn {
            Some(assets_withdrawn) => {
                trade.trade_info.clone().unwrap().assets_withdrawn == *assets_withdrawn
            }
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
    if let Some(f) = filters.clone() {
        if let Some(counterer) = f.counterer {
            query_all_trades_by_counterer(deps, start_after, limit, counterer, filters)
        } else {
            query_all_trades_raw(deps, start_after, limit, filters)
        }
    } else {
        query_all_trades_raw(deps, start_after, limit, filters)
    }
}

pub fn query_all_trades_raw(
    deps: Deps,
    start_after: Option<u64>,
    limit: Option<u32>,
    filters: Option<QueryFilters>,
) -> StdResult<AllTradesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(|s| Bound::Exclusive(U64Key::new(s).joined_key()));

    let mut trades: Vec<TradeResponse> = TRADE_INFO
        .range(deps.storage, None, start.clone(), Order::Descending)
        .take(BASE_LIMIT)
        .map(|kv_item| parse_trades(deps.api, kv_item))
        .filter(|response| trade_filter(deps.api, response, &filters))
        .take(limit)
        .collect::<StdResult<Vec<TradeResponse>>>()?;

    if trades.is_empty() {
        let trade_id = TRADE_INFO
            .keys(deps.storage, None, start, Order::Descending)
            .take(BASE_LIMIT)
            .last();

        if let Some(trade_id) = trade_id {
            let trade_id = u64::from_be_bytes(trade_id.try_into().unwrap());
            if trade_id != 0 {
                trades = vec![TradeResponse {
                    trade_id,
                    counter_id: None,
                    trade_info: None,
                }]
            }
        }
    }
    Ok(AllTradesResponse { trades })
}

pub fn query_all_trades_by_counterer(
    deps: Deps,
    start_after: Option<u64>,
    limit: Option<u32>,
    counterer: String,
    filters: Option<QueryFilters>,
) -> StdResult<AllTradesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let start =
        start_after.map(|s| Bound::Exclusive((U64Key::new(s), U64Key::new(0)).joined_key()));

    let counter_filters = Some(QueryFilters {
        owner: Some(counterer),
        ..QueryFilters::default()
    });

    let mut trades: Vec<TradeResponse> = COUNTER_TRADE_INFO
        .range(deps.storage, None, start.clone(), Order::Descending)
        .take(BASE_LIMIT)
        .map(|kv_item| parse_all_counter_trades(deps.api, deps.storage, kv_item))
        .filter(|response| trade_filter(deps.api, response, &counter_filters))
        .filter_map(|response| response.ok())
        // Now we get back the trade_id and query the trade_info
        .map(|response| response.trade_id)
        .unique()
        .map(|trade_id| {
            Ok((
                U64Key::new(trade_id).joined_key(),
                TRADE_INFO.load(deps.storage, trade_id.into())?,
            ))
        })
        .map(|kv_item| parse_trades(deps.api, kv_item))
        .filter(|response| trade_filter(deps.api, response, &filters))
        .take(limit)
        .collect::<StdResult<Vec<TradeResponse>>>()?;

    if trades.is_empty() {
        let trade_info: Option<TradeResponse> = COUNTER_TRADE_INFO
            .range(deps.storage, None, start, Order::Descending)
            .take(BASE_LIMIT)
            .map(|kv_item| parse_all_counter_trades(deps.api, deps.storage, kv_item))
            .filter_map(|response| response.ok())
            .map(|response| response.trade_id)
            .unique()
            .map(|trade_id| {
                Ok((
                    U64Key::new(trade_id).joined_key(),
                    TRADE_INFO.load(deps.storage, trade_id.into())?,
                ))
            })
            .filter_map(|kv_item| parse_trades(deps.api, kv_item).ok())
            .last();

        if let Some(trade_info) = trade_info {
            if trade_info.trade_id != 0 && trade_info.counter_id.unwrap() != 0 {
                trades = vec![TradeResponse {
                    trade_id: trade_info.trade_id,
                    counter_id: trade_info.counter_id,
                    trade_info: None,
                }]
            }
        }
    }

    Ok(AllTradesResponse { trades })
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

    let mut counter_trades: Vec<TradeResponse> = COUNTER_TRADE_INFO
        .range(deps.storage, None, start.clone(), Order::Descending)
        .take(BASE_LIMIT)
        .map(|kv_item| parse_all_counter_trades(deps.api, deps.storage, kv_item))
        .filter(|response| trade_filter(deps.api, response, &filters))
        .take(limit)
        .collect::<StdResult<Vec<TradeResponse>>>()?;

    if counter_trades.is_empty() {
        let id = COUNTER_TRADE_INFO
            .keys(deps.storage, None, start, Order::Descending)
            .take(BASE_LIMIT)
            .last();

        if let Some(id) = id {
            let (trade_id, counter_id) = joined_to_key(id);
            if trade_id != 0 && counter_id != 0 {
                counter_trades = vec![TradeResponse {
                    trade_id,
                    counter_id: Some(counter_id),
                    trade_info: None,
                }]
            }
        }
    }

    Ok(AllCounterTradesResponse { counter_trades })
}

pub fn query_counter_trades(
    deps: Deps,
    trade_id: u64,
    start_after: Option<u64>,
    limit: Option<u32>,
    filters: Option<QueryFilters>,
) -> StdResult<AllCounterTradesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let start = start_after.map(|s| Bound::Exclusive(U64Key::new(s).joined_key()));

    let mut counter_trades: Vec<TradeResponse> = COUNTER_TRADE_INFO
        .prefix(trade_id.into())
        .range(deps.storage, None, start.clone(), Order::Descending)
        .take(BASE_LIMIT)
        .map(|kv_item| parse_counter_trades(deps.api, deps.storage, kv_item, trade_id))
        .filter(|response| trade_filter(deps.api, response, &filters))
        .take(limit)
        .collect::<StdResult<Vec<TradeResponse>>>()?;

    if counter_trades.is_empty() {
        let counter_id = COUNTER_TRADE_INFO
            .prefix(trade_id.into())
            .keys(deps.storage, None, start, Order::Descending)
            .take(BASE_LIMIT)
            .last();

        if let Some(counter_id) = counter_id {
            let counter_id = u64::from_be_bytes(counter_id.try_into().unwrap());
            if trade_id != 0 && counter_id != 0 {
                counter_trades = vec![TradeResponse {
                    trade_id,
                    counter_id: Some(counter_id),
                    trade_info: None,
                }]
            }
        }
    }

    Ok(AllCounterTradesResponse { counter_trades })
}
