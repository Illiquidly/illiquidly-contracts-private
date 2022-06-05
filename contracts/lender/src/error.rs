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

    #[error("You can't borrow more than the borrow limit on this asset. Asset: {collateral_address}, wanted: {wanted:?}, limit: {limit:?}")]
    TooMuchBorrowed {
        collateral_address: String,
        wanted: Uint128,
        limit: Uint128,
    },

    #[error("Only the borrower can repay a loan if it's not defaulted")]
    CannotLiquidateBeforeDefault {},

    #[error("The Loan is defaulted, you can't repay your own debt anymore...")]
    CannotRepayWhenDefaulted {},

    #[error("Fixed loans cannot be repaid partially. Expected assets : {expected:?}, Provided assets: {provided:?}")]
    CanOnlyRepayWholeFixedLoan {
        expected: Uint128,
        provided: Uint128,
    },

    #[error("Loans cannot be liquidated partially, this is not Anchor")]
    CanOnlyLiquidateWholeLoan {},

    #[error("You can't repay a loan whose collateral has already been withdrawn")]
    AssetAlreadyWithdrawn {},

    // Rate increase-decrease errors
    #[error("Only the original borrower can decrease their interest rate")]
    OnlyBorrowerCanLowerRate {},

    #[error("You can't have multiple rate increasors")]
    CantIncreaseRateMultipleTimes {},

    #[error("You can only change to the safe zone from the expensive zone")]
    OnlyFromExpensiveZone {},

    #[error("You need to repay the expensive zone before going forward")]
    NeedToRepayExpensiveZone {},

    #[error("You can only change to the expensive zone from the safe zone")]
    OnlyFromSafeZone {},

    #[error("A fixed interest/duration loan, doesn't have an intere rate")]
    FixedLoanNoInterestRate {},

    #[error("The format of your transfer message was wrong for the lender contract")]
    ReceiveMsgNotAccepted {},

    #[error("The assets you sent don't match the message you used")]
    AssetsSentDontMatch {},

    #[error("The value of the paramter you are trying to change is not acceptable")]
    ParamNotAccepted {},

    #[error("A repaiement must cover the increasor incentives")]
    MustAtLeastCoverIncreasor {},
}
