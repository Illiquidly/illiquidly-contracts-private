use crate::state::{BorrowerInfo, CollateralInfo, ContractInfo, LoanTerms, OfferInfo};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{StdError, StdResult, Decimal};
use utils::msg::is_valid_name;
use utils::state::AssetInfo;

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct InstantiateMsg {
    pub name: String,
    pub owner: Option<String>,
    pub fee_distributor: String,
    pub fee_rate: Decimal,
}

impl InstantiateMsg {
    pub fn validate(&self) -> StdResult<()> {
        // Check name
        if !is_valid_name(&self.name) {
            return Err(StdError::generic_err(
                "Name is not in the expected format (3-50 UTF-8 bytes)",
            ));
        }
        // Check the fee distribution
        if self.fee_rate >= Decimal::one(){
            return Err(StdError::generic_err(
                "The Fee rate should be lower than 1"
            ))
        }


        Ok(())
    }
}
/// This contract nevers holds any funds
/// In case it does, it's that an error occured
#[cw_serde]
pub enum ExecuteMsg {
    //// We support both Cw721 and Cw1155
    DepositCollaterals {
        tokens: Vec<AssetInfo>,
        terms: Option<LoanTerms>,
        comment: Option<String>,
    },
    /// Used to modify the loan terms and the associated comment
    ModifyCollaterals {
        loan_id: u64,
        terms: Option<LoanTerms>,
        comment: Option<String>,
    },
    /// Used to withdraw the collateral before the loan starts
    WithdrawCollaterals {
        loan_id: u64,
    },

    /// Make an offer to deposited collaterals
    MakeOffer {
        borrower: String,
        loan_id: u64,
        terms: LoanTerms,
        comment: Option<String>,
    },
    CancelOffer {
        global_offer_id: String,
    },
    RefuseOffer {
        global_offer_id: String,
    },
    WithdrawRefusedOffer {
        global_offer_id: String,
    },
    AcceptOffer {
        global_offer_id: String,
    },
    AcceptLoan {
        borrower: String,
        loan_id: u64,
        comment: Option<String>,
    },
    RepayBorrowedFunds {
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
    ClaimOwnership { },
    SetFeeDistributor {
        fee_depositor: String,
    },
    SetFeeRate {
        fee_rate: Decimal,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ContractInfo)]
    ContractInfo {},
    #[returns(BorrowerInfo)]
    BorrowerInfo { borrower: String },

    #[returns(CollateralResponse)]
    CollateralInfo { borrower: String, loan_id: u64 },

    #[returns(MultipleCollateralsResponse)]
    Collaterals {
        borrower: String,
        start_after: Option<u64>,
        limit: Option<u32>,
    },

    #[returns(MultipleCollateralsAllResponse)]
    AllCollaterals {
        start_after: Option<(String, u64)>,
        limit: Option<u32>,
    },

    #[returns(OfferResponse)]
    OfferInfo { global_offer_id: String },

    #[returns(MultipleOffersResponse)]
    Offers {
        borrower: String,
        loan_id: u64,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(MultipleOffersResponse)]
    LenderOffers {
        lender: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
}

#[cw_serde]
pub struct CollateralResponse {
    pub borrower: String,
    pub loan_id: u64,
    pub collateral: CollateralInfo,
}

#[cw_serde]
pub struct MultipleCollateralsResponse {
    pub collaterals: Vec<CollateralResponse>,
    pub next_collateral: Option<u64>,
}

#[cw_serde]
pub struct MultipleCollateralsAllResponse {
    pub collaterals: Vec<CollateralResponse>,
    pub next_collateral: Option<(String, u64)>,
}

#[cw_serde]
pub struct OfferResponse {
    pub global_offer_id: String,
    pub offer_info: OfferInfo,
}

#[cw_serde]
pub struct MultipleOffersResponse {
    pub offers: Vec<OfferResponse>,
    pub next_offer: Option<String>,
}
