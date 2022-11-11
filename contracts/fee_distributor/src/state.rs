use crate::error::ContractError;
use cosmwasm_std::{Addr, Coin, Deps};
use cw_storage_plus::{Item, Map};
use fee_distributor_export::state::ContractInfo;

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");
pub const ALLOCATED_FUNDS: Map<&Addr, Vec<Coin>> = Map::new("allocated_funds");
pub const ASSOCIATED_FEE_ADDRESS: Map<&Addr, Addr> = Map::new("associated_fee_address");

pub fn is_admin(deps: Deps, addr: Addr) -> Result<(), ContractError> {
    if CONTRACT_INFO.load(deps.storage)?.owner == addr {
        Ok(())
    } else {
        Err(ContractError::Unauthorized {})
    }
}

pub fn is_admin_or_address(
    deps: Deps,
    addr: Addr,
    project_address: &Addr,
    fee_address: Addr,
) -> Result<(), ContractError> {
    is_admin(deps, addr).or_else(|_| is_fee_address(deps, project_address, fee_address))
}

pub fn is_fee_address(
    deps: Deps,
    project_address: &Addr,
    fee_address: Addr,
) -> Result<(), ContractError> {
    if ASSOCIATED_FEE_ADDRESS.load(deps.storage, project_address)? == fee_address {
        Ok(())
    } else {
        Err(ContractError::Unauthorized {})
    }
}
