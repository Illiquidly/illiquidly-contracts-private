use cosmwasm_std::StdError;
use p2p_trading_export::state::TradeState;
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


    #[error("An unplanned bug just happened :/")]
    ContractBug{},

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

    #[error("Trade cannot be countered, it is not ready or is already cancelled/terminated")]
    NotCounterable {},

    #[error("Wrong state of the trade for the current operation")]
    WrongTradeState{state:TradeState},

    #[error("Can change the state of the trade")]
    CantChangeTradeState { from: TradeState, to: TradeState },

    #[error("Can change the state of the counter-trade")]
    CantChangeCounterTradeState { from: TradeState, to: TradeState },

    #[error("Sorry, you can't accept a counter trade that is not published yet")]
    CantAcceptNotPublishedCounter {},

    #[error("Sorry, you can't accept a counter trade that is not published yet")]
    TradeAlreadyAccepted {},

    #[error("Sorry, this trade is not accepted yet")]
    TradeNotAccepted {},

    #[error("Sorry, this trade is cancelled")]
    TradeCancelled {},
}
