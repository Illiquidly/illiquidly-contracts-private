use cw_storage_plus::{Item, Map, U64Key};

use cosmwasm_std::{Addr, Storage};

use crate::error::ContractError;
use nft_loans_export::state::{BorrowerInfo, CollateralInfo, ContractInfo, LoanState};

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");

pub const COLLATERAL_INFO: Map<(&Addr, U64Key), CollateralInfo> = Map::new("collateral_info");

pub const BORROWER_INFO: Map<&Addr, BorrowerInfo> = Map::new("collateral_info");

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
        LoanState::Published | LoanState::Ended => Ok(()),
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
