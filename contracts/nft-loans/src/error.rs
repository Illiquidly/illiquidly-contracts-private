use cosmwasm_std::StdError;
use p2p_trading_export::state::TradeState;
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

    #[error("Sorry, your asset is not withdrawable at this stage")]
    NotWithdrawable {},

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

    #[error("Wrong state of the trade for the current operation : {state:?}")]
    WrongTradeState { state: TradeState },

    #[error("Can change the state of the trade from {from:?} to {to:?}")]
    CantChangeTradeState { from: TradeState, to: TradeState },
}
