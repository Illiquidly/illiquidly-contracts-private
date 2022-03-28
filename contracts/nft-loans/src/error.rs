use cosmwasm_std::StdError;
use nft_loans_export::state::{LoanState, OfferState};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Please be careful what you send, some sthings are not possible")]
    MalformedMessage {},

    #[error("An unplanned bug just happened :/")]
    ContractBug {},

    #[error("You need to send exactly one coin with this transaction")]
    MultipleCoins {},

    #[error("Fund sent do not match the loan terms")]
    FundsDontMatchTerms {},

    #[error("Sorry, your asset is not withdrawable at this stage")]
    NotWithdrawable {},

    #[error("Sorry, no assets to withdraw here")]
    NoFundsToWithdraw {},

    #[error("Sorry, you can't accept this loan")]
    NotAcceptable {},

    #[error("Sorry, you can't make an offer on this trade")]
    NotCounterable {},

    #[error("This loan doesn't have any terms")]
    NoTermsSpecified {},

    #[error("Sorry, this loan doesn't exist :/")]
    LoanNotFound {},

    #[error("Sorry, this offer doesn't exist :/")]
    OfferNotFound {},

    #[error("Wrong state of the loan for the current operation : {state:?}")]
    WrongLoanState { state: LoanState },

    #[error("Wrong state of the offer for the current operation : {state:?}")]
    WrongOfferState { state: OfferState },

    #[error("Can change the state of the offer from {from:?} to {to:?}")]
    CantChangeOfferState { from: OfferState, to: OfferState },
}
