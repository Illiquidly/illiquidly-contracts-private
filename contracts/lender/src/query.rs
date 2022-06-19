use crate::error::ContractError;
#[cfg(not(feature = "library"))]
use anyhow::{anyhow, Result};
use cosmwasm_std::{Addr, Deps, Env, Order, StdResult, Uint128};
use cw_4626::query::QueryMsg as Cw4626QueryMsg;
use cw_4626::state::AssetInfo;
use lender_export::state::{
    BorrowInfo, BorrowMode, BorrowZone, Cw721Info, InterestType, InterestsInfo, BORROWS,
    MIN_BLOCK_OFFSET, PERCENTAGE_RATE,
};
use std::convert::TryInto;

const SAFE_ZONE_LIMIT: u128 = 3_333u128;
const EXPENSIVE_ZONE_LIMIT: u128 = 6_666u128;
const ZONE_LIMIT_PERCENTAGE_POINT: u128 = 10_000u128;

pub fn get_vault_token_asset(deps: Deps, vault_token: String) -> Result<AssetInfo> {
    let asset_info: AssetInfo = deps
        .querier
        .query_wasm_smart(vault_token, &Cw4626QueryMsg::Asset {})
        .map_err(|x| anyhow!(x))?;
    Ok(asset_info)
}

pub fn get_asset_interests(
    _deps: Deps,
    _env: Env,
    _asset_info: Cw721Info,
    mode: BorrowMode,
    zone: BorrowZone,
) -> StdResult<InterestType> {
    // TODO, determine a borrowing strategy !

    let interests_info = InterestsInfo {
        safe_interest_rate: Uint128::from(78u128), // In 1/PERCENTAGE_RATE per block
        expensive_interest_rate: Uint128::from(78u128), // In 1/PERCENTAGE_RATE per block
    };

    match mode {
        BorrowMode::Fixed => Ok(InterestType::Fixed {
            interests: Uint128::from(67u128),
            duration: 67u64,
        }),
        BorrowMode::Continuous => Ok(match zone {
            BorrowZone::SafeZone => InterestType::Continuous {
                last_interest_rate: interests_info.safe_interest_rate,
                interests_accrued: Uint128::zero(),
            },
            BorrowZone::ExpensiveZone | BorrowZone::LiquidationZone => InterestType::Continuous {
                last_interest_rate: interests_info.expensive_interest_rate,
                interests_accrued: Uint128::zero(),
            },
        }),
    }
}

pub fn get_borrower_interest_rate(borrow_info: &BorrowInfo) -> Result<Uint128> {
    match borrow_info.interests {
        InterestType::Fixed { .. } => Err(anyhow!(ContractError::FixedLoanNoInterestRate {})),
        InterestType::Continuous {
            last_interest_rate, ..
        } => Ok(last_interest_rate),
    }
}

pub fn get_asset_price(_deps: Deps, _env: Env, _asset_info: Cw721Info) -> StdResult<Uint128> {
    // TODO, query the oracle contract for the asset price
    Ok(Uint128::from(161_000_000u128))
}

pub fn get_safe_zone_limit_price(asset_price: Uint128) -> StdResult<Uint128> {
    Ok(asset_price * Uint128::from(SAFE_ZONE_LIMIT) / Uint128::from(ZONE_LIMIT_PERCENTAGE_POINT))
}

pub fn get_expensive_zone_limit_price(asset_price: Uint128) -> StdResult<Uint128> {
    Ok(asset_price * Uint128::from(EXPENSIVE_ZONE_LIMIT)
        / Uint128::from(ZONE_LIMIT_PERCENTAGE_POINT))
}

pub fn get_zone(deps: Deps, env: Env, borrow_info: &BorrowInfo) -> Result<BorrowZone> {
    match borrow_info.interests {
        InterestType::Fixed { .. } => Ok(BorrowZone::SafeZone),
        InterestType::Continuous { .. } => {
            let loan_value = get_loan_value(env.clone(), borrow_info);
            let asset_price = get_asset_price(
                deps,
                env,
                borrow_info
                    .clone()
                    .collateral
                    .ok_or(ContractError::AssetAlreadyWithdrawn {})?,
            )?;
            if loan_value <= get_safe_zone_limit_price(asset_price)? {
                Ok(BorrowZone::SafeZone)
            } else if loan_value <= get_expensive_zone_limit_price(asset_price)? {
                Ok(BorrowZone::ExpensiveZone)
            } else {
                Ok(BorrowZone::LiquidationZone)
            }
        }
    }
}

