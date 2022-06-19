#[cfg(not(feature = "library"))]
use cosmwasm_std::{Deps, Env, StdResult, Uint128};
use cw20_base::state::{TokenInfo, TOKEN_INFO};

use cw_4626::state::{query_asset_balance, query_asset_liabilities, AssetInfo, STATE};
use bignumber::{Decimal256, Uint256};

use cw20_base::contract::query_balance;
pub fn query_asset(deps: Deps) -> StdResult<AssetInfo> {
    let state = STATE.load(deps.storage)?;
    Ok(state.underlying_asset)
}

pub fn query_total_assets(deps: Deps) -> StdResult<Uint128> {
    let state = STATE.load(deps.storage)?;
    Ok(state.total_underlying_asset_supply)
}

pub fn compute_exchange_rate(
    deps: Deps,
    env: Env,
    token_info: &TokenInfo,
    deposit_amount: Option<Uint128>,
) -> StdResult<Decimal256> {
    let share_supply = token_info.total_supply;
    if share_supply.is_zero() {
        return Ok(Decimal256::one());
    }

    let asset_balance =
        query_asset_balance(deps, env.clone())? - deposit_amount.unwrap_or_else(Uint128::zero);
    let liabilities = query_asset_liabilities(deps, env)?;
    let total_assets = Decimal256::from_uint256(Uint256::from((asset_balance + liabilities).u128()));
    Ok(total_assets / Decimal256::from_uint256(Uint256::from(share_supply.u128())))
}

pub fn convert_to_shares(
    deps: Deps,
    env: Env,
    assets: Uint128,
    deposit_amount: Option<Uint128>,
) -> StdResult<Uint128> {
    let token_info = TOKEN_INFO.load(deps.storage)?;

    let exchange_rate = compute_exchange_rate(deps, env, &token_info, deposit_amount)?;
    Ok((Uint256::from(assets.u128()) / exchange_rate).into())
}

pub fn convert_to_assets(
    deps: Deps,
    env: Env,
    shares: Uint128,
    deposit_amount: Option<Uint128>,
) -> StdResult<Uint128> {
    let token_info = TOKEN_INFO.load(deps.storage)?;
    let exchange_rate = compute_exchange_rate(deps, env, &token_info, deposit_amount)?;
    Ok((Uint256::from(shares) * exchange_rate).into())
}
pub fn preview_deposit(deps: Deps, env: Env, assets: Uint128) -> StdResult<Uint128> {
    convert_to_shares(deps, env, assets, None)
}

pub fn preview_mint(deps: Deps, env: Env, shares: Uint128) -> StdResult<Uint128> {
    convert_to_assets(deps, env, shares, None)
}

pub fn preview_withdraw(deps: Deps, env: Env, assets: Uint128) -> StdResult<Uint128> {
    convert_to_shares(deps, env, assets, None)
}

pub fn preview_redeem(deps: Deps, env: Env, shares: Uint128) -> StdResult<Uint128> {
    convert_to_assets(deps, env, shares, None)
}

pub fn max_deposit(_deps: Deps, _env: Env, _receiver: String) -> StdResult<Uint128> {
    Ok(Uint128::MAX)
}

pub fn max_mint(_deps: Deps, _env: Env, _receiver: String) -> StdResult<Uint256> {
    Ok(Uint256::from(Uint128::MAX))
}

pub fn max_withdraw(deps: Deps, env: Env, owner: String) -> StdResult<Uint128> {
    convert_to_shares(deps, env.clone(), max_redeem(deps, env, owner)?, None)
}

pub fn max_redeem(deps: Deps, _env: Env, owner: String) -> StdResult<Uint128> {
    Ok(query_balance(deps, owner)?.balance)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::contract::instantiate;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::DepsMut;
    use cw_4626::msg::InstantiateMsg;
    fn init_helper(deps: DepsMut) {
        let instantiate_msg = InstantiateMsg {
            name: "Iliq treasury token".to_string(),
            symbol: "ailiq".to_string(),
            decimals: 6u8,
            initial_balances: vec![],
            mint: None,
            marketing: None,
            asset: AssetInfo::Coin("uluna".to_string()),
            borrower: Some("borrower".to_string()),
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();

        instantiate(deps, env, info, instantiate_msg).unwrap();
    }

    #[test]
    fn test_convert_sanity() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        init_helper(deps.as_mut());
        let initial_assets = Uint128::from(6764562356574737676u128);
        let shares = convert_to_shares(deps.as_ref(), env.clone(), initial_assets, None).unwrap();

        let final_assets = convert_to_assets(deps.as_ref(), env, shares, None).unwrap();

        assert_eq!(initial_assets, final_assets);
    }
}
