use strum_macros;

use cosmwasm_std::{Addr, Coin, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use utils::state::{AssetInfo, Cw20Coin};
// We neep a map per user of all loans that are happening right now !
// The info should be redondant and linked

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct CollateralInfo {
    pub terms: Option<LoanTerms>,
    pub associated_asset: AssetInfo,
    pub state: LoanState,
    pub offers: Vec<OfferInfo>,
    pub active_loan: Option<u64>,
    pub start_block: Option<u64>,
}

impl Default for CollateralInfo {
    fn default() -> Self {
        Self {
            terms: None,
            associated_asset: AssetInfo::Cw20Coin(Cw20Coin {
                address: "".to_string(),
                amount: Uint128::zero(),
            }),
            state: LoanState::Published,
            offers: vec![],
            active_loan: None,
            start_block: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct BorrowerInfo {
    pub last_collateral_id: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct OfferInfo {
    pub lender: Addr,
    pub terms: LoanTerms,
    pub state: OfferState,
    pub deposited_funds: Option<Coin>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub struct LoanTerms {
    pub principle: Coin,
    pub interest: Uint128,
    pub duration_in_blocks: u64,
    pub default_terms: Option<DefaultTerms>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub struct DefaultTerms {
    pub late_payback_rate: Uint128, // (100%/block = 10_000_000)
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, strum_macros::Display)]
#[serde(rename_all = "snake_case")]
pub enum LoanState {
    Published,
    Started,
    Defaulted,
    Ended,
    AssetWithdrawn,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, strum_macros::Display)]
#[serde(rename_all = "snake_case")]
pub enum OfferState {
    Published,
    Accepted,
    Refused,
    Cancelled,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ContractInfo {
    pub name: String,
    pub owner: Addr,
    pub treasury: String,
    pub fee_rate: Uint128,
}