pub fn get_last_collateral(deps: Deps, owner: &Addr) -> Option<u64> {
    let last_collateral_key: Vec<Vec<u8>> = BORROWS
        .prefix(owner)
        .keys(deps.storage, None, None, Order::Descending)
        .take(1)
        .collect();

    return last_collateral_key
        .get(0)
        .map(|x| u64::from_be_bytes(x.clone().try_into().unwrap()));
}

pub fn get_total_interests(env: Env, borrow_info: &BorrowInfo) -> Uint128 {
    let old_interests = match borrow_info.interests {
        InterestType::Fixed { interests, .. } => interests,
        InterestType::Continuous {
            interests_accrued, ..
        } => interests_accrued,
    };
    println!("{:?}", old_interests);
    old_interests + get_new_interests_accrued(env, borrow_info)
}

pub fn get_loan_value(env: Env, borrow_info: &BorrowInfo) -> Uint128 {
    let interests = get_total_interests(env, borrow_info);
    borrow_info.principle + interests
}

pub fn get_new_interests_accrued(env: Env, borrow_info: &BorrowInfo) -> Uint128 {
    match borrow_info.interests {
        InterestType::Fixed { .. } => Uint128::zero(),
        InterestType::Continuous {
            last_interest_rate, ..
        } => get_interests_with(
            env,
            borrow_info.principle,
            last_interest_rate,
            borrow_info.start_block,
        ),
    }
}

pub fn get_interests_with(
    env: Env,
    principle: Uint128,
    interest_rate: Uint128,
    start_block: u64,
) -> Uint128 {
    interest_rate
        * principle
        * Uint128::from((env.block.height - start_block) / MIN_BLOCK_OFFSET * MIN_BLOCK_OFFSET)
        / Uint128::from(PERCENTAGE_RATE)
    // Here we divide and multiply by MIN_BLOCK_OFFSET to make sure the interests due don't fluctuate too much
}
pub fn get_liquidation_value(env: Env, borrow_info: &BorrowInfo) -> Result<Uint128> {
    // TODO determine a liquidation strategy
    Ok(get_loan_value(env, borrow_info))
}

pub fn is_loan_defaulted(deps: Deps, env: Env, borrow_info: &BorrowInfo) -> Result<bool> {
    // If a duration was specified, the loan defaults if and only if the duration has expired
    if let InterestType::Fixed { duration, .. } = borrow_info.interests {
        if borrow_info.start_block + duration < env.block.height {
            Ok(true)
        } else {
            Ok(false)
        }
    } else {
        // Else, we want to rely on the price oracle to determine if the loan is liquidated
        let asset_info = borrow_info
            .collateral
            .clone()
            .ok_or(ContractError::AssetAlreadyWithdrawn {})?;
        // We query the asset price using the on-chain oracle
        let asset_price = get_asset_price(deps, env.clone(), asset_info)?;

        // The loan is considered liquidated when the loan value is more than EXPENSIVE_ZONE_LIMIT*asset_price
        if get_loan_value(env, borrow_info) > get_expensive_zone_limit_price(asset_price)? {
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

pub fn can_repay_loan(
    deps: Deps,
    env: Env,
    sender: Addr,
    borrower: Addr,
    borrow_info: &BorrowInfo,
) -> Result<()> {
    let loan_defaulted = is_loan_defaulted(deps, env, borrow_info)?;

    if sender == borrower {
        if loan_defaulted {
            Err(anyhow::anyhow!(ContractError::CannotRepayWhenDefaulted {}))
        } else {
            Ok(())
        }
    } else if loan_defaulted {
        // If the sender is not the borrower and the loan has defaulted,
        // The sender can repay the loan to claim the associated NFT at a discount
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            ContractError::CannotLiquidateBeforeDefault {}
        ))
    }
}
