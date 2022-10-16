use crate::error::ContractError;
#[cfg(not(feature = "library"))]
use anyhow::{anyhow, Result};
use cosmwasm_std::{
    coins, to_binary, Addr, BankMsg, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, Uint128,
};
use lender_export::state::{
    BorrowInfo, BorrowMode, BorrowZone, ContractInfo, Cw721Info, InterestType, RateIncreasor,
    BORROWS, CONTRACT_INFO, PERCENTAGE_RATE, STATE,
};
use serde::Serialize;
use utils::msg::into_cosmos_msg;

use crate::query::{
    can_repay_loan, get_asset_interests, get_asset_price, get_borrower_interest_rate,
    get_interests_with, get_last_collateral, get_liquidation_value, get_loan_value,
    get_new_interests_accrued, get_safe_zone_limit_price, get_total_interests, get_zone,
};
use cw20::Cw20ExecuteMsg;
use cw721::Cw721ExecuteMsg;
use cw_4626::msg::ExecuteMsg as Cw4626ExecuteMsg;
use cw_4626::state::AssetInfo;
use fee_distributor_export::msg::ExecuteMsg as DistributorExecuteMsg;
use fee_contract_export::state::{FeeType};
pub fn _diff_abs(x: u128, y: u128) -> u128 {
    std::cmp::max(x, y) - std::cmp::min(x, y)
}

pub fn diff_abs(x: Uint128, y: Uint128) -> Uint128 {
    Uint128::from(_diff_abs(x.u128(), y.u128()))
}

// Borrow mecanism
/// Withdraw some of the assets from the vault
/// Updates the internal structure for the loan to be liquidated when the terms allow it
pub fn execute_borrow(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    collateral_info: Cw721Info,
    assets_to_borrow: Uint128,
    borrow_mode: BorrowMode,
) -> Result<Response> {
    // First we checked it is allowed to borrow assets
    let state = STATE.load(deps.storage)?;
    if state.borrow_locked {
        return Err(anyhow::anyhow!(ContractError::BorrowLocked {}));
    }

    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    // First we query the terms of a loan involving the asset
    let asset_price = get_asset_price(deps.as_ref(), env.clone(), collateral_info.clone())?;
    let borrow_limit = get_safe_zone_limit_price(asset_price)?;
    // Then we verify the borrow limit is indeed above the assets_to_borrow
    if assets_to_borrow > borrow_limit {
        return Err(anyhow::anyhow!(ContractError::TooMuchBorrowed {
            collateral_address: collateral_info.nft_address,
            wanted: assets_to_borrow,
            limit: borrow_limit
        }));
    }
    // We get the interest rate depending on the mode chosen by the sender
    let interests = get_asset_interests(
        deps.as_ref(),
        env.clone(),
        collateral_info.clone(),
        borrow_mode,
        BorrowZone::SafeZone,
    )?;

    // We get the last collateral_id that was saved
    let new_collateral_id = get_last_collateral(deps.as_ref(), &info.sender)
        .map(|x| x + 1)
        .unwrap_or(0u64);
    // We save the borrow info to memory
    BORROWS.save(
        deps.storage,
        (&info.sender, new_collateral_id),
        &BorrowInfo {
            principle: assets_to_borrow,
            interests,
            start_block: env.block.height,
            collateral: Some(collateral_info.clone()),
            borrow_zone: BorrowZone::SafeZone,
            rate_increasor: None,
        },
    )?;

    // Then we transfer the collateral asset to this contract
    let deposit_message = into_cosmos_msg(
        Cw721ExecuteMsg::TransferNft {
            recipient: env.contract.address.into(),
            token_id: collateral_info.token_id.clone(),
        },
        collateral_info.nft_address.clone(),
        None,
    )?;

    // And we transfer the borrowed assets to the lender
    let borrow_message = into_cosmos_msg(
        Cw4626ExecuteMsg::Borrow {
            receiver: info.sender.to_string(),
            assets: assets_to_borrow,
        },
        contract_info.vault_token,
        None,
    )?;

    Ok(Response::new()
        .add_message(deposit_message)
        .add_message(borrow_message)
        .add_attribute("action", "borrow")
        .add_attribute("collateral_address", collateral_info.nft_address)
        .add_attribute("collateral_token_id", collateral_info.token_id)
        .add_attribute("borrower", info.sender))
}

