use strum_macros;

use cosmwasm_std::{Addr, Coin, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::LoanTerms;
use utils::state::{AssetInfo, Cw20Coin};
// We neep a map per user of all loans that are happening right now !
// The info should be redondant and linked

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct CollateralInfo {
    pub borrower: Addr,
    pub terms: Option<LoanTerms>,
    pub associated_asset: AssetInfo,
    pub state: LoanState,
    pub offers: Vec<LoanInfo>,
    pub active_loan: Option<u32>,
    pub start_block: Option<u64>,
}

impl Default for CollateralInfo {
    fn default() -> Self {
        Self {
            borrower: Addr::unchecked(""),
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
pub struct LoanInfo {
    pub lender: Addr,
    pub terms: LoanTerms,
    pub deposited_funds: Option<Coin>,
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

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ContractInfo {
    pub name: String,
    pub owner: Addr,
    pub fee_contract: Option<Addr>,
}

/*
impl Default for TradeInfo {
    fn default() -> Self {
        Self {
            owner: Addr::unchecked(""),
            associated_assets: vec![],
            associated_funds: vec![],
            state: TradeState::Created,
            last_counter_id: None,
            whitelisted_users: HashSet::new(),
            additionnal_info: AdditionnalTradeInfo::default(),
            accepted_info: None,
            assets_withdrawn: false,
        }
    }
}
*/
