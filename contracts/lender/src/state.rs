use crate::error::ContractError;
use anyhow::{anyhow, Result};
use cosmwasm_std::{Addr, DepsMut, MessageInfo, Response, Storage, Uint128};
use lender_export::state::{ContractInfo, CONTRACT_INFO, PERCENTAGE_RATE, STATE};

pub fn is_owner(storage: &dyn Storage, sender: Addr) -> Result<ContractInfo, ContractError> {
    let contract_info = CONTRACT_INFO.load(storage)?;
    if sender == contract_info.owner {
        Ok(contract_info)
    } else {
        Err(ContractError::Unauthorized {})
    }
}

pub fn set_owner(deps: DepsMut, info: MessageInfo, owner: String) -> Result<Response> {
    let mut contract_info = is_owner(deps.storage, info.sender.clone())?;
    contract_info.owner = deps.api.addr_validate(&owner)?;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "set_owner")
        .add_attribute("caller", info.sender)
        .add_attribute("owner", owner))
}

pub fn set_oracle(deps: DepsMut, info: MessageInfo, oracle: String) -> Result<Response> {
    let mut contract_info = is_owner(deps.storage, info.sender.clone())?;
    contract_info.oracle = deps.api.addr_validate(&oracle)?;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "set_oracle")
        .add_attribute("caller", info.sender)
        .add_attribute("oracle", oracle))
}

pub fn set_lock(deps: DepsMut, info: MessageInfo, lock: bool) -> Result<Response> {
    is_owner(deps.storage, info.sender.clone())?;

    STATE.update::<_, anyhow::Error>(deps.storage, |mut x| {
        x.borrow_locked = lock;
        Ok(x)
    })?;

    Ok(Response::new()
        .add_attribute("action", "set_oracle")
        .add_attribute("caller", info.sender)
        .add_attribute("lock", lock.to_string()))
}

pub fn set_increasor_incentives(
    deps: DepsMut,
    info: MessageInfo,
    incentives: Uint128,
) -> Result<Response> {
    let mut contract_info = is_owner(deps.storage, info.sender.clone())?;
    if incentives.u128() > PERCENTAGE_RATE {
        return Err(anyhow!(ContractError::ParamNotAccepted {}));
    }
    contract_info.increasor_incentives = incentives;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "set_oracle")
        .add_attribute("caller", info.sender)
        .add_attribute("increasor_incentives", incentives.to_string()))
}

pub fn set_interests_fee_rate(deps: DepsMut, info: MessageInfo, rate: Uint128) -> Result<Response> {
    let mut contract_info = is_owner(deps.storage, info.sender.clone())?;
    if rate.u128() > PERCENTAGE_RATE {
        return Err(anyhow!(ContractError::ParamNotAccepted {}));
    }
    contract_info.interests_fee_rate = rate;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "set_oracle")
        .add_attribute("caller", info.sender)
        .add_attribute("interests_fee_rate", rate.to_string()))
}
