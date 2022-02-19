use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::Cw721Coin;
use cosmwasm_std::{Addr, Coin};
use cw20::Cw20Coin;

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfo {
    Cw20Coin(Cw20Coin),
    Cw721Coin(Cw721Coin),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TradeState {
    Created,
    Published,
    Acknowledged,
    Countered,
    Refused,
    Accepted,
    Cancelled,
    Withdrawn,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CounterTradeState {
    Created,
    Countered,
    Refused,
    Suggested,
    Accepted,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ContractInfo {
    pub name: String,
    pub owner: String,
    pub last_trade_id: Option<u64>,
}


#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AcceptedTradeInfo {
    pub trade_id: u64,
    pub counter_id: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct TradeInfo {
    pub owner: Addr,
    pub associated_assets: Vec<AssetInfo>,
    pub associated_funds: Vec<Coin>,
    pub state: TradeState,
    pub last_counter_id: Option<u64>,
    pub comment: Option<String>,
    pub accepted_info: Option<AcceptedTradeInfo>
}