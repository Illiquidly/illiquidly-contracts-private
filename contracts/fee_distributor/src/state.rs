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
