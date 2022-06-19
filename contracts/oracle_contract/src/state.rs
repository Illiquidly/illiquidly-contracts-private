use crate::error::ContractError;
use cosmwasm_std::{Addr, Deps};
use cw_4626::state::AssetInfo;
use cw_storage_plus::{Item, Map};
use oracle_export::state::{ContractInfo, NftPrice};
pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");
pub const NFT_PRICES: Map<(&Addr, AssetInfo), NftPrice> = Map::new("fee_rates");

pub fn is_owner(deps: Deps, addr: Addr) -> Result<(), ContractError> {
    if CONTRACT_INFO.load(deps.storage)?.owner == addr {
        Ok(())
    } else {
        Err(ContractError::Unauthorized {})
    }
}
