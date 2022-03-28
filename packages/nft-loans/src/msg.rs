use cosmwasm_std::{StdError, StdResult, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utils::msg::is_valid_name;

use crate::state::LoanTerms;

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMsg {
    pub name: String,
    pub owner: Option<String>,
    pub treasury: String,
    pub fee_rate: Uint128,
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
/// This contract nevers holds any funds
/// In case it does, it's that an error occured
/// TODO, we need to provide a way to make sure we can get those funds back
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    //// We support both Cw721 and Cw1155
    DepositCollateral {
        address: String,
        token_id: String,
        value: Option<Uint128>,
        terms: Option<LoanTerms>,
    },
    /// Used to withdraw the collateral before the loan starts
    WithdrawCollateral {
        loan_id: u64,
    },
    SetTerms {
        loan_id: u64,
        terms: LoanTerms,
    },
    MakeOffer {
        borrower: String,
        loan_id: u64,
        terms: LoanTerms,
    },
    CancelOffer {
        borrower: String,
        loan_id: u64,
        offer_id: u64,
    },
    RefuseOffer {
        loan_id: u64,
        offer_id: u64,
    },
    WithdrawRefusedOffer {
        borrower: String,
        loan_id: u64,
        offer_id: u64,
    },
    AcceptOffer {
        loan_id: u64,
        offer_id: u64,
    },
    AcceptLoan {
        borrower: String,
        loan_id: u64,
    },
    RepayBorrowedFunds {
        loan_id: u64,
    },
    ForceDefault {
        borrower: String,
        loan_id: u64,
    },
    /// Used only when the loan can be paid back late
    WithdrawDefaultedLoan {
        borrower: String,
        loan_id: u64,
    },
    /// Internal state
    SetOwner {
        owner: String,
    },
    SetTreasury {
        treasury: String,
    },
    SetFeeRate {
        fee_rate: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub struct QueryFilters {
    pub states: Option<Vec<String>>,
    pub owner: Option<String>,
    pub whitelisted_user: Option<String>,
    pub contains_token: Option<String>,
    pub wanted_nft: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    ContractInfo {},
    CollateralInfo { borrower: String, loan_id: u64 },
    BorrowerInfo { borrower: String },
}
