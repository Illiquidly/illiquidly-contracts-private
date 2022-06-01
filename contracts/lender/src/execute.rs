use crate::error::ContractError;
#[cfg(not(feature = "library"))]
use anyhow::Result;
use cosmwasm_std::{DepsMut, Env, MessageInfo, Response,  Uint128};
use lender_export::state::{ STATE, CONTRACT_INFO, Cw721Info, BorrowTerms, BORROWS, BorrowInfo, InterestType};
use utils::msg::into_cosmos_msg;

use cw_4626::msg::ExecuteMsg as Cw4626ExecuteMsg;
use cw721::Cw721ExecuteMsg;

use crate::query::{get_terms, get_loan_value, get_liquidation_value, get_last_collateral, can_repay_loan};


pub fn _diff_abs(x: u128, y:u128) -> u128{
    std::cmp::max(x, y) - std::cmp::min(x,y)
}

pub fn diff_abs(x: Uint128, y:Uint128) -> Uint128{
    Uint128::from(_diff_abs(x.u128(),y.u128()))
}



// Borrow mecanism
/// Withdraw some of the assets from the vault
/// Updates the internal structure for the loan to be liquidated when the terms allow it
pub fn execute_borrow(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset_info: Cw721Info,
    wanted_terms: BorrowTerms,
    principle_slippage: Uint128

) -> Result<Response> {
    // First we checked it is allowed to borrow assets
    let state = STATE.load(deps.storage)?;  
    if state.borrow_locked{
        return Err(anyhow::anyhow!(ContractError::BorrowLocked{}));
    }

    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    // First we query the terms of a loan involving the asset
    let current_terms = get_terms(deps.as_ref(), env.clone(), asset_info.clone())?;
    // Then we verify the terms coincide with what the sender wants
    if diff_abs(wanted_terms.principle, current_terms.principle) > principle_slippage{
        return Err(anyhow::anyhow!(ContractError::TooMuchSlippage{}));
    }

    // We get the last collateral_id that was saved
    let new_collateral_id = get_last_collateral(deps.as_ref(), &info.sender)
        .map(|x| x+1)
        .unwrap_or(0u64);
    // We save the borrow info to memory
    BORROWS.save(deps.storage, (&info.sender, new_collateral_id.into()), &BorrowInfo{
        terms: current_terms.clone(),
        start_block: env.block.height,
        asset: Some(asset_info.clone())
    })?;

    // Then we transfer the collateral asset to this contract
    let deposit_message = into_cosmos_msg(
        Cw721ExecuteMsg::TransferNft {
                recipient: env.contract.address.into(),
                token_id: asset_info.token_id.clone(),
            },
            asset_info.nft_address.clone(),
            None
    )?;

    // And we transfer the borrowed assets to the lender
    let borrow_message = into_cosmos_msg(
        Cw4626ExecuteMsg::Borrow {
            receiver: info.sender.to_string(),
            assets: current_terms.principle,
        },
        contract_info.vault_token,
        None
    )?;

    Ok(Response::new()
        .add_message(deposit_message)
        .add_message(borrow_message)
        .add_attribute("action", "borrow")
        .add_attribute("collateral_address",asset_info.nft_address)
        .add_attribute("collateral_token_id", asset_info.token_id)
        .add_attribute("borrower", info.sender))
}

// Borrow mecanism
/// Repay funds to lower the amount of debt of the contract
/// If more funds than the current debt are sent via this mecanism, assets are deposited in the vault but no extra tokens are minted
pub fn execute_repay(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    borrower:String,
    loan_id: u64,
    assets: Uint128
) -> Result<Response> {

    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    // We load the borrow object that they want to repay
    let borrower = deps.api.addr_validate(&borrower)?;
    let mut borrow_info = BORROWS.load(deps.storage, (&borrower, loan_id.into()))?;

    // We check the sender can repay the loan
    can_repay_loan(deps.as_ref(), env.clone(), info.sender.clone(), borrower.clone(), borrow_info.clone())?;

    // We check if there is even an asset to withdraw !
    let asset_info = borrow_info.clone().asset.ok_or(
        ContractError::AssetAlreadyWithdrawn{})?;

    // If the sender is the borrower, they can repay part of the loan to lower their loan to value ratio
    // Else they have to repay the whole loan and they get the asset back
    let loan_value = get_loan_value(env.clone(), borrow_info.clone());
    let collateral_messages = if info.sender == borrower{
        // If the borrower repays the whole loan
        if assets >= loan_value{
            borrow_info.terms.principle = Uint128::zero();
            // We send them their collateral back

            borrow_info.asset = None;
            vec![into_cosmos_msg(
                Cw721ExecuteMsg::TransferNft{
                    recipient: borrower.to_string(),
                    token_id: asset_info.token_id
                },
                asset_info.nft_address,
                None
            )?]
        }else{
            // If the borrower repays the loan only partially
            // The borrower can only repay loans partially if they are continuous
            if let InterestType::Fixed(_) = borrow_info.terms.interests.clone(){
                return Err(anyhow::anyhow!(ContractError::CanOnlyRepayWholeFixedLoan{
                    expected: loan_value,
                    provided: assets
                }))
            }
            borrow_info.terms.principle = loan_value - assets;
            borrow_info.start_block = env.block.height;
            vec![]
        }
    }else{
        // In case someone wants to liquidate the collateral
        let liquidation_value = get_liquidation_value(env, borrow_info.clone())?;
        if assets < liquidation_value{
             return Err(anyhow::anyhow!(ContractError::CanOnlyLiquidateWholeLoan{}))
        }
        borrow_info.asset = None;
        vec![into_cosmos_msg(
                Cw721ExecuteMsg::TransferNft{
                    recipient: info.sender.to_string(),
                    token_id: asset_info.token_id
                },
                asset_info.nft_address,
                None
        )?]
    };

    BORROWS.save(deps.storage, (&borrower, loan_id.into()), &borrow_info)?;

    // We repay assets to the vault
    let repay_message = into_cosmos_msg(
        Cw4626ExecuteMsg::Repay{
            owner: Some(info.sender.to_string()),
            assets,
        },
        contract_info.vault_token,
        Some(info.funds)
    )?;


    Ok(Response::new()
        .add_messages(collateral_messages.clone())
        .add_message(repay_message)
        .add_attribute("action", "repay")
        .add_attribute("caller", info.sender)
        .add_attribute("borrower", borrower)
        .add_attribute("assets", assets.to_string())
        .add_attribute("collateral_withdrawn", (!collateral_messages.is_empty()).to_string())
    )
}
