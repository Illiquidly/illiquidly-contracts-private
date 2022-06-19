#[cfg(not(feature = "library"))]
use anyhow::{anyhow, Result};
use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, Uint128,
};

use crate::error::ContractError;
use oracle_export::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, NftPriceResponse, QueryMsg};
use oracle_export::state::{ContractInfo, NftPrice};

use crate::state::{is_owner, CONTRACT_INFO, NFT_PRICES};
use cw_4626::state::AssetInfo;

const DEFAULT_TIMEOUT: u64 = 8 * 3600; // Price timeout in seconds (8hrs)

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // Verify the contract name
    msg.validate()?;
    // store token info
    let data = ContractInfo {
        name: msg.name,
        owner: msg
            .owner
            .map(|x| deps.api.addr_validate(&x))
            .unwrap_or(Ok(info.sender))?,
        timeout: msg.timeout.unwrap_or(DEFAULT_TIMEOUT),
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    // Initialisation with fixed rates

    Ok(Response::default().add_attribute("fee_contract", "init"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> Result<Response> {
    match msg {
        ExecuteMsg::SetNftPrice {
            contract,
            oracle_owner,
            price,
            unit,
        } => execute_set_nft_price(
            deps,
            env,
            info.clone(),
            contract,
            oracle_owner.unwrap_or_else(|| info.sender.to_string()),
            unit,
            price,
        ),
        ExecuteMsg::SetOwner { owner } => set_owner(deps, env, info, owner),
        ExecuteMsg::SetTimeout { timeout } => set_timeout(deps, env, info, timeout),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // No state migrations performed, just returned a Response
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary> {
    match msg {
        QueryMsg::ContractInfo {} => to_binary(&contract_info(deps)?).map_err(|e| anyhow!(e)),
        QueryMsg::NftPrice { contract, unit } => {
            to_binary(&query_nft_price(deps, env, contract, unit)?).map_err(|e| anyhow!(e))
        }
    }
}

/// This function is used to withdraw funds from an accepted trade.
/// It uses information from the trades and counter trades to determine how much needs to be paid
/// If the fee is sufficient, it sends the fee to the fee_depositor contract (responsible for fee distribution)
pub fn execute_set_nft_price(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    contract: String,
    oracle_owner: String,
    unit: AssetInfo,
    price: Uint128,
) -> Result<Response> {
    // The fee can be paid in any Terra native currency.
    // It needs to be paid in a single currency
    if info.funds.len() != 1 {
        return Err(anyhow!(ContractError::FeeNotPaid {}));
    }
    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    let contract_addr = deps.api.addr_validate(&contract)?;
    let oracle_owner_addr = deps.api.addr_validate(&oracle_owner)?;
    NFT_PRICES.update(deps.storage, (&contract_addr, unit.clone()), |x| match x {
        Some(nft_price) => {
            if info.sender != nft_price.oracle_owner {
                return Err(anyhow!(ContractError::Unauthorized {}));
            }
            Ok(NftPrice {
                price,
                oracle_owner: oracle_owner_addr,
                last_update: env.block.time,
            })
        }
        None => {
            if info.sender != contract_info.owner {
                return Err(anyhow!(ContractError::Unauthorized {}));
            }
            Ok(NftPrice {
                price,
                oracle_owner: oracle_owner_addr,
                last_update: env.block.time,
            })
        }
    })?;

    Ok(Response::new()
        .add_attribute("action", "set_oracle_price")
        .add_attribute("nft", contract)
        .add_attribute("unit", unit.to_string())
        .add_attribute("price", price.to_string()))
}

pub fn set_owner(deps: DepsMut, _env: Env, info: MessageInfo, owner: String) -> Result<Response> {
    is_owner(deps.as_ref(), info.sender)?;

    let owner_addr = deps.api.addr_validate(&owner)?;
    CONTRACT_INFO.update::<_, StdError>(deps.storage, |mut x| {
        x.owner = owner_addr;
        Ok(x)
    })?;

    Ok(Response::new()
        .add_attribute("action", "parameter_update")
        .add_attribute("parameter", "owner")
        .add_attribute("value", owner))
}

pub fn set_timeout(deps: DepsMut, _env: Env, info: MessageInfo, timeout: u64) -> Result<Response> {
    is_owner(deps.as_ref(), info.sender)?;

    CONTRACT_INFO.update::<_, StdError>(deps.storage, |mut x| {
        x.timeout = timeout;
        Ok(x)
    })?;

    Ok(Response::new()
        .add_attribute("action", "parameter_update")
        .add_attribute("parameter", "timeout")
        .add_attribute("value", timeout.to_string()))
}

pub fn contract_info(deps: Deps) -> Result<ContractInfo> {
    CONTRACT_INFO.load(deps.storage).map_err(|e| anyhow!(e))
}

pub fn query_nft_price(
    deps: Deps,
    env: Env,
    contract: String,
    unit: AssetInfo,
) -> Result<NftPriceResponse> {
    let contract_addr = deps.api.addr_validate(&contract)?;
    let nft_price = NFT_PRICES.load(deps.storage, (&contract_addr, unit.clone()))?;
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    Ok(NftPriceResponse {
        contract,
        price: nft_price.price,
        unit,
        oracle_owner: nft_price.oracle_owner.to_string(),
        timeout: nft_price.last_update.plus_seconds(contract_info.timeout) < env.block.time,
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    //use cosmwasm_std::{coins, Coin, SubMsg};

    fn init_helper(deps: DepsMut) -> Response {
        let instantiate_msg = InstantiateMsg {
            name: "fee_contract".to_string(),
            owner: None,
            timeout: Some(8 * 3600u64),
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

    /*

    #[test]
    fn test_update_fee_rates() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let info = mock_info("creator", &[]);
        let env = mock_env();
        execute(
            deps.as_mut(),
            env,
            info,
            ExecuteMsg::UpdateFeeRates {
                asset_fee_rate: Some(Uint128::from(5u128)), // In thousandths
                fee_max: Some(Uint128::from(6u128)),        // In uusd
                first_teer_limit: Some(Uint128::from(7u128)),
                first_teer_rate: Some(Uint128::from(8u128)),
                second_teer_limit: Some(Uint128::from(9u128)),
                second_teer_rate: Some(Uint128::from(10u128)),
                third_teer_rate: Some(Uint128::from(11u128)),
                acceptable_fee_deviation: Some(Uint128::from(12u128)),
            },
        )
        .unwrap();

        let fee_rate = FEE_RATES.load(&deps.storage).unwrap();
        assert_eq!(
            fee_rate,
            FeeInfo {
                asset_fee_rate: Uint128::from(5u128), // In thousandths
                fee_max: Uint128::from(6u128),        // In uusd
                first_teer_limit: Uint128::from(7u128),
                first_teer_rate: Uint128::from(8u128),
                second_teer_limit: Uint128::from(9u128),
                second_teer_rate: Uint128::from(10u128),
                third_teer_rate: Uint128::from(11u128),
                acceptable_fee_deviation: Uint128::from(12u128),
            }
        );
    }

    #[test]
    fn test_fee_amount() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let fee = fee_amount_raw(deps.as_ref(), &[], &[]).unwrap();
        assert_eq!(fee, Uint128::zero());

        let fee = fee_amount_raw(
            deps.as_ref(),
            &[AssetInfo::Cw20Coin(Cw20Coin {
                amount: Uint128::from(42u64),
                address: "token".to_string(),
            })],
            &[],
        )
        .unwrap();
        assert_eq!(fee, Uint128::new(250_000u128));

        let fee = fee_amount_raw(
            deps.as_ref(),
            &[AssetInfo::Cw20Coin(Cw20Coin {
                amount: Uint128::from(42u64),
                address: "token".to_string(),
            })],
            &[AssetInfo::Cw20Coin(Cw20Coin {
                amount: Uint128::from(42u64),
                address: "token".to_string(),
            })],
        )
        .unwrap();
        assert_eq!(fee, Uint128::new(500_000u128));

        let fee = fee_amount_raw(
            deps.as_ref(),
            &[
                AssetInfo::Cw20Coin(Cw20Coin {
                    amount: Uint128::from(42u64),
                    address: "token".to_string(),
                }),
                AssetInfo::Cw20Coin(Cw20Coin {
                    amount: Uint128::from(42u64),
                    address: "token".to_string(),
                }),
            ],
            &[],
        )
        .unwrap();
        assert_eq!(fee, Uint128::new(500_000u128));
    }
    */
}
