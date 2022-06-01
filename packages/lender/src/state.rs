use cosmwasm_std::{
     Addr, 
    Uint128,
};
use cw_storage_plus::{Item, Map,U64Key};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONTRACT_INFO: Item::<ContractInfo> = Item::new("contract_info");
pub const STATE: Item::<State> = Item::new("state");
pub const BORROWS: Map::<(&Addr, U64Key), BorrowInfo> = Map::new("borrows");

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ContractInfo {
    pub name: String,
    pub vault_token: Addr,
    pub owner: Addr,
    pub oracle: Addr
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
    pub interests: InterestType
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub enum InterestType{
    Fixed(FixedInterests),
    Continuous(ContinousInterests)
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct FixedInterests {
    pub interests: Uint128,
    pub duration: u64
}

pub const PERCENTAGE_RATE: u128 = 10_000u128;

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct ContinousInterests {
    pub interest_rate: Uint128 // In 1/PERCENTAGE_RATE per block
}


/// Structure to hold the borrow informations
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct BorrowInfo {
    pub asset: Option<Cw721Info>,
    pub terms: BorrowTerms,
    pub start_block: u64
}

/// Structure to hold the borrow informations
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct Cw721Info {
    pub nft_address: String,
    pub token_id: String 
}

