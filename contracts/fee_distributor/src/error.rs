use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Fee not paid correctly, please provide only one native asset at a time")]
    DepositNotCorrect {},

    #[error("Projects fee allocation cannot be higher than 100%")]
    AllocationTooHigh {},
}
