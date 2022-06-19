use crate::error::ContractError;
use cosmwasm_std::{Addr, Deps};
use cw_storage_plus::{Item, IndexedMap, MultiIndex};
use escrow_export::state::{ContractInfo, TokenOwner, TokenIndexes};

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");

pub fn token_owner_idx(d: &TokenOwner) -> Addr {
    d.owner.clone()
}

pub struct DepositNft<'a> {
    pub nfts: IndexedMap<'a, &'a str, TokenOwner, TokenIndexes<'a>>,
}

impl Default for DepositNft<'_>{
    fn default() -> Self {

        let indexes: TokenIndexes = TokenIndexes {
                owner: MultiIndex::new(token_owner_idx, "tokens", "tokens__owner"),
            };
        Self{
            nfts: IndexedMap::new("tokens", indexes)
        }
    }
}

pub fn is_owner(deps: Deps, addr: Addr) -> Result<(), ContractError> {
    if CONTRACT_INFO.load(deps.storage)?.owner == addr {
        Ok(())
    } else {
        Err(ContractError::Unauthorized {})
    }
}
