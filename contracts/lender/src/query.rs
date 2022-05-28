#[cfg(not(feature = "library"))]

use anyhow::Result;
use std::convert::TryInto;
use cosmwasm_std::{Deps, Env, StdResult, Uint128, Addr, Order};
use lender_export::state::{BorrowTerms, Cw721Info, BORROWS, BorrowInfo, InterestType, PERCENTAGE_RATE, FixedInterests};
use crate::error::ContractError;
pub fn get_terms(
    _deps: Deps,
    _env: Env,
    _asset_info: Cw721Info
) -> StdResult<BorrowTerms> {

    // TODO, query the oracle contract for the borrowing terms
    Ok(BorrowTerms{
        principle: Uint128::from(8742u128),
        interests: InterestType::Fixed(FixedInterests {
            interests: Uint128::from(67u128),
            duration: 100u64
        }),
    })
}

pub fn get_last_collateral(
    deps: Deps,
    owner: &Addr
) -> Option<u64> {
    let last_collateral_key: Vec<Vec<u8>> = BORROWS
       .prefix(owner)
       .keys(
            deps.storage,
            None,
            None,
            Order::Descending,
        )
        .take(1)
        .collect();

    return last_collateral_key.get(0).map(|x| u64::from_be_bytes(x.clone().try_into().unwrap()))
}



pub fn get_loan_value(env: Env, borrow_info: BorrowInfo) -> Uint128{
    let interests = match borrow_info.terms.interests{
        InterestType::Fixed(x) => x.interests,
        InterestType::Continuous(x) => x.interest_rate * borrow_info.terms.principle * Uint128::from(env.block.height - borrow_info.start_block) / Uint128::from(PERCENTAGE_RATE),

    };
    borrow_info.terms.principle + interests
}
pub fn get_liquidation_value(env: Env, borrow_info: BorrowInfo) -> Result<Uint128>{
    Ok(get_loan_value(env, borrow_info))
}

pub fn is_loan_defaulted(deps: Deps, env: Env,borrow_info: BorrowInfo) -> Result<bool>{
    // If a duration was specified, the loan defaults if and only if the duration has expired
    if let InterestType::Fixed(interests) = borrow_info.terms.interests{
        if borrow_info.start_block + interests.duration < env.block.height{
            Ok(true)
        }else{
            Ok(false)
        }
    }else{
        let asset_info = borrow_info.asset.clone().ok_or(
            ContractError::AssetAlreadyWithdrawn{}
        )?;
        // Else, we want to rely on the price oracle, we the proceed to query it
        let current_terms = get_terms(deps, env.clone(), asset_info)?;
        // If we borrowed more than the actual proposed principle against the NFT, the loan is considered liquidated
        if get_loan_value(env, borrow_info) > current_terms.principle{
            Ok(true)
        }else{
            Ok(false)
        }
    }
}

pub fn can_repay_loan(
    deps: Deps,
    env: Env,
    sender: Addr,
    borrower: Addr,
    borrow_info: BorrowInfo
) -> Result<()>{

    let loan_defaulted = is_loan_defaulted(deps, env, borrow_info)?;

    if sender == borrower{
        if loan_defaulted{
            Err(anyhow::anyhow!(ContractError::CannotRepayWhenDefaulted{}))
        }else{
            Ok(())
        }
    }else if loan_defaulted{
        // If the sender is not the borrower and the loan has defaulted, 
        // The sender can repay the loan to claim the associated NFT at a discount
        // TODO The funds sent have to be enough to claim the asset
        Ok(())
    }else{
       Err(anyhow::anyhow!(ContractError::CannotLiquidateBeforeDefault{}))
    }
}