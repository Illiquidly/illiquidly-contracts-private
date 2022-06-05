use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map, U64Key};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_4626::state::AssetInfo;
pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");
pub const STATE: Item<State> = Item::new("state");
pub const BORROWS: Map<(&Addr, U64Key), BorrowInfo> = Map::new("borrows");

// This allows the interest to pay to be stable (at the scale of a human) with the blocks mined
pub const MIN_BLOCK_OFFSET: u64 = 10u64;

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ContractInfo {
    pub name: String,
    pub vault_token: Addr,
    pub vault_asset: AssetInfo,
    pub owner: Addr,
    pub oracle: Addr,
    pub increasor_incentives: Uint128, // In 1/PERCENTAGE_RATE of the intersts surplus generated
    pub interests_fee_rate: Uint128,   // In 1/PERCENTAGE_RATE of the total interests
    pub fee_distributor: Addr,
}

/// Internal state of the contract
/// Can be changed at anytime (without time locks for now) by the owner of the contract
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct State {
    pub borrow_locked: bool,
}

/// Terms of borrowing for a contract
/// If duration is not specified, the loan can be liquidated using info from the oracle contract
/// The borrower has more risk so the terms should be favorable in that cas
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct BorrowTerms {
    pub principle: Uint128,
    pub interests: InterestType,
}

pub const PERCENTAGE_RATE: u128 = 10_000u128;
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub enum InterestType {
    Fixed {
        interests: Uint128,
        duration: u64,
    },
    Continuous {
        last_interest_rate: Uint128, // In 1/PERCENTAGE_RATE per block
        interests_accrued: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct InterestsInfo {
    pub safe_interest_rate: Uint128, // In 1/PERCENTAGE_RATE per block
    pub expensive_interest_rate: Uint128, // In 1/PERCENTAGE_RATE per block
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub enum BorrowMode {
    Fixed,
    Continuous,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub enum BorrowZone {
    SafeZone,
    ExpensiveZone,
    LiquidationZone,
}

/// Structure to hold the borrow informations
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct BorrowInfo {
    pub collateral: Option<Cw721Info>,
    pub principle: Uint128,
    pub interests: InterestType,
    pub start_block: u64,
    pub borrow_zone: BorrowZone,
    pub rate_increasor: Option<RateIncreasor>,
}

/// Information about the increasor (their address, and the rate of the safe zone at the time of triggering)
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct RateIncreasor {
    pub increasor: Addr,
    pub previous_rate: Uint128,
}

/// Structure to hold the borrow informations
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct Cw721Info {
    pub nft_address: String,
    pub token_id: String,
}
