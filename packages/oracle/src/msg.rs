use cosmwasm_std::{StdError, StdResult, Uint128};
use cw_4626::state::AssetInfo;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utils::msg::is_valid_name;

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMsg {
    pub name: String,
    pub owner: Option<String>,
    pub timeout: Option<u64>,
}

impl InstantiateMsg {
    pub fn validate(&self) -> StdResult<()> {
        // Check name, symbol, decimals
        if !is_valid_name(&self.name) {
            return Err(StdError::generic_err(
                "Name is not in the expected format (3-50 UTF-8 bytes)",
            ));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    SetNftPrice {
        contract: String,
        oracle_owner: Option<String>,
        price: Uint128,
        unit: AssetInfo,
    },
    SetOwner {
        owner: String,
    },
    SetTimeout {
        timeout: u64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    NftPrice { contract: String, unit: AssetInfo },
    ContractInfo {},
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct NftPriceResponse {
    pub contract: String,
    pub price: Uint128,
    pub unit: AssetInfo,
    pub oracle_owner: String,
    pub timeout: bool,
}