// Borrow more assets for a same collateral
/// Withdraw some of the assets from the vault
pub fn execute_borrow_more(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    loan_id: u64,
    assets_to_borrow: Uint128,
) -> Result<Response> {
    // First we checked it is allowed to borrow assets
    let state = STATE.load(deps.storage)?;
    if state.borrow_locked {
        return Err(anyhow::anyhow!(ContractError::BorrowLocked {}));
    }
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    // First we make sure the loan indeed has a collateral
    let borrower = info.sender.clone();
    let mut borrow_info = BORROWS.load(deps.storage, (&borrower, loan_id))?;
    let collateral_info = borrow_info
        .clone()
        .collateral
        .ok_or(ContractError::AssetAlreadyWithdrawn {})?;

    // Then the loan type should be a continuous one to borrow more
    if let InterestType::Fixed { .. } = borrow_info.interests {
        return Err(anyhow!(ContractError::UnavailableFixedLoan {}));
    }

    // First you need to repay the increasor if you want to borrow more
    if borrow_info.borrow_zone != BorrowZone::SafeZone {
        return Err(anyhow!(ContractError::NeedToRepayExpensiveZone {}));
    }

    // We update the loan interests accrued up to there
    _update_interests_accrued(env.clone(), &mut borrow_info)?;

    let asset_price = get_asset_price(deps.as_ref(), env.clone(), collateral_info.clone())?;
    let borrow_limit = get_safe_zone_limit_price(asset_price)?;
    let current_loan_value = get_loan_value(env.clone(), &borrow_info);

    // Then we verify the borrow limit is indeed above the total assets to borrow
    if current_loan_value + assets_to_borrow > borrow_limit {
        return Err(anyhow::anyhow!(ContractError::TooMuchBorrowed {
            collateral_address: collateral_info.nft_address,
            wanted: current_loan_value + assets_to_borrow,
            limit: borrow_limit
        }));
    }

    // We set the new principle
    borrow_info.principle += assets_to_borrow;

    // We set the new interests rate
    borrow_info.interests = get_asset_interests(
        deps.as_ref(),
        env.clone(),
        collateral_info,
        BorrowMode::Continuous,
        BorrowZone::SafeZone,
    )?;
    borrow_info.start_block = env.block.height;

    // We save the borrow info to memory
    BORROWS.save(deps.storage, (&info.sender, loan_id), &borrow_info)?;

    // And we transfer the borrowed assets to the lender
    let borrow_message = into_cosmos_msg(
        Cw4626ExecuteMsg::Borrow {
            receiver: info.sender.to_string(),
            assets: assets_to_borrow,
        },
        contract_info.vault_token,
        None,
    )?;

    Ok(Response::new()
        .add_message(borrow_message)
        .add_attribute("action", "borrow")
        .add_attribute("borrower", info.sender)
        .add_attribute("loan_id", loan_id.to_string())
        .add_attribute("asset_borrowed", assets_to_borrow))
}

// Borrow mecanism
/// Repay a loan.
/// This function has multiple use cases
/// 1. Repay your own loans in whole and get your collateral back
///    In order to do that, you need to send exactly or more than the amount of assets that match the value of the lonan
/// 2. Repay parts of your loan to lower your LTV (only possible for continuous loans)
///      This will effectively lower your LTV and allow you to continue borrowing your funds
///     If you have a fixed loan this option is not available to you
/// 3. Liquidate someone elses loan (only possible when the loan is defaulted)
pub fn _execute_repay(
    deps: DepsMut,
    env: Env,
    sender: Addr,

    borrower: String,
    loan_id: u64,
    assets: Uint128,
) -> Result<Response> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    // We load the borrow object that they want to repay
    let borrower = deps.api.addr_validate(&borrower)?;
    let mut borrow_info = BORROWS.load(deps.storage, (&borrower, loan_id))?;

    // We check the sender can repay the loan
    can_repay_loan(
        deps.as_ref(),
        env.clone(),
        sender.clone(),
        borrower.clone(),
        &borrow_info,
    )?;

    // We check if there is even a collateral backing the loan
    let asset_info = borrow_info
        .clone()
        .collateral
        .ok_or(ContractError::AssetAlreadyWithdrawn {})?;
    let nft_address = asset_info.nft_address.clone();

    let loan_value = get_loan_value(env.clone(), &borrow_info);

    // First we start by dealing with the increasor incentives
    // This function will repay the incresor their share, or fail

    let (increasor_incentive, increasor_message) =
        send_interests_to_increasor(env.clone(), contract_info.clone(), &borrow_info)?
            .unwrap_or((Uint128::zero(), vec![]));

    let assets_left_to_repay: Uint128 = assets
        .checked_sub(increasor_incentive)
        .map_err(|_| anyhow!(ContractError::MustAtLeastCoverIncreasorIncentive {}))?;

    // We erase the increasor from memory
    borrow_info.rate_increasor = None;

    // Then, we update the interests accrued to the loan
    _update_interests_accrued(env.clone(), &mut borrow_info)?;

    // Now we can go to the repay part
    let repay_messages = if sender == borrower {
        if assets >= loan_value {
            // Case 1. The borrower repays the whole loan

            // We save the interests repaid for later use
            // This will always be safe (per construction, increasor_incentive is a perentage of the interests)
            let interests_repaid = assets_left_to_repay - borrow_info.principle;

            // We update the internal state of the contract to reflect the loan has ended
            // The collateral has been withdrawn
            // The principle is not existent anymore
            borrow_info.collateral = None;
            borrow_info.principle = Uint128::zero();
            [
                // We send the borrower their collateral back
                vec![into_cosmos_msg(
                    Cw721ExecuteMsg::TransferNft {
                        recipient: borrower.to_string(),
                        token_id: asset_info.token_id,
                    },
                    nft_address.clone(),
                    None,
                )?],
                // We repay the vault and the fee_depositor
                create_repay_and_fee_messages(
                    contract_info,
                    interests_repaid,
                    assets_left_to_repay,
                    nft_address,
                )?,
            ]
            .concat()
        } else {
            // Case 2. If the borrower repays the loan only partially

            // We repay part of the loan, interests first, principle second
            let interests_repaid = _repay_some_loan(env.clone(), &mut borrow_info, assets)?;

            // We update the interest rate

            let borrow_zone = get_zone(deps.as_ref(), env.clone(), &borrow_info)?;
            if borrow_zone == BorrowZone::SafeZone
                && borrow_info.borrow_zone == BorrowZone::ExpensiveZone
            {
                borrow_info.borrow_zone = BorrowZone::SafeZone;
                borrow_info.interests = get_asset_interests(
                    deps.as_ref(),
                    env,
                    asset_info,
                    BorrowMode::Continuous,
                    BorrowZone::SafeZone,
                )?;
            }
            // And we repay the vault
            create_repay_and_fee_messages(
                contract_info,
                interests_repaid,
                assets_left_to_repay,
                borrow_info.collateral.clone().unwrap().nft_address,
            )?
        }
    } else {
        // Case 3. Someone else liquidates the collateral
        // TODO
        // They can only liquidate a loan if they pay enough assets to the contract (liquidation value)
        let liquidation_value = get_liquidation_value(env, &borrow_info)?;
        if assets < liquidation_value {
            return Err(anyhow::anyhow!(ContractError::CanOnlyLiquidateWholeLoan {}));
        }

        // We save those variables to create cosmos messages
        let interests_repaid = assets_left_to_repay - borrow_info.principle;

        // The loan is ended
        // The collateral has been withdrawn
        // The principle is not existent anymore
        // The loan has been liquidated
        borrow_info.collateral = None;
        borrow_info.principle = Uint128::zero();
        borrow_info.borrow_zone = BorrowZone::LiquidationZone;

        [
            vec![into_cosmos_msg(
                Cw721ExecuteMsg::TransferNft {
                    recipient: sender.to_string(),
                    token_id: asset_info.token_id,
                },
                nft_address.clone(),
                None,
            )?],
            create_repay_and_fee_messages(
                contract_info,
                interests_repaid,
                assets_left_to_repay,
                nft_address,
            )?,
        ]
        .concat()
    };

    // We save the changes to memory
    BORROWS.save(deps.storage, (&borrower, loan_id), &borrow_info)?;

    Ok(Response::new()
        .add_messages(increasor_message)
        .add_messages(repay_messages)
        .add_attribute("action", "repay")
        .add_attribute("caller", sender)
        .add_attribute("borrower", borrower)
        .add_attribute("assets", assets.to_string())
        .add_attribute(
            "collateral_withdrawn",
            borrow_info.collateral.is_none().to_string(),
        ))
}

