use cosmwasm_std::{Addr, Deps, StdError, StdResult};
use cw_storage_plus::Item;
use fee_contract_export::state::{ContractInfo, FeeInfo};

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");
pub const FEE_RATES: Item<FeeInfo> = Item::new("fee_rates");

pub fn is_admin(deps: Deps, addr: Addr) -> StdResult<()> {
    if CONTRACT_INFO.load(deps.storage)?.owner == addr {
        Ok(())
    } else {
        Err(StdError::generic_err("Unauthorized"))
    }
}
