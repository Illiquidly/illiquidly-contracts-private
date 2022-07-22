use cosmwasm_std::StdError;
use raffles_export::state::{AssetInfo, RaffleState};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("An unplanned bug just happened :/")]
    ContractBug {},

    #[error("This action is not allowed, the contract is locked")]
    ContractIsLocked {},

    #[error("Key already exists in RaffleInfo")]
    ExistsInRaffleInfo {},

    #[error("Raffle ID does not exist")]
    NotFoundInRaffleInfo {},

    #[error("You can't buy tickets on this raffle anymore")]
    CantBuyTickets {},

    #[error("A raffle can only be done with CW721 or CW1155 assets")]
    WrongAssetType {},

    #[error("Tickets to a raffle can only be bought with native assets or CW20 coins")]
    WrongFundsType {},

    #[error("The sent asset doesn't match the asset in the message sent along with it")]
    AssetMismatch {},

    #[error("The sent assets ({assets_received:?}) don't match the required assets ({assets_wanted:?}) for this raffle")]
    PaiementNotSufficient {
        assets_wanted: AssetInfo,
        assets_received: AssetInfo,
    },

    #[error("Too much tickets were already purchased for this raffle")]
    TooMuchTickets {},

    #[error("The provided randomness is invalid current round : {round:?}")]
    RandomnessNotAccepted { round: u64 },

    #[error("This raffle is not ready to accept new randomness. Only Closed raffles can be decided upon. Current status : {status:?}")]
    WrongStateForRandmness { status: RaffleState },

    #[error("This raffle is not ready to be claimed.  Current status : {status:?}")]
    WrongStateForClaim { status: RaffleState },

    #[error("The public key you indicated is invalid")]
    InvalidPubkey {},

    #[error("The randomness signatur is invalid")]
    InvalidSignature {},

    #[error("Wrong Format for the verify response")]
    ParseReplyError {},

    #[error("This parameter name was not found, you can't change it !")]
    ParameterNotFound {},
}
