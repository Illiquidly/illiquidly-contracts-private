use strum_macros;

use cosmwasm_std::{coin, Addr, Binary, Coin, Timestamp, Uint128, Env};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/*
pub const MINIMUM_RAFFLE_DURATION: u64 = 3600; // A raffle last at least 1 hour
pub const MINIMUM_RAFFLE_TIMEOUT: u64 = 120; // The raffle duration is a least 2 minutes
pub const MINIMUM_RAND_FEE: u128 = 1; // The randomness provider gets at least 1/10_000 of the total raffle price
pub const MAXIMUM_PARTICIPANT_NUMBER: u64 = 1000;
*/

pub const MINIMUM_RAFFLE_DURATION: u64 = 1;
pub const MINIMUM_RAFFLE_TIMEOUT: u64 = 120; // The raffle duration is a least 2 minutes
pub const MINIMUM_RAND_FEE: u128 = 1; // The randomness provider gets at least 1/10_000 of the total raffle price

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

impl AssetInfo {
    pub fn coin(amount: u128, denom: &str) -> Self {
        AssetInfo::Coin(coin(amount, denom))
    }

    pub fn coin_raw(amount: Uint128, denom: &str) -> Self {
        AssetInfo::Coin(Coin {
            denom: denom.to_string(),
            amount,
        })
    }

    pub fn cw20(amount: u128, address: &str) -> Self {
        AssetInfo::cw20_raw(Uint128::from(amount), address)
    }

    pub fn cw20_raw(amount: Uint128, address: &str) -> Self {
        AssetInfo::Cw20Coin(Cw20Coin {
            address: address.to_string(),
            amount,
        })
    }

    pub fn cw721(address: &str, token_id: &str) -> Self {
        AssetInfo::Cw721Coin(Cw721Coin {
            address: address.to_string(),
            token_id: token_id.to_string(),
        })
    }

    pub fn cw1155(address: &str, token_id: &str, value: u128) -> Self {
        AssetInfo::cw1155_raw(address, token_id, Uint128::from(value))
    }

    pub fn cw1155_raw(address: &str, token_id: &str, value: Uint128) -> Self {
        AssetInfo::Cw1155Coin(Cw1155Coin {
            address: address.to_string(),
            token_id: token_id.to_string(),
            value,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, strum_macros::Display)]
#[serde(rename_all = "snake_case")]
pub enum RaffleState {
    Created,
    Started,
    Closed,
    Finished,
    Claimed,
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
    pub rand_fee: Uint128,   // in 10_000
    pub lock: bool,
    pub drand_url: String,
    pub verify_signature_contract: Addr,
    pub random_pubkey: Binary,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct RaffleOptions{
    pub raffle_start_timestamp: Timestamp, // If not specified, starts immediately
    pub raffle_duration: u64,
    pub raffle_timeout: u64,
    pub comment: Option<String>,
    pub max_participant_number: Option<u32>,
    pub max_ticket_per_address: Option<u32>
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct RaffleOptionsMsg{
    pub raffle_start_timestamp: Option<Timestamp>, // If not specified, starts immediately
    pub raffle_duration: Option<u64>,
    pub raffle_timeout: Option<u64>,
    pub comment: Option<String>,
    pub max_participant_number: Option<u32>,
    pub max_ticket_per_address: Option<u32>
}

impl RaffleOptions{
    pub fn new(env: Env, raffle_options: RaffleOptionsMsg, contract_info: ContractInfo) -> Self{
        Self{
             raffle_start_timestamp: raffle_options.raffle_start_timestamp
                .unwrap_or(env.block.time),
            raffle_duration: raffle_options.raffle_duration
                .unwrap_or(contract_info.minimum_raffle_duration)
                .max(contract_info.minimum_raffle_duration),
            raffle_timeout: raffle_options.raffle_timeout
                .unwrap_or(contract_info.minimum_raffle_timeout)
                .max(contract_info.minimum_raffle_timeout),
            comment: raffle_options.comment,
            max_participant_number: raffle_options.max_participant_number,
            max_ticket_per_address: raffle_options.max_ticket_per_address,
        }
    }
}  



#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct Randomness{
    pub randomness: [u8; 32],
    pub randomness_round: u64,
    pub randomness_owner: Addr
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct RaffleInfo {
    pub owner: Addr,
    pub asset: AssetInfo,
    pub raffle_ticket_price: AssetInfo,
    pub accumulated_ticket_fee: AssetInfo,
    pub number_of_tickets: u32,
    pub randomness: Option<Randomness>,
    pub winner: Option<Addr>,
    pub raffle_options: RaffleOptions
}
