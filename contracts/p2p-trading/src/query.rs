use cosmwasm_std::{Api, Pair};
#[cfg(not(feature = "library"))]
use cosmwasm_std::{Coin, Deps, Order, StdResult};
use cw_storage_plus::Bound;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{CONTRACT_INFO, COUNTER_TRADE_INFO, TRADE_INFO};
use p2p_trading_export::state::{AcceptedTradeInfo, AssetInfo, ContractInfo, TradeInfo};

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct TradeResponse {
    pub trade_id: String,
    pub owner: String,
    pub associated_assets: Vec<AssetInfo>,
    pub associated_funds: Vec<Coin>,
    pub state: String,
    pub last_counter_id: Option<u64>,
    pub comment: Option<String>,
    pub accepted_info: Option<AcceptedTradeInfo>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct CounterTradeResponse {
    pub composite_id: String,
    pub trade_id: String,
    pub counter_id: String,
    pub owner: String,
    pub associated_assets: Vec<AssetInfo>,
    pub associated_funds: Vec<Coin>,
    pub state: String,
    pub last_counter_id: Option<u64>,
    pub comment: Option<String>,
    pub accepted_info: Option<AcceptedTradeInfo>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct AllTradesResponse {
    pub trades: Vec<TradeResponse>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct AllCounterTradesResponse {
    pub counter_trades: Vec<CounterTradeResponse>,
}

pub fn query_contract_info(deps: Deps) -> StdResult<ContractInfo> {
    CONTRACT_INFO.load(deps.storage)
}

// parse trades to human readable format
fn parse_trades(_: &dyn Api, item: StdResult<Pair<TradeInfo>>) -> StdResult<TradeResponse> {
    item.map(|(k, trade)| {
        let mut trade_id: [u8; 8] = [Default::default(); 8];
        trade_id[..k.len()].copy_from_slice(&k);

        TradeResponse {
            trade_id: u64::from_be_bytes(trade_id).to_string(),
            owner: trade.owner.to_string(),
            associated_assets: trade.associated_assets,
            state: trade.state.to_string(),
            associated_funds: trade.associated_funds,
            last_counter_id: trade.last_counter_id,
            comment: trade.comment,
            accepted_info: trade.accepted_info,
        }
    })
}
pub fn query_all_trades(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
    states: Option<Vec<String>>,
    owner: Option<String>,
) -> StdResult<AllTradesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start =
        start_after.map(|s| Bound::Exclusive(s.parse::<u64>().unwrap().to_be_bytes().to_vec()));

    let trades: StdResult<Vec<TradeResponse>> = TRADE_INFO
        .range(deps.storage, start, None, Order::Ascending)
        .map(|kv_item| parse_trades(deps.api, kv_item))
        .filter(|response| {
            let trade = response.as_ref().unwrap();

            // Owner not provided check if states are, and query only by states.
            if owner.is_none() {
                return match &states {
                    Some(state) => state.contains(&trade.state),
                    None => true, // No states defined return all items
                };
            }

            // Owner defined here, allow owner user to query his own trades and by state of his trades
            match &states {
                Some(state) => {
                    state.contains(&trade.state) && trade.owner == owner.clone().unwrap_or_default()
                }
                None => trade.owner == owner.clone().unwrap_or_default(), // Only query owner in case when owner is defined and states are not
            }
        })
        .take(limit)
        .collect();

    Ok(AllTradesResponse { trades: trades? })
}

// parse counter trades to human readable format
fn parse_all_counter_trades(
    _: &dyn Api,
    item: StdResult<Pair<TradeInfo>>,
) -> StdResult<CounterTradeResponse> {
    item.map(|(ck, trade)| {
        // First two bytes define size [0,8] since we know it's u64 skip it.
        let (trade_k, counter_k) = (&ck[2..10], &ck[10..]);

        // Used for pagination
        let mut composite_id: [u8; 16] = [Default::default(); 16];
        composite_id[..ck[2..].len()].copy_from_slice(&ck[2..]);

        let mut trade_id: [u8; 8] = [Default::default(); 8];
        trade_id[..trade_k.len()].copy_from_slice(trade_k);

        let mut counter_id: [u8; 8] = [Default::default(); 8];
        counter_id[..counter_k.len()].copy_from_slice(counter_k);

        CounterTradeResponse {
            composite_id: u128::from_be_bytes(composite_id).to_string(),
            trade_id: u64::from_be_bytes(trade_id).to_string(),
            counter_id: u64::from_be_bytes(counter_id).to_string(),
            owner: trade.owner.to_string(),
            associated_assets: trade.associated_assets,
            state: trade.state.to_string(),
            associated_funds: trade.associated_funds,
            last_counter_id: trade.last_counter_id,
            comment: trade.comment,
            accepted_info: trade.accepted_info,
        }
    })
}

pub fn query_all_counter_trades(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
    states: Option<Vec<String>>,
    owner: Option<String>,
) -> StdResult<AllCounterTradesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let start = start_after.map(|s| {
        Bound::Exclusive(
            [
                vec![0, 8], // size
                s.parse::<u128>().unwrap().to_be_bytes().to_vec(),
            ]
            .concat(),
        )
    });

    let counter_trades: StdResult<Vec<CounterTradeResponse>> = COUNTER_TRADE_INFO
        .range(deps.storage, start, None, Order::Ascending)
        .map(|kv_item| parse_all_counter_trades(deps.api, kv_item))
        .filter(|response| {
            let trade = response.as_ref().unwrap();

            // Owner not provided check if states are, and query only by states.
            if owner.is_none() {
                return match &states {
                    Some(state) => state.contains(&trade.state),
                    None => true, // No states defined return all items
                };
            }

            // Owner defined here, allow owner user to query his own trades and by state of his trades
            match &states {
                Some(state) => {
                    state.contains(&trade.state) && trade.owner == owner.clone().unwrap_or_default()
                }
                None => trade.owner == owner.clone().unwrap_or_default(), // Only query owner in case when owner is defined and states are not
            }
        })
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
    trade_k: Vec<u8>,
) -> StdResult<CounterTradeResponse> {
    item.map(|(k, trade)| {
        // compsited_id is used for query pagination
        let mut composite_id: [u8; 16] = [Default::default(); 16];
        composite_id[..[trade_k.clone(), k.clone()].concat().len()]
            .copy_from_slice(&[trade_k.clone(), k.clone()].concat());


        let mut trade_id: [u8; 8] = [Default::default(); 8];
        trade_id[..trade_k.len()].copy_from_slice(&trade_k);

        let mut counter_id: [u8; 8] = [Default::default(); 8];
        counter_id[..k.len()].copy_from_slice(&k);

        CounterTradeResponse {
            composite_id: u128::from_be_bytes(composite_id).to_string(),
            trade_id: u64::from_be_bytes(trade_id).to_string(),
            counter_id: u64::from_be_bytes(counter_id).to_string(),
            owner: trade.owner.to_string(),
            associated_assets: trade.associated_assets,
            state: trade.state.to_string(),
            associated_funds: trade.associated_funds,
            last_counter_id: trade.last_counter_id,
            comment: trade.comment,
            accepted_info: trade.accepted_info,
        }
    })
}

pub fn query_counter_trades(deps: Deps, trade_id: u64) -> StdResult<AllCounterTradesResponse> {
    let counter_trades: StdResult<Vec<CounterTradeResponse>> = COUNTER_TRADE_INFO
        .prefix(&trade_id.to_be_bytes())
        .range(deps.storage, None, None, Order::Ascending)
        .map(|kv_item| parse_counter_trades(deps.api, kv_item, (&trade_id.to_be_bytes()).to_vec()))
        .collect();

    Ok(AllCounterTradesResponse {
        counter_trades: counter_trades?,
    })
}
