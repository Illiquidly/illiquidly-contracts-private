#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    Deps, StdResult
};


use crate::state::{CONTRACT_INFO};
use p2p_trading_export::state::{ContractInfo};

pub fn query_contract_info(deps: Deps) -> StdResult<ContractInfo>{
    CONTRACT_INFO.load(deps.storage)
}