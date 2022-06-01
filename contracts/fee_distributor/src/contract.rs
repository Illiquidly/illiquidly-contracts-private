use cosmwasm_std::{
    coin, coins, entry_point, to_binary, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo,
    Order, Response, StdError, StdResult, Uint128,
};
use cw_storage_plus::{Bound, PrimaryKey};
use itertools::Itertools;
#[cfg(not(feature = "library"))]
use std::convert::TryInto;
use utils::state::maybe_addr;

use fee_distributor_export::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use fee_distributor_export::state::ContractInfo;

use crate::error::ContractError;
use crate::state::{is_admin, ALLOCATED_FUNDS, ASSOCIATED_FEE_ADDRESS, CONTRACT_INFO};

const PROJECTS_ALLOCATION: u128 = 75u128; // In percent
const DEFAULT_LIMIT: u32 = 10u32;
const MAX_LIMIT: u32 = 30u32;
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // Verify the contract name
    msg.validate()?;

    // store contract info
    let data = ContractInfo {
        name: msg.name,
        owner: msg
            .owner
            .map(|x| deps.api.addr_validate(&x))
            .unwrap_or(Ok(info.sender))?,
        treasury: deps.api.addr_validate(&msg.treasury)?,
        projects_allocation: Uint128::from(PROJECTS_ALLOCATION),
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    Ok(Response::default()
        .add_attribute("action", "init")
        .add_attribute("contract_name", "fee_distributor"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::ModifyContractInfo {
            owner,
            treasury,
            projects_allocation,
        } => modify_contract_info(deps, env, info, owner, treasury, projects_allocation),
        ExecuteMsg::AddAssociatedAddress {
            address,
            fee_address,
        } => add_associated_address(deps, env, info, address, fee_address),
        ExecuteMsg::DepositFees { addresses } => deposit_fees(deps, env, info, addresses),
        ExecuteMsg::WithdrawFees { addresses } => withdraw_fees(deps, env, info, addresses),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // No state migrations performed, just returned a Response
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ContractInfo {} => to_binary(&contract_info(deps)?),
        QueryMsg::Amount { address } => to_binary(&query_amount(deps, address)?),
        QueryMsg::Addresses { start_after, limit } => {
            to_binary(&query_addresses(deps, start_after, limit)?)
        }
    }
}

/// Modify all contract info using this function
/// Must be the admin to change the parameters
pub fn modify_contract_info(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    owner: Option<String>,
    treasury: Option<String>,
    projects_allocation: Option<Uint128>,
) -> Result<Response, ContractError> {
    is_admin(deps.as_ref(), info.sender)?;

    let mut contract_info = CONTRACT_INFO.load(deps.storage)?;
    contract_info.owner = maybe_addr(deps.api, owner)?.unwrap_or(contract_info.owner);
    contract_info.treasury = maybe_addr(deps.api, treasury)?.unwrap_or(contract_info.treasury);
    contract_info.projects_allocation =
        projects_allocation.unwrap_or(contract_info.projects_allocation);

    if contract_info.projects_allocation.u128() > 100u128 {
        return Err(ContractError::AllocationTooHigh {});
    }
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new().add_attribute("action", "parameter_update"))
}

/// Add or modify the address associated to a token to withdraw the funds deposited in the contract
pub fn add_associated_address(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
    fee_address: String,
) -> Result<Response, ContractError> {
    is_admin(deps.as_ref(), info.sender)?;

    let valid_address = deps.api.addr_validate(&address)?;
    let valid_fee_address = deps.api.addr_validate(&fee_address)?;
    ASSOCIATED_FEE_ADDRESS.save(deps.storage, &valid_address, &valid_fee_address)?;

    Ok(Response::new()
        .add_attribute("action", "associated_address_update")
        .add_attribute("address", address)
        .add_attribute("associated_addreee", fee_address))
}

/// Main Function of this contract
/// Deposit Fees and distribute them according to the addresses provided
pub fn deposit_fees(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    addresses: Vec<String>,
) -> Result<Response, ContractError> {
    // The deposited funds must be a unique fund type
    if info.funds.len() != 1 {
        return Err(ContractError::DepositNotCorrect {});
    }

    let fund = info.funds[0].clone();
    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    let n_addresses: u128 = addresses.len().try_into().unwrap();
    let each_project_allocation = if n_addresses > 0 {
        fund.amount * contract_info.projects_allocation
            / Uint128::from(100u128)
            / Uint128::from(n_addresses)
    } else {
        Uint128::zero()
    };
    let treasury_allocation = fund.amount - each_project_allocation * Uint128::from(n_addresses);
    let each_project_fund = coin(each_project_allocation.u128(), fund.denom.clone());
    // First we save the fees that just arrived into the contract memory
    for address in &addresses {
        let valid_address = deps.api.addr_validate(address)?;
        ALLOCATED_FUNDS.update::<_, StdError>(deps.storage, &valid_address, |x| {
            match x {
                Some(mut funds) => {
                    // We check the sent funds are with the right format
                    let existing_denom = funds.iter_mut().find(|c| c.denom == fund.denom.clone());

                    if let Some(existing_fund) = existing_denom {
                        *existing_fund = Coin {
                            denom: fund.denom.clone(),
                            amount: existing_fund.amount + each_project_fund.amount,
                        };
                    } else {
                        funds.push(each_project_fund.clone());
                    }
                    Ok(funds)
                }
                None => Ok(vec![each_project_fund.clone()]),
            }
        })?;
    }
    // Then we try to distribute the fees from the addresses that were just credited (if they have an associated address)
    let fee_withdrawal_messages = if !addresses.is_empty() {
        _withdraw_registered_addresses(deps, env, info, addresses)?
    } else {
        vec![]
    };

    // We send the treasury allocation
    let treasury_message = BankMsg::Send {
        to_address: contract_info.treasury.to_string(),
        amount: coins(treasury_allocation.u128(), fund.denom.clone()),
    };

    Ok(Response::new()
        .add_attribute("action", "saved_fee")
        .add_attribute("action", "distributed_fee")
        .add_message(treasury_message)
        .add_messages(fee_withdrawal_messages))
}

/// Manually triggers withdrawal for the indicated addresses
/// The fees will be withdrawn only if the indicated addresses have an associated address registered
/// This function won't error if one or more addresses doesn't have an associated address
pub fn withdraw_fees(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    addresses: Vec<String>,
) -> Result<Response, ContractError> {
    let messages = _withdraw_registered_addresses(deps, env, info, addresses)?;
    Ok(Response::new()
        .add_attribute("action", "distributed_fee")
        .add_messages(messages))
}

/// Internal function
/// It withdraws the fees for tokens with associated addresses in the list provided in argument
pub fn _withdraw_registered_addresses(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    addresses: Vec<String>,
) -> StdResult<Vec<BankMsg>> {
    let addresses: Vec<String> = addresses.into_iter().unique().collect();
    let mut messages = vec![];
    for address in &addresses {
        let valid_address = deps.api.addr_validate(address)?;
        let loaded_funds = ALLOCATED_FUNDS.load(deps.storage, &valid_address);
        let associated_address = ASSOCIATED_FEE_ADDRESS.load(deps.storage, &valid_address);
        if let (Ok(loaded_funds), Ok(associated_address)) = (loaded_funds, associated_address) {
            messages.push(BankMsg::Send {
                to_address: associated_address.to_string(),
                amount: loaded_funds,
            });
            ALLOCATED_FUNDS.save(deps.storage, &valid_address, &vec![])?;
        }
    }
    Ok(messages)
}

pub fn contract_info(deps: Deps) -> StdResult<ContractInfo> {
    CONTRACT_INFO.load(deps.storage)
}

/// Query the amount of fee deposited in the contract for a given token address (cw721 and cw1155 supposedly)
pub fn query_amount(deps: Deps, address: String) -> StdResult<Vec<Coin>> {
    let address = deps.api.addr_validate(&address)?;
    ALLOCATED_FUNDS.load(deps.storage, &address).or(Ok(vec![]))
}

