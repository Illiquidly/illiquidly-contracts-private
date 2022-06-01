use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("The contract doesn't accept borrowing. You can only repay your debts")]
    BorrowLocked {},

    #[error("The difference was too big between the wanted terms and the actual terms of the loan")]
    TooMuchSlippage{},

    #[error("Only the borrower can repay a loan if it's not defaulted")]
    CannotLiquidateBeforeDefault{},

    #[error("The Loan is defaulted, you can't repay your own debt anymore...")]
    CannotRepayWhenDefaulted{},

    #[error("Fixed loans cannot be repaid partially. Expected assets : {expected:?}, Provided assets: {provided:?}")]
    CanOnlyRepayWholeFixedLoan{expected: Uint128, provided: Uint128},

    #[error("Loans cannot be liquidated partially, this is not Anchor")]
    CanOnlyLiquidateWholeLoan{},

    #[error("You can't repay a loan whose collateral has already been withdrawn")]
    AssetAlreadyWithdrawn{},
}
