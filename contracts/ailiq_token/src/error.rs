use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid Execute Message: lender contract")]
    InvalidMessage {},

    #[error("Cannot set to own account")]
    CannotSetOwnAccount {},

    #[error("Invalid zero amount")]
    InvalidZeroAmount {},

    #[error("Allowance is expired")]
    Expired {},

    #[error("No allowance for this account")]
    NoAllowance {},

    #[error("Minting cannot exceed the cap")]
    CannotExceedCap {},

    #[error("Logo binary data exceeds 5KB limit")]
    LogoTooBig {},

    #[error("Invalid xml preamble for SVG")]
    InvalidXmlPreamble {},

    #[error("Invalid png header")]
    InvalidPngHeader {},

    #[error("Wrong asset deposited into the vault: sent {sent:?}, expected: {expected:?}")]
    WrongAssetDeposited { sent: String, expected: String },

    #[error("Not enough assets deposited into the vault: sent {sent:?}, expected: {expected:?}")]
    InsufficientAssetDeposited { sent: Uint128, expected: Uint128 },

    #[error("You can't deposit 0 assets")]
    ZeroDeposit {},

    #[error("You can't burn assets right now, sorry (check your balance is high enough and you have the right to burn those tokens")]
    UnableToBurn {},
}
