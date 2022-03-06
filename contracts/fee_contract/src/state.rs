use cw_storage_plus::{Item};

use fee_contract_export::state::ContractInfo;

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");