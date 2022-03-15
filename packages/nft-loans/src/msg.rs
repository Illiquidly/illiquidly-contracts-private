use cosmwasm_std::{Coin, StdError, StdResult, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utils::msg::is_valid_name;

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMsg {
    pub name: String,
    pub owner: Option<String>,
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
    /// Used to withdraw the collateral
    /// 1. Before the loan starts
    /// 2. During the loan (by sending funds with the transaction)
    /// 3. When the loan defaults
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
    AcceptOffer {
        loan_id: u64,
        offer_id: u64,
    },
    AcceptLoan {
        borrower: String,
        loan_id: u64,
    },
    WithdrawCancelledOffer {},
    WithdrawEndedLoan {},
    /// Internal state
    SetNewOwner {
        owner: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub struct LoanTerms {
    pub principle: Option<Coin>,
    pub rate: Option<String>,
    pub duration_in_block: Option<u64>,
    pub default_terms: Option<DefaultTerms>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub struct DefaultTerms {
    pub can_payback_late: bool,
    pub late_payback_rate: Option<String>,
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
pub enum QueryMsg {}
