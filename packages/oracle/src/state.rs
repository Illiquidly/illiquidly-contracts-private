use cosmwasm_std::{Addr, Timestamp, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ContractInfo {
    pub name: String,
    pub owner: Addr,
    pub timeout: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct NftPrice {
    pub price: Uint128,
    pub oracle_owner: Addr,
    pub last_update: Timestamp,
}
