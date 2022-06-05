use cosmwasm_std::{Binary, StdError, StdResult};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{BorrowMode, Cw721Info};
use cosmwasm_std::Uint128;

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMsg {
    pub name: String,
    pub owner: Option<String>,
    pub oracle: Option<String>,
    pub vault_token: String,
    pub increasor_incentives: Uint128,
    pub interests_fee_rate: Uint128,
    pub fee_distributor: String,
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
        assets_to_borrow: Uint128,
        borrow_mode: BorrowMode,
    },
    BorrowMore {
        loan_id: u64,
        assets_to_borrow: Uint128,
    },
    Repay {
        borrower: String,
        loan_id: u64,
        assets: Uint128,
    },
    Receive {
        sender: String,
        amount: Uint128,
        msg: Binary,
    },
    ModifyRate {
        borrower: String,
        loan_id: u64,
    },
    // Admin specific
    SetOwner {
        owner: String,
    },
    SetOracle {
        oracle: String,
    },
    ToggleLock {
        lock: bool,
    },
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
    /// Returns the borrow info of the designated loan
    BorrowInfo {
        owner: String,
        loan_id: u64,
    },
    BorrowZones {
        asset_info: Cw721Info,
    },
    BorrowTerms {
        asset_info: Cw721Info,
        borrow_mode: BorrowMode,
    },
}
