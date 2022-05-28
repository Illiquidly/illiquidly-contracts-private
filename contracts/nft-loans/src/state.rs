use cw_storage_plus::{Item, Map, U64Key};

use cosmwasm_std::{Addr, Env, Storage};

use crate::error::ContractError;
use nft_loans_export::state::{
    BorrowerInfo, CollateralInfo, ContractInfo, LoanState, OfferInfo, OfferState,
};

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");

pub const COLLATERAL_INFO: Map<(&Addr, U64Key), CollateralInfo> = Map::new("collateral_info");

pub const BORROWER_INFO: Map<&Addr, BorrowerInfo> = Map::new("borrower_info");

pub const LENDER_OFFERS: Map<&Addr, Vec<(Addr, u64, u64)>> = Map::new("lender_offers");

pub fn is_owner(storage: &dyn Storage, sender: Addr) -> Result<ContractInfo, ContractError> {
    let contract_info = CONTRACT_INFO.load(storage)?;
    if sender == contract_info.owner {
        Ok(contract_info)
    } else {
        Err(ContractError::Unauthorized {})
    }
}

pub fn add_new_offer(
    storage: &mut dyn Storage,
    collateral: &mut CollateralInfo,
    collateral_key: (Addr, u64),
    offer: OfferInfo,
) -> Result<u64, ContractError> {
    // We add the new offer to the collateral object
    collateral.offers.push(offer.clone());
    COLLATERAL_INFO.save(
        storage,
        (&collateral_key.0, collateral_key.1.into()),
        collateral,
    )?;
    let offer_id = (collateral.offers.len() - 1) as u64;
    // We add the new offer to the lender object
    LENDER_OFFERS.update::<_, ContractError>(storage, &offer.lender, |x| match x {
        Some(mut offers) => {
            offers.push((collateral_key.0, collateral_key.1, offer_id));
            Ok(offers)
        }
        None => Ok(vec![(collateral_key.0, collateral_key.1, offer_id)]),
    })?;
    Ok(offer_id)
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
        _ => Err(ContractError::NotModifiable {}),
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
        Err(ContractError::WrongLoanState {
            state: LoanState::Defaulted {},
        })
    } else if collateral.state != LoanState::Started {
        Err(ContractError::WrongLoanState {
            state: collateral.state.clone(),
        })
    } else {
        Ok(())
    }
}

pub fn is_loan_defaulted(env: Env, collateral: &CollateralInfo) -> Result<(), ContractError> {
    // If there is no offer, the loan can't be defaulted
    let offer = get_active_loan(collateral)?;
    match &collateral.state {
        LoanState::Started => {
            if collateral.start_block.unwrap() + offer.terms.duration_in_blocks < env.block.height {
                Ok(())
            } else {
                Err(ContractError::WrongLoanState {
                    state: LoanState::Started,
                })
            }
        }
        LoanState::Defaulted => Ok(()),
        _ => Err(ContractError::WrongLoanState {
            state: collateral.state.clone(),
        }),
    }
}

pub fn get_offer(collateral: &CollateralInfo, offer_id: usize) -> Result<OfferInfo, ContractError> {
    if offer_id < collateral.offers.len() {
        let mut offer = collateral.offers[offer_id].clone();
        // We check the status of the offer.
        // A refused offer isn't marked as such but depends o=n the overlying collateral info state
        offer.state = match &offer.state {
            OfferState::Published => {
                if collateral.state != LoanState::Published {
                    OfferState::Refused
                } else {
                    OfferState::Published
                }
            }
            _ => offer.state,
        };
        Ok(offer)
    } else {
        Err(ContractError::OfferNotFound {})
    }
}

pub fn get_active_loan(collateral: &CollateralInfo) -> Result<OfferInfo, ContractError> {
    let offer_id = collateral
        .active_loan
        .ok_or(ContractError::OfferNotFound {})?;
    get_offer(collateral, offer_id as usize)
}

pub fn is_lender(
    lender: Addr,
    collateral: &CollateralInfo,
    offer_id: usize,
) -> Result<OfferInfo, ContractError> {
    let offer = get_offer(collateral, offer_id)?;
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