// In this function, we check if there was an increasor of interest rate between the last update and now
// If so, we need to send funds back to them when updating the interest rate
pub fn send_interests_to_increasor(
    env: Env,
    contract_info: ContractInfo,
    borrow_info: &BorrowInfo,
) -> Result<Option<(Uint128, Vec<CosmosMsg>)>> {
    // If we had someone increase the rate of the loan
    if let Some(increasor) = borrow_info.rate_increasor.clone() {
        let current_rate = get_borrower_interest_rate(borrow_info)?;
        let previous_rate = increasor.previous_rate;
        if current_rate > previous_rate {
            // We compute their incentive
            let incentive = get_interests_with(
                env,
                borrow_info.principle,
                current_rate - previous_rate,
                borrow_info.start_block,
            ) * contract_info.increasor_incentives
                / Uint128::from(PERCENTAGE_RATE);

            if incentive == Uint128::zero() {
                Ok(None)
            } else {
                // If there is an incentive to give, we create a message to do so
                let send_messages = send_asset(
                    contract_info.vault_asset,
                    increasor.increasor.to_string(),
                    incentive,
                )?;
                Ok(Some((incentive, vec![send_messages])))
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

pub fn create_repay_and_fee_messages(
    contract_info: ContractInfo,
    interests_due: Uint128,
    assets: Uint128,
    nft_address: String,
) -> Result<Vec<CosmosMsg>> {
    let fee = interests_due * contract_info.interests_fee_rate / Uint128::from(PERCENTAGE_RATE);
    println!(
        "fee : {:?}, repaiement : {:?}, interests : {:?}",
        fee,
        assets - fee,
        interests_due
    );
    Ok(vec![
        // We send the fee to the fee depositor
        send_asset_to_contract(
            contract_info.vault_asset.clone(),
            contract_info.fee_distributor.to_string(),
            fee,
            DistributorExecuteMsg::DepositFees {
                addresses: vec![nft_address],
                fee_type: FeeType::Funds
            },
        )?,
        // We send the rest to the vault
        send_asset_to_contract(
            contract_info.vault_asset,
            contract_info.vault_token.to_string(),
            assets - fee,
            Cw4626ExecuteMsg::Repay {
                owner: None,
                assets: assets - fee,
            },
        )?,
    ])
}

pub fn _repay_some_loan(
    env: Env,
    mut borrow_info: &mut BorrowInfo,
    assets: Uint128,
) -> Result<Uint128> {
    let total_interests = get_total_interests(env, borrow_info);
    match borrow_info.interests {
        InterestType::Fixed { .. } => {
            Err(anyhow::anyhow!(ContractError::CanOnlyRepayWholeFixedLoan {
                expected: borrow_info.principle + total_interests,
                provided: assets
            }))
        }

        InterestType::Continuous {
            last_interest_rate, ..
        } => {
            if assets > total_interests {
                borrow_info.interests = {
                    InterestType::Continuous {
                        interests_accrued: Uint128::zero(),
                        last_interest_rate,
                    }
                };
                // We diminish the principle
                if assets - total_interests <= borrow_info.principle {
                    borrow_info.principle -= assets - total_interests;
                } else {
                    borrow_info.principle = Uint128::zero();
                }

                Ok(total_interests)
            } else {
                borrow_info.interests = InterestType::Continuous {
                    interests_accrued: total_interests - assets,
                    last_interest_rate,
                };
                Ok(assets)
            }
        }
    }
}

pub fn send_asset(asset: AssetInfo, recipient: String, assets: Uint128) -> Result<CosmosMsg> {
    match asset {
        AssetInfo::Coin(denom) => Ok(CosmosMsg::from(BankMsg::Send {
            to_address: recipient,
            amount: coins(assets.u128(), denom),
        })),
        AssetInfo::Cw20(address) => into_cosmos_msg(
            Cw20ExecuteMsg::Transfer {
                recipient,
                amount: assets,
            },
            address,
            None,
        )
        .map_err(|x| anyhow!(x)),
    }
}

pub fn send_asset_to_contract<M: Serialize>(
    asset: AssetInfo,
    contract: String,
    assets: Uint128,
    msg: M,
) -> Result<CosmosMsg> {
    match asset {
        AssetInfo::Coin(denom) => Ok(into_cosmos_msg(
            msg,
            contract,
            Some(coins(assets.u128(), denom)),
        )?),
        AssetInfo::Cw20(address) => into_cosmos_msg(
            Cw20ExecuteMsg::Send {
                contract,
                amount: assets,
                msg: to_binary(&msg)?,
            },
            address,
            None,
        )
        .map_err(|x| anyhow!(x)),
    }
}

pub fn execute_raise_interest_rate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
) -> Result<Response> {
    let borrower = deps.api.addr_validate(&borrower)?;
    let mut borrow_info = BORROWS.load(deps.storage, (&borrower, loan_id))?;
    let zone = get_zone(deps.as_ref(), env.clone(), &borrow_info)?;

    // We start by updating the interest rate accrued so far
    _update_interests_accrued(env.clone(), &mut borrow_info)?;

    if zone == BorrowZone::ExpensiveZone {
        // You can only increase the rate from the safe zone
        if borrow_info.borrow_zone != BorrowZone::SafeZone {
            return Err(anyhow!(ContractError::OnlyFromSafeZone {}));
        }
        // The sender is saved in the increasor object
        if borrow_info.rate_increasor.is_some() {
            return Err(anyhow!(ContractError::CantIncreaseRateMultipleTimes {}));
        }
        borrow_info.rate_increasor = Some(RateIncreasor {
            increasor: info.sender,
            previous_rate: get_borrower_interest_rate(&borrow_info)?,
        });

        // We increase the interest rate
        borrow_info.borrow_zone = BorrowZone::ExpensiveZone;
        set_interest_rate(deps.as_ref(), env, &mut borrow_info)?;
    }

    Ok(Response::new())
}

pub fn set_interest_rate(deps: Deps, env: Env, borrow_info: &mut BorrowInfo) -> Result<()> {
    match borrow_info.interests {
        InterestType::Fixed { .. } => {
            return Err(anyhow!(ContractError::FixedLoanNoInterestRate {}))
        }
        InterestType::Continuous { .. } => {
            borrow_info.interests = get_asset_interests(
                deps,
                env,
                borrow_info
                    .collateral
                    .clone()
                    .ok_or(ContractError::AssetAlreadyWithdrawn {})?,
                BorrowMode::Continuous,
                borrow_info.borrow_zone.clone(),
            )?;
        }
    };
    Ok(())
}
pub fn _update_interests_accrued(env: Env, borrow_info: &mut BorrowInfo) -> Result<Uint128> {
    // We get the interests accrued since the last call
    let new_interest_accrued = get_new_interests_accrued(env.clone(), borrow_info);

    // We update the borrow info accordingly (only in the continuous case)
    // And we return the current interests due
    match borrow_info.interests {
        InterestType::Fixed { interests, .. } => Ok(interests),

        InterestType::Continuous {
            interests_accrued,
            last_interest_rate,
        } => {
            let new_interests = interests_accrued + new_interest_accrued;
            if new_interest_accrued > Uint128::zero() {
                borrow_info.interests = {
                    InterestType::Continuous {
                        interests_accrued: new_interests,
                        last_interest_rate,
                    }
                };
                borrow_info.start_block = env.block.height;
            }
            Ok(new_interests)
        }
    }
}
