use strum_macros;

use cosmwasm_std::{Addr, Coin, Timestamp, Uint128, Binary};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MINIMUM_RAFFLE_DURATION: u64 = 3600; // A raffle last at least 1 hour
pub const MINIMUM_RAFFLE_TIMEOUT: u64 = 120; // The raffle duration is a least 2 minutes
pub const MINIMUM_RAND_FEE: u128 = 1; // The randomness provider gets at least 1/10_000 of the total raffle price
pub const MAXIMUM_PARTICIPANT_NUMBER: u64 = 1000;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Cw1155Coin {
    pub address: String,
    pub token_id: String,
    pub value: Uint128,
}

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
    Cw1155Coin(Cw1155Coin),
    Coin(Coin),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, strum_macros::Display)]
#[serde(rename_all = "snake_case")]
pub enum RaffleState {
    Created,
    Started,
    Closed,
    Finished
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ContractInfo {
    pub name: String,
    pub owner: Addr,
    pub fee_addr: Addr,
    pub last_raffle_id: Option<u64>,
    pub minimum_raffle_duration: u64,
    pub minimum_raffle_timeout: u64,
    pub raffle_fee: Uint128, // in 10_000
    pub rand_fee: Uint128 // in 10_000
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct RaffleTicket {
    pub raffle_id: u64,
    pub owner: Addr,
}


#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct RaffleInfo {
    pub owner: Addr,
    pub asset: AssetInfo,
    pub raffle_start_timestamp: Timestamp,
    pub raffle_duration: u64,
    pub raffle_timeout: u64,
    pub comment: Option<String>,
    pub raffle_ticket_price: AssetInfo,
    pub tickets: Vec<Addr>,
    pub current_randomness: Binary,
    pub randomness_round: u64,
    pub max_participant_number: u64
}

