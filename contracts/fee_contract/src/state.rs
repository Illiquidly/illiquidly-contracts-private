use cosmwasm_std::{Addr, Deps};
use cw_storage_plus::Item;
use fee_contract_export::error::ContractError;
use fee_contract_export::state::{ContractInfo, FeeInfo};

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");
pub const FEE_RATES: Item<FeeInfo> = Item::new("fee_rates");

pub fn is_admin(deps: Deps, addr: Addr) -> Result<(), ContractError> {
    if CONTRACT_INFO.load(deps.storage)?.owner == addr {
        Ok(())
    } else {
        Err(ContractError::Unauthorized {})
    }
}
