use cw_storage_plus::{Item, Map, U64Key};

use cosmwasm_std::{Addr, Env, Storage};

use crate::error::ContractError;
use nft_loans_export::state::{BorrowerInfo, CollateralInfo, ContractInfo, LoanState, OfferInfo};

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");

pub const COLLATERAL_INFO: Map<(&Addr, U64Key), CollateralInfo> = Map::new("collateral_info");

pub const BORROWER_INFO: Map<&Addr, BorrowerInfo> = Map::new("borrower_info");

pub fn is_owner(storage: &dyn Storage, sender: Addr) -> Result<ContractInfo, ContractError> {
    let contract_info = CONTRACT_INFO.load(storage)?;
    if sender == contract_info.owner {
        Ok(contract_info)
    } else {
        Err(ContractError::Unauthorized {})
    }
}

pub fn is_collateral_withdrawable(collateral: &CollateralInfo) -> Result<(), ContractError> {
    match collateral.state {
        LoanState::Published => Ok(()),
        _ => Err(ContractError::NotWithdrawable {}),
    }
}

pub fn is_loan_modifiable(collateral: &CollateralInfo) -> Result<(), ContractError> {
    match collateral.state {
        LoanState::Published => Ok(()),
        _ => Err(ContractError::NotWithdrawable {}),
    }
}

pub fn is_loan_acceptable(collateral: &CollateralInfo) -> Result<(), ContractError> {
    match collateral.state {
        LoanState::Published => Ok(()),
        _ => Err(ContractError::NotAcceptable {}),
    }
}

pub fn is_loan_counterable(collateral: &CollateralInfo) -> Result<(), ContractError> {
    match collateral.state {
        LoanState::Published => Ok(()),
        _ => Err(ContractError::NotCounterable {}),
    }
}

pub fn can_repay_loan(env: Env, collateral: &CollateralInfo) -> Result<(), ContractError> {
    if is_loan_defaulted(env, collateral).is_ok() {
        return Err(ContractError::WrongLoanState {
            state: LoanState::Defaulted {},
        });
    }

    match collateral.state {
        LoanState::Started => Ok(()),
        _ => Err(ContractError::WrongLoanState {
            state: collateral.state.clone(),
        }),
    }
}

pub fn is_loan_defaulted(env: Env, collateral: &CollateralInfo) -> Result<(), ContractError> {
    // If there is no offer, the loan can't be defaulted
    let offer = get_active_loan(collateral)?;
    match &collateral.state {
        LoanState::Started => {
            if collateral.start_block.unwrap() + offer.terms.duration_in_blocks < env.block.height
                && offer.terms.default_terms.is_none()
            {
                Ok(())
            } else {
                Err(ContractError::WrongLoanState {
                    state: LoanState::Started,
                })
            }
        }
        _ => Err(ContractError::WrongLoanState {
            state: collateral.state.clone(),
        }),
    }
}

pub fn get_loan(collateral: &CollateralInfo, offer_id: usize) -> Result<OfferInfo, ContractError> {
    if offer_id < collateral.offers.len() {
        Ok(collateral.offers[offer_id].clone()) 
    } else {
        Err(ContractError::OfferNotFound {})
    }
}

pub fn get_active_loan(collateral: &CollateralInfo) -> Result<OfferInfo, ContractError> {
    let offer_id = collateral
        .active_loan
        .ok_or( ContractError::OfferNotFound {})?;
    get_loan(collateral, offer_id as usize)
}

pub fn is_lender(
    lender: Addr,
    collateral: &CollateralInfo,
    offer_id: usize,
) -> Result<OfferInfo, ContractError> {
    let offer = get_loan(collateral, offer_id)?;
    if lender != offer.lender {
        return Err(ContractError::Unauthorized {});
    }
    Ok(offer)
}

pub fn is_active_lender(
    lender: Addr,
    collateral: &CollateralInfo,
) -> Result<OfferInfo, ContractError> {
    let offer = get_active_loan(collateral)?;
    if lender != offer.lender {
        return Err(ContractError::Unauthorized {});
    }
    Ok(offer)
}
