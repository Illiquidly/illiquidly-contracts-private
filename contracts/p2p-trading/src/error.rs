use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

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

    #[error("Key already exists in TradeInfo")]
    ExistsInTradeInfo {},

    #[error("Key does not exist in TradeInfo")]
    NotFoundInTradeInfo {},

    #[error("Trader not creator of the trade")]
    TraderNotCreator {},

    #[error("Key already exists in CounterTradeInfo")]
    ExistsInCounterTradeInfo {},

    #[error("Key does not exist in CounterTradeInfo")]
    NotFoundInCounterTradeInfo {},
    
    #[error("Trader not creator of the CounterTrade")]
    CounterTraderNotCreator {},

}
