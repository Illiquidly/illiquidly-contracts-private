use strum_macros;

use cosmwasm_std::{Addr, Coin, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, strum_macros::Display)]
#[serde(rename_all = "snake_case")]
pub enum TradeState {
    Created,
    Published,
    Countered,
    Refused,
    Accepted,
    Cancelled,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ContractInfo {
    pub name: String,
    pub owner: Addr,
    pub fee_contract: Option<Addr>,
    pub last_trade_id: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct CounterTradeInfo {
    pub trade_id: u64,
    pub counter_id: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AdditionnalTradeInfo {
    pub comment: Option<String>,
    pub nfts_wanted: HashSet<Addr>,
}

impl Default for AdditionnalTradeInfo{
    fn default() -> Self{
        Self{
            comment: None,
            nfts_wanted: HashSet::new()
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct TradeInfo {
    pub owner: Addr,
    pub associated_assets: Vec<AssetInfo>,
    pub associated_funds: Vec<Coin>,
    pub state: TradeState,
    pub last_counter_id: Option<u64>,
    pub whitelisted_users: HashSet<Addr>,
    pub additionnal_info : AdditionnalTradeInfo,
    pub accepted_info: Option<CounterTradeInfo>,
    pub assets_withdrawn: bool,
}

impl Default for TradeInfo{
    fn default() -> Self{
        Self{
            owner: Addr::unchecked(""),
            associated_assets: vec![],
            associated_funds: vec![],
            state: TradeState::Created,
            last_counter_id: None,
            whitelisted_users: HashSet::new(),
            additionnal_info: AdditionnalTradeInfo::default(),
            accepted_info: None,
            assets_withdrawn: false,
        }
    }
}
