use crate::error::ContractError;
use cosmwasm_std::{Addr, Deps};
use cw_storage_plus::{Item, Map};
use escrow_export::state::ContractInfo;

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");
pub const DEPOSITED_NFTS: Map<&str, Addr> = Map::new("deposited_nfts");
pub const USER_OWNED_NFTS: Map<&Addr, Vec<String>> = Map::new("user_owned_nfts");

pub fn is_owner(deps: Deps, addr: Addr) -> Result<(), ContractError> {
    if CONTRACT_INFO.load(deps.storage)?.owner == addr {
        Ok(())
    } else {
        Err(ContractError::Unauthorized {})
    }
}
