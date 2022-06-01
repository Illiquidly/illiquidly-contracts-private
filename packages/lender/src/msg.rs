use cosmwasm_std::{StdError, StdResult};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Uint128};
use crate::state::{BorrowTerms, Cw721Info};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMsg {
    pub name: String,
    pub owner: Option<String>,
    pub oracle: Option<String>,
    pub vault_token: String
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

fn is_valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() < 3 || bytes.len() > 50 {
        return false;
    }
    true
}


#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // User accessible functions
    Borrow {
        asset_info: Cw721Info,
        wanted_terms: BorrowTerms,
        principle_slippage: Uint128
    },
    Repay {
        borrower: String, 
        loan_id: u64,
        assets: Uint128
    },
    // Admin specific
    SetOwner {
        owner: String
    },
    SetOracle {
        oracle: String
    },
    ToggleLock {
        lock: bool
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Returns the current state of the contract (locked)
    /// Return type: StateResponse.
    State {},
    /// Returns metadata on the contract - name, owner, oracle, etc.
    /// Return type: ContractInfoResponse.
    ContratInfo {},
}
