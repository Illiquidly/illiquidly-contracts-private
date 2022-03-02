use std::fmt::{Display, Formatter, Result};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr, Coin, Uint128};

use std::collections::HashSet;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Cw721Coin {
    pub address: String,
    pub token_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Cw20Coin {
    pub address: String,
    pub amount: Uint128,
}

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
    Countered,
    Refused,
    Accepted,
    Cancelled,
}

impl Display for TradeState {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{:?}", self)
    }
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
    pub whitelisted_users: HashSet<String>,
    pub comment: Option<String>,
    pub accepted_info: Option<AcceptedTradeInfo>,
    pub assets_withdrawn: bool,
}