pub fn query_addresses(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<String>> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = maybe_addr(deps.api, start_after)?
        .as_ref()
        .map(|x| Bound::exclusive(x.joined_key()));

    ALLOCATED_FUNDS
        .keys(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|x| {
            std::str::from_utf8(&x)
                .map(|x| x.to_string())
                .map_err(|_| StdError::generic_err("Error while getting utf8 transcript of keys"))
        })
        .collect::<Result<Vec<String>, StdError>>()
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use cosmwasm_std::{
        from_binary,
        testing::{mock_dependencies, mock_env, mock_info},
        SubMsg,
    };
    //use cosmwasm_std::{coins, Coin, SubMsg};

    fn init_helper(deps: DepsMut) -> Response {
        let instantiate_msg = InstantiateMsg {
            name: "fee_contract".to_string(),
            owner: None,
            treasury: "treasury".to_string(),
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();

        instantiate(deps, env, info, instantiate_msg).unwrap()
    }

    #[test]
    fn test_init_sanity() {
        let mut deps = mock_dependencies(&[]);
        let res = init_helper(deps.as_mut());
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn test_modify_info() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let info = mock_info("creator", &[]);
        let env = mock_env();

        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::ModifyContractInfo {
                owner: Some("memyselfandI".to_string()),
                treasury: Some("new_treasury".to_string()),
                projects_allocation: Some(Uint128::from(34u128)),
            },
        )
        .unwrap();

        let err = execute(
            deps.as_mut(),
            env,
            info,
            ExecuteMsg::ModifyContractInfo {
                owner: Some("memyselfandI".to_string()),
                treasury: Some("new_treasury".to_string()),
                projects_allocation: Some(Uint128::from(34u128)),
            },
        )
        .unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {})
    }

    #[test]
    fn test_deposit_funds() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let info = mock_info("creator", &coins(54u128, "uluna"));
        let env = mock_env();
        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::DepositFees {
                addresses: vec!["test".to_string()],
            },
        )
        .unwrap();
        let response = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Amount {
                address: "test".to_string(),
            },
        )
        .unwrap();
        assert_eq!(
            from_binary::<Vec<Coin>>(&response).unwrap(),
            coins(54u128 * 75u128 / 100u128, "uluna")
        );

        let response = execute(
            deps.as_mut(),
            env,
            info,
            ExecuteMsg::DepositFees {
                addresses: vec!["test".to_string()],
            },
        )
        .unwrap();
        assert_eq!(
            response.messages,
            vec![SubMsg::new(BankMsg::Send {
                to_address: "treasury".to_string(),
                amount: coins(54u128 - 54u128 * 75u128 / 100u128, "uluna")
            })]
        );
    }

    #[test]
    fn test_deposit_funds_before_and_after() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let info = mock_info("creator", &coins(54u128, "uluna"));
        let env = mock_env();

        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::DepositFees {
                addresses: vec!["test".to_string()],
            },
        )
        .unwrap();

        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::DepositFees {
                addresses: vec!["test".to_string()],
            },
        )
        .unwrap();

        let response = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Amount {
                address: "test".to_string(),
            },
        )
        .unwrap();
        assert_eq!(
            from_binary::<Vec<Coin>>(&response).unwrap(),
            coins(
                54u128 * 75u128 / 100u128 + 54u128 * 75u128 / 100u128,
                "uluna"
            )
        );

        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::AddAssociatedAddress {
                address: "test".to_string(),
                fee_address: "fee".to_string(),
            },
        )
        .unwrap();

        let response = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::DepositFees {
                addresses: vec!["test".to_string()],
            },
        )
        .unwrap();

        assert_eq!(
            response.messages,
            vec![
                SubMsg::new(BankMsg::Send {
                    to_address: "treasury".to_string(),
                    amount: coins(54u128 - 54u128 * 75u128 / 100u128, "uluna")
                }),
                SubMsg::new(BankMsg::Send {
                    to_address: "fee".to_string(),
                    amount: coins(54u128 * 75u128 / 100u128 * 3u128, "uluna")
                })
            ]
        );

        let response = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Amount {
                address: "test".to_string(),
            },
        )
        .unwrap();
        assert_eq!(from_binary::<Vec<Coin>>(&response).unwrap(), vec![]);

        let response = execute(
            deps.as_mut(),
            env.clone(),
            info,
            ExecuteMsg::DepositFees {
                addresses: vec!["test".to_string()],
            },
        )
        .unwrap();

        assert_eq!(
            response.messages,
            vec![
                SubMsg::new(BankMsg::Send {
                    to_address: "treasury".to_string(),
                    amount: coins(54u128 - 54u128 * 75u128 / 100u128, "uluna")
                }),
                SubMsg::new(BankMsg::Send {
                    to_address: "fee".to_string(),
                    amount: coins(54u128 * 75u128 / 100u128, "uluna")
                })
            ]
        );

        let response = query(
            deps.as_ref(),
            env,
            QueryMsg::Amount {
                address: "test".to_string(),
            },
        )
        .unwrap();
        assert_eq!(from_binary::<Vec<Coin>>(&response).unwrap(), vec![]);
    }

    #[test]
    fn test_multiple_addresses() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let info = mock_info("creator", &coins(54u128, "uluna"));
        let env = mock_env();

        execute(
            deps.as_mut(),
            env.clone(),
            info,
            ExecuteMsg::DepositFees {
                addresses: vec![
                    "test".to_string(),
                    "test1".to_string(),
                    "test2".to_string(),
                    "test3".to_string(),
                    "test4".to_string(),
                    "test5".to_string(),
                    "test6".to_string(),
                    "test7".to_string(),
                    "test8".to_string(),
                    "test9".to_string(),
                ],
            },
        )
        .unwrap();

        let response = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Addresses {
                start_after: None,
                limit: Some(4u32),
            },
        )
        .unwrap();
        assert_eq!(
            from_binary::<Vec<String>>(&response).unwrap(),
            ["test", "test1", "test2", "test3"]
        );

        let response = query(
            deps.as_ref(),
            env,
            QueryMsg::Addresses {
                start_after: Some("test3".to_string()),
                limit: Some(8u32),
            },
        )
        .unwrap();
        assert_eq!(
            from_binary::<Vec<String>>(&response).unwrap(),
            ["test4", "test5", "test6", "test7", "test8", "test9"]
        );
    }
}
