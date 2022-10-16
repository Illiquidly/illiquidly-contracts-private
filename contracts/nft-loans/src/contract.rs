#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_binary, Addr, BankMsg, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Order,
    Response, StdError, StdResult, Uint128,
};
use cw_storage_plus::Bound;

use crate::error::ContractError;

use crate::state::{
    add_new_offer, can_repay_loan, get_active_loan, get_offer, is_active_lender,
    is_collateral_withdrawable, is_lender, is_loan_acceptable, is_loan_counterable,
    is_loan_defaulted, is_loan_modifiable, is_owner, BORROWER_INFO, COLLATERAL_INFO, CONTRACT_INFO,
    LENDER_OFFERS,
};

use cw1155::Cw1155ExecuteMsg;
use cw721::Cw721ExecuteMsg;

use nft_loans_export::msg::{
    CollateralResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, OfferResponse, QueryMsg,
};
use nft_loans_export::state::{
    BorrowerInfo, CollateralInfo, ContractInfo, LoanState, LoanTerms, OfferInfo, OfferState,
};
use utils::msg::into_cosmos_msg;
use utils::state::{AssetInfo, Cw1155Coin, Cw721Coin};

use fee_contract_export::state::FeeType;
use fee_distributor_export::msg::{ExecuteMsg as FeeDistributorMsg};

const MAX_QUERY_LIMIT: u32 = 30;
const DEFAULT_QUERY_LIMIT: u32 = 10;

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
        owner: deps
            .api
            .addr_validate(&msg.owner.unwrap_or_else(|| info.sender.to_string()))?,
        fee_distributor: msg.fee_distributor,
        fee_rate: msg.fee_rate,
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    Ok(Response::default()
        .add_attribute("action", "initialization")
        .add_attribute("contract", "p2p-loans"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::DepositCollateral {
            address,
            token_id,
            value,
            terms,
        } => deposit_collateral(deps, env, info, address, token_id, value, terms),
        ExecuteMsg::WithdrawCollateral { loan_id } => withdraw_collateral(deps, env, info, loan_id),

        ExecuteMsg::SetTerms { loan_id, terms } => set_loan_terms(deps, env, info, loan_id, terms),
        ExecuteMsg::AcceptLoan { borrower, loan_id } => {
            accept_loan(deps, env, info, borrower, loan_id)
        }

        ExecuteMsg::AcceptOffer { loan_id, offer_id } => {
            accept_offer(deps, env, info, loan_id, offer_id)
        }
        ExecuteMsg::MakeOffer {
            borrower,
            loan_id,
            terms,
        } => make_offer(deps, env, info, borrower, loan_id, terms),

        ExecuteMsg::CancelOffer {
            borrower,
            loan_id,
            offer_id,
        } => cancel_offer(deps, env, info, borrower, loan_id, offer_id),

        ExecuteMsg::RefuseOffer { loan_id, offer_id } => {
            refuse_offer(deps, env, info, loan_id, offer_id)
        }

        ExecuteMsg::WithdrawRefusedOffer {
            borrower,
            loan_id,
            offer_id,
        } => withdraw_refused_offer(deps, env, info, borrower, loan_id, offer_id),

        ExecuteMsg::RepayBorrowedFunds { loan_id } => {
            repay_borrowed_funds(deps, env, info, loan_id)
        }
        ExecuteMsg::WithdrawDefaultedLoan { borrower, loan_id } => {
            withdraw_defaulted_loan(deps, env, info, borrower, loan_id)
        }

        // Internal Contract Logic
        ExecuteMsg::SetOwner { owner } => set_owner(deps, env, info, owner),

        ExecuteMsg::SetFeeDistributor { fee_depositor } => {
            set_fee_distributor(deps, env, info, fee_depositor)
        }

        ExecuteMsg::SetFeeRate { fee_rate } => set_fee_rate(deps, env, info, fee_rate),

        // Generic (will have to remove at the end of development)
        _ => Err(ContractError::Std(StdError::generic_err(
            "Ow whaou, please wait just a bit, it's not implemented yet !",
        ))),
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
        QueryMsg::ContractInfo {} => to_binary(&query_contract_info(deps)?),
        QueryMsg::CollateralInfo { borrower, loan_id } => {
            to_binary(&query_collateral_info(deps, borrower, loan_id)?)
        }
        QueryMsg::BorrowerInfo { borrower } => to_binary(&query_borrower_info(deps, borrower)?),
    }
}

/// Owner only function
/// Sets a new owner
/// The owner can set the parameters of the contract
/// * Owner
/// * Fee distributor contract
/// * Fee Rate
pub fn set_owner(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_owner: String,
) -> Result<Response, ContractError> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    let new_owner = deps.api.addr_validate(&new_owner)?;
    contract_info.owner = new_owner.clone();
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::default()
        .add_attribute("action", "changed-contract-parameter")
        .add_attribute("parameter", "owner")
        .add_attribute("value", new_owner))
}

/// Owner only function
/// Sets a new fee-distributor contract
/// This contract distributes fees back to the projects (and Illiquidly DAO gets to keep a small amount too)
pub fn set_fee_distributor(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_distributor: String,
) -> Result<Response, ContractError> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    contract_info.fee_distributor = new_distributor.clone();
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::default()
        .add_attribute("action", "changed-contract-parameter")
        .add_attribute("parameter", "fee_distributor")
        .add_attribute("value", new_distributor))
}

/// Owner only function
/// Sets a new fee rate
/// fee_rate is in units of a 1/100_000th, so e.g. if fee_rate=5_000, the fee_rate is 5%
/// It correspond to the part of interests that are kept by the organisation (for redistribution and DAO purposes)
pub fn set_fee_rate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_fee_rate: Uint128,
) -> Result<Response, ContractError> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    contract_info.fee_rate = new_fee_rate;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "changed-contract-parameter")
        .add_attribute("parameter", "fee_rate")
        .add_attribute("value", new_fee_rate))
}

/// Deposit an NFT collateral
/// This is the first entry point of the loan flow.
/// Users deposit their collateral for other users to accept their terms in exchange of interest paid at the end of the loan duration
/// The borrower (the person that deposits collaterals) can specify terms at which they wish to borrow funds against their collateral
/// If terms are specified, fund lenders can accept the loan directly
/// If not, lenders can propose terms than may be accepted by the borrower in return to start the loan
/// This deposit function allows CW721 and CW1155 tokens to be deposited
pub fn deposit_collateral(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address: String,
    token_id: String,
    value: Option<Uint128>,
    terms: Option<LoanTerms>,
) -> Result<Response, ContractError> {
    let borrower = info.sender;

    // We prepare for storing and transfering the token from the borrower to the contract
    // Yes this is custodial, we could change that to make it non-custodial
    // REQUIRED TODO make it non-custodial for the lender
    let (asset_info, transfer_message) = if let Some(value) = value {
        // In case of a Cw1155
        (
            AssetInfo::Cw1155Coin(Cw1155Coin {
                address: address.clone(),
                token_id: token_id.clone(),
                value,
            }),
            into_cosmos_msg(
                Cw1155ExecuteMsg::SendFrom {
                    from: borrower.to_string(),
                    to: env.contract.address.into(),
                    token_id,
                    value,
                    msg: None,
                },
                address,
                None,
            )?,
        )
    } else {
        // In case of a CW721
        (
            AssetInfo::Cw721Coin(Cw721Coin {
                address: address.clone(),
                token_id: token_id.clone(),
            }),
            into_cosmos_msg(
                Cw721ExecuteMsg::TransferNft {
                    recipient: env.contract.address.into(),
                    token_id,
                },
                address,
                None,
            )?,
        )
    };

    // We save the collateral info in our internal structure
    // First we update the number of collateral a user has deposited (to make sure the id assigned is unique)
    let loan_id = BORROWER_INFO
        .update::<_, ContractError>(deps.storage, &borrower.clone(), |x| match x {
            Some(mut info) => {
                info.last_collateral_id += 1;
                Ok(info)
            }
            None => Ok(BorrowerInfo::default()),
        })?
        .last_collateral_id;
    // Then we save an collateral info object
    COLLATERAL_INFO.save(
        deps.storage,
        (&borrower, loan_id),
        &CollateralInfo {
            terms,
            associated_asset: asset_info,
            ..Default::default()
        },
    )?;

    Ok(Response::new()
        .add_message(transfer_message)
        .add_attribute("action", "deposit-collateral")
        .add_attribute("borrower", borrower)
        .add_attribute("loan_id", loan_id.to_string()))
}

/// Withdraw an NFT collateral
/// This simply cancels the potential loan
pub fn withdraw_collateral(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    loan_id: u64,
) -> Result<Response, ContractError> {
    // We query the loan info
    let borrower = info.sender;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id))?;
    is_collateral_withdrawable(&collateral)?;

    // We start by creating the transfer message
    let transfer_message = _withdraw_asset(
        collateral.associated_asset.clone(),
        env.contract.address,
        borrower.clone(),
    )?;

    // We update the internal state, the loan proposal is no longer valid
    collateral.state = LoanState::AssetWithdrawn;
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id), &collateral)?;

    Ok(Response::new()
        .add_message(transfer_message)
        .add_attribute("action", "withdraw-collateral")
        .add_attribute("event", "cancel-loan")
        .add_attribute("borrower", borrower)
        .add_attribute("loan_id", loan_id.to_string()))
}

/// Change the loan terms of a loan before it's accepted by anyone
/// Or just add some terms because you didn't have the chance before
/// If you want to update the terms of your collateral, because no-one wanted to accept it or because the market changed
pub fn set_loan_terms(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    loan_id: u64,
    terms: LoanTerms,
) -> Result<Response, ContractError> {
    // We query the loan info
    let borrower = info.sender;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id))?;
    is_loan_modifiable(&collateral)?;

    // Update the terms
    collateral.terms = Some(terms);
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id), &collateral)?;

    Ok(Response::new()
        .add_attribute("action", "modify-loan_terms")
        .add_attribute("borrower", borrower)
        .add_attribute("loan_id", loan_id.to_string()))
}

/// Make an offer (offer some terms) to lend some money against someone's collateral
/// The borrower will then be able to accept those terms if they please them
pub fn make_offer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
    terms: LoanTerms,
) -> Result<Response, ContractError> {
    // We query the loan info
    let borrower = deps.api.addr_validate(&borrower)?;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id))?;
    is_loan_counterable(&collateral)?;

    // Make sure the transaction contains funds that match the principle indicated in the terms
    if info.funds.len() != 1 {
        return Err(ContractError::MultipleCoins {});
    } else if terms.principle != info.funds[0].clone() {
        return Err(ContractError::FundsDontMatchTerms {});
    }

    let offer_id = add_new_offer(
        deps.storage,
        &mut collateral,
        (borrower.clone(), loan_id),
        OfferInfo {
            lender: info.sender.clone(),
            terms,
            state: OfferState::Published,
            deposited_funds: Some(info.funds[0].clone()),
        },
    )?;

    Ok(Response::new()
        .add_attribute("action", "make-offer")
        .add_attribute("borrower", borrower)
        .add_attribute("lender", info.sender)
        .add_attribute("loan_id", loan_id.to_string())
        .add_attribute("offer_id", offer_id.to_string()))
}

/// Cancel an offer you made in case the market changes or whatever
/// The borrower won't be able to accept the loan if you cancel it
pub fn cancel_offer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
    offer_id: u64,
) -> Result<Response, ContractError> {
    let lender = info.sender;
    // We query the loan info
    let borrower = deps.api.addr_validate(&borrower)?;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id))?;

    // We can cancel an offer only if the Borrower is still searching for a loan
    if collateral.state != LoanState::Published {
        return Err(ContractError::Unauthorized {});
    }
    // We need to verify the offer exists and it belongs to the address calling the contract and that's in the right state to be cancelled
    let mut offer = is_lender(lender.clone(), &collateral, offer_id as usize)?;
    if offer.state != OfferState::Published {
        return Err(ContractError::CantChangeOfferState {
            from: offer.state,
            to: OfferState::Cancelled,
        });
    }

    // The funds deposited for lending are withdrawn
    let withdraw_response = _withdraw_offer_unsafe(
        deps.as_ref(),
        borrower.clone(),
        lender.clone(),
        loan_id,
        offer_id as usize,
    )?;

    // We save the changes in the collateral object
    offer.state = OfferState::Cancelled;
    collateral.offers[offer_id as usize] = offer;
    // And mark the deposited funds as withdrawn
    collateral.offers[offer_id as usize].deposited_funds = None;
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id), &collateral)?;

    Ok(Response::new()
        .add_message(withdraw_response)
        .add_attribute("action", "cancel-offer")
        .add_attribute("action", "withdraw-funds")
        .add_attribute("borrower", borrower)
        .add_attribute("lender", lender)
        .add_attribute("loan_id", loan_id.to_string())
        .add_attribute("offer_id", offer_id.to_string()))
}

/// Withdraw the funds from a refused offer
/// In case the borrower refuses your offer, you need to manually withdraw your funds
/// This is actually done in order for you to know where your funds are and keep control of your transfers
pub fn withdraw_refused_offer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
    offer_id: u64,
) -> Result<Response, ContractError> {
    let lender = info.sender;
    // We query the loan info
    let borrower = deps.api.addr_validate(&borrower)?;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id))?;

    // We need to verify the offer exists and the sender is actually the owner of the offer
    let offer = is_lender(lender.clone(), &collateral, offer_id as usize)?;

    // TODO, please verify this shit right there
    if offer.state != OfferState::Refused {
        return Err(ContractError::NotWithdrawable {});
    }

    // The funds deposited for lending are withdrawn
    let withdraw_message = _withdraw_offer_unsafe(
        deps.as_ref(),
        borrower.clone(),
        lender.clone(),
        loan_id,
        offer_id as usize,
    )?;

    // And we mark the deposited funds as withdrawn
    collateral.offers[offer_id as usize].deposited_funds = None;
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id), &collateral)?;

    Ok(Response::new()
        .add_message(withdraw_message)
        .add_attribute("action", "withdraw-funds")
        .add_attribute("event", "refused-offer")
        .add_attribute("borrower", borrower)
        .add_attribute("lender", lender)
        .add_attribute("loan_id", loan_id.to_string())
        .add_attribute("offer_id", offer_id.to_string()))
}

/// This creates withdraw messages to withdraw the funds from an offer (to the lender of the borrower depending on the situation
/// This function does not do any checks on the validity of the procedure
/// Be careful when using this internal function
pub fn _withdraw_offer_unsafe(
    deps: Deps,
    borrower: Addr,
    recipient: Addr,
    loan_id: u64,
    offer_id: usize,
) -> Result<BankMsg, ContractError> {
    // We query the loan info
    let collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id))?;
    let offer = get_offer(&collateral, offer_id)?;

    // We get the funds to withdraw
    let funds_to_withdraw = offer
        .deposited_funds
        .ok_or(ContractError::NoFundsToWithdraw {})?;

    Ok(BankMsg::Send {
        to_address: recipient.to_string(),
        amount: vec![funds_to_withdraw],
    })
}

/// Refuse an offer to a borrowers collateral
/// This is needed only for printing and db procedure, and not actually needed in the flow
/// This however blocks other interactions with the offer (except withdrawing the funds)
pub fn refuse_offer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    loan_id: u64,
    offer_id: u64,
) -> Result<Response, ContractError> {
    // We query the loan info
    let borrower = info.sender;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id))?;

    // Mark the offer as refused
    let mut offer = get_offer(&collateral, offer_id as usize)?;
    offer.state = OfferState::Refused;
    collateral.offers[offer_id as usize] = offer.clone();

    // And save the changes to the collateral object
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id), &collateral)?;

    Ok(Response::new()
        .add_attribute("action", "refuse-offer")
        .add_attribute("borrower", borrower)
        .add_attribute("lender", offer.lender)
        .add_attribute("loan_id", loan_id.to_string())
        .add_attribute("offer_id", offer_id.to_string()))
}

/// Accept a loan and its terms directly
/// As soon as the lender executes this messages, the loan starts and the borrower will need to repay the loan before the term
pub fn accept_loan(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
) -> Result<Response, ContractError> {
    // We query the loan info
    let borrower = deps.api.addr_validate(&borrower)?;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id))?;

    // We verify the loan is acceptable as is
    is_loan_acceptable(&collateral)?;
    let terms: LoanTerms = collateral
        .terms
        .clone()
        .ok_or(ContractError::NoTermsSpecified {})?;

    // We verify the funds received from the lender
    if info.funds.len() != 1 {
        return Err(ContractError::MultipleCoins {});
    } else if terms.principle != info.funds[0].clone() {
        return Err(ContractError::FundsDontMatchTerms {});
    }

    // Then we can save the original offer as accepted
    collateral.state = LoanState::Started;
    collateral.start_block = Some(env.block.height);

    // We add this offer at the end of the list of offers.
    // All other offers are marked as refused automatically (see `get_offer` in state.rs)
    let offer_id = add_new_offer(
        deps.storage,
        &mut collateral,
        (borrower.clone(), loan_id),
        OfferInfo {
            lender: info.sender.clone(),
            terms: terms.clone(),
            state: OfferState::Accepted,
            deposited_funds: Some(info.funds[0].clone()),
        },
    )?;
    // We update the active loan variable
    collateral.active_loan = Some(offer_id);
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id), &collateral)?;

    // We withdraw funds to the borrower
    let message = _withdraw_offer_unsafe(
        deps.as_ref(),
        borrower.clone(),
        borrower.clone(),
        loan_id,
        offer_id as usize,
    )?;

    Ok(Response::new()
        .add_message(message)
        .add_attribute("action", "start-loan")
        .add_attribute("denom-borrowed", terms.principle.denom)
        .add_attribute("amount_borrowed", terms.principle.amount.to_string())
        .add_attribute("borrower", borrower)
        .add_attribute("lender", info.sender)
        .add_attribute("loan_id", loan_id.to_string())
        .add_attribute("offer_id", offer_id.to_string()))
}

/// Accept an offer someone made for your collateral
/// As soon as the borrower executes this messages, the loan starts and the they will need to repay the loan before the term
pub fn accept_offer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    loan_id: u64,
    offer_id: u64,
) -> Result<Response, ContractError> {
    // We query the loan info
    let borrower = info.sender;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id))?;
    is_loan_acceptable(&collateral)?;
    let offer_id_usize = offer_id as usize;
    let mut offer = get_offer(&collateral, offer_id_usize)?;

    // We verify the offer is still valid
    if offer.state == OfferState::Published {
        // We can start the loan right away !
        collateral.state = LoanState::Started;
        collateral.start_block = Some(env.block.height);
        collateral.active_loan = Some(offer_id);
        offer.state = OfferState::Accepted;
        collateral.offers[offer_id_usize] = offer.clone();

        COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id), &collateral)?;
    } else {
        return Err(ContractError::OfferNotFound {});
    };

    // We transfer the funds directly when the offer is accepted
    let message = _withdraw_offer_unsafe(
        deps.as_ref(),
        borrower.clone(),
        borrower.clone(),
        loan_id,
        offer_id as usize,
    )?;

    Ok(Response::new()
        .add_message(message)
        .add_attribute("action", "start-loan")
        .add_attribute("denom-borrowed", offer.terms.principle.denom)
        .add_attribute("amount_borrowed", offer.terms.principle.amount.to_string())
        .add_attribute("borrower", borrower)
        .add_attribute("lender", offer.lender)
        .add_attribute("loan_id", loan_id.to_string())
        .add_attribute("offer_id", offer_id.to_string()))
}

/// Repay Borrowed funds and get back your collateral
/// This function receives principle + interest funds to end the loan and unlock the collateral
pub fn repay_borrowed_funds(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    loan_id: u64,
) -> Result<Response, ContractError> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    // We query the loan info
    let borrower = info.sender;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id))?;
    can_repay_loan(env.clone(), &collateral)?;
    let offer = get_active_loan(&collateral)?;

    // We verify the sent funds correspond to the principle + interests
    let interests = offer.terms.interest;
    if info.funds.len() != 1 {
        return Err(ContractError::MultipleCoins {});
    } else if offer.terms.principle.denom != info.funds[0].denom.clone() {
        return Err(ContractError::Std(StdError::generic_err(
            "You didn't send the right kind of funds",
        )));
    } else if offer.terms.principle.amount + interests > info.funds[0].amount {
        return Err(ContractError::Std(StdError::generic_err(
            format!(
                "Fund sent do not match the loan terms (principle + interests). Needed : {needed}, Received : {received}", 
                needed = offer.terms.principle.amount + interests,
                received = info.funds[0].amount.clone()
            )
        )));
    }

    // We save the collateral state
    collateral.state = LoanState::Ended;
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id), &collateral)?;

    // We prepare the funds to send back to the lender
    let lender_payback = offer.terms.principle.amount
        + interests * (Uint128::new(100_000u128) - contract_info.fee_rate)
            / Uint128::new(100_000u128);

    // And the funds to send to the fee_depositor contract
    let fee_depositor_payback = info.funds[0].amount - lender_payback;

    // The fee depositor needs to know which assets where involved in the transaction
    let collateral_address = match &collateral.associated_asset {
        AssetInfo::Cw1155Coin(cw1155) => cw1155.address.clone(),
        AssetInfo::Cw721Coin(cw721) => cw721.address.clone(),
        _ => {
            return Err(ContractError::Std(StdError::generic_err(
                "Unreachable error",
            )))
        }
    };

    Ok(Response::new()
        // We get the funds back to the lender
        .add_message(BankMsg::Send {
            to_address: offer.lender.to_string(),
            amount: coins(lender_payback.u128(), info.funds[0].denom.clone()),
        })
        // And the collateral back to the borrower
        .add_message(_withdraw_asset(
            collateral.associated_asset.clone(),
            env.contract.address,
            borrower.clone(),
        )?)
        // And we pay the fee to the treasury
        .add_message(into_cosmos_msg(
            FeeDistributorMsg::DepositFees {
                addresses: vec![collateral_address],
                fee_type: FeeType::Funds
            },
            contract_info.fee_distributor,
            Some(coins(
                fee_depositor_payback.u128(),
                info.funds[0].denom.clone(),
            )),
        )?)
        .add_attribute("action", "repay-loan")
        .add_attribute("borrower", borrower)
        .add_attribute("lender", offer.lender)
        .add_attribute("loan_id", loan_id.to_string()))
}

/// Withdraw the collateral from a defaulted loan
/// If the loan duration has exceeded, the collateral can be withdrawn by the lender
/// This closes the loan and puts it in a defaulted state
pub fn withdraw_defaulted_loan(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
) -> Result<Response, ContractError> {
    // We query the loan info
    let borrower = deps.api.addr_validate(&borrower)?;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id))?;
    is_loan_defaulted(env.clone(), &collateral)?;
    let offer = is_active_lender(info.sender, &collateral)?;

    // We need to test if the loan hasn't already been defaulted
    if collateral.state == LoanState::Defaulted {
        return Err(ContractError::LoanAlreadyDefaulted {});
    }

    // Saving the collateral state, the loan is defaulted, we can default it again
    collateral.state = LoanState::Defaulted;
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id), &collateral)?;

    // We create the collateral withdrawal message
    let withdraw_message = _withdraw_asset(
        collateral.associated_asset.clone(),
        env.contract.address,
        offer.lender.clone(),
    )?;

    Ok(Response::new()
        .add_message(withdraw_message)
        .add_attribute("action", "default-loan")
        .add_attribute("borrower", borrower)
        .add_attribute("lender", offer.lender)
        .add_attribute("loan_id", loan_id.to_string()))
}

pub fn _withdraw_asset(asset: AssetInfo, sender: Addr, recipient: Addr) -> StdResult<CosmosMsg> {
    match asset {
        AssetInfo::Cw1155Coin(cw1155) => {
            let address = cw1155.address;
            let token_id = cw1155.token_id;
            into_cosmos_msg(
                Cw1155ExecuteMsg::SendFrom {
                    from: sender.to_string(),
                    to: recipient.to_string(),
                    token_id,
                    value: cw1155.value,
                    msg: None,
                },
                address,
                None,
            )
        }

        AssetInfo::Cw721Coin(cw721) => {
            let address = cw721.address;
            let token_id = cw721.token_id;
            into_cosmos_msg(
                Cw721ExecuteMsg::TransferNft {
                    recipient: recipient.to_string(),
                    token_id,
                },
                address,
                None,
            )
        }
        _ => Err(StdError::generic_err("Unreachable error")),
    }
}

// TODO we need more queries, to query loan by user
pub fn query_contract_info(deps: Deps) -> StdResult<ContractInfo> {
    CONTRACT_INFO.load(deps.storage)
}

pub fn query_collateral_info(
    deps: Deps,
    borrower: String,
    loan_id: u64,
) -> StdResult<CollateralInfo> {
    let borrower = deps.api.addr_validate(&borrower)?;
    COLLATERAL_INFO
        .load(deps.storage, (&borrower, loan_id))
        .map_err(|_| StdError::generic_err("LoanNotFound"))
}

pub fn query_offer_info(
    deps: Deps,
    borrower: String,
    loan_id: u64,
    offer_id: u64,
) -> StdResult<OfferInfo> {
    let borrower = deps.api.addr_validate(&borrower)?;
    let collateral = COLLATERAL_INFO
        .load(deps.storage, (&borrower, loan_id))
        .map_err(|_| StdError::generic_err("LoanNotFound"))?;

    get_offer(&collateral, offer_id as usize).map_err(|_| StdError::generic_err("OfferNotFound"))
}

pub fn query_borrower_info(deps: Deps, borrower: String) -> StdResult<BorrowerInfo> {
    let borrower = deps.api.addr_validate(&borrower)?;
    BORROWER_INFO
        .load(deps.storage, &borrower)
        .map_err(|_| StdError::generic_err("UnknownBorrower"))
}

pub fn query_collaterals(
    deps: Deps,
    borrower: String,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<Vec<CollateralResponse>> {
    let borrower = deps.api.addr_validate(&borrower)?;
    let limit = limit.unwrap_or(DEFAULT_QUERY_LIMIT).min(MAX_QUERY_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    COLLATERAL_INFO
        .prefix(&borrower)
        .range(deps.storage, None, start, Order::Descending)
        .map(|result| {
            result.map(|(loan_id, el)| CollateralResponse {
                borrower: borrower.to_string(),
                loan_id,
                collateral: el,
            })
        })
        .take(limit)
        .collect()
}

pub fn query_offers(
    deps: Deps,
    lender: String,
    start_after: Option<u32>,
    limit: Option<u32>,
) -> StdResult<Vec<OfferResponse>> {
    let lender = deps.api.addr_validate(&lender)?;
    let limit = limit.unwrap_or(DEFAULT_QUERY_LIMIT).min(MAX_QUERY_LIMIT) as usize;
    let start = start_after.unwrap_or(0u32) as usize;

    Ok(LENDER_OFFERS
        .load(deps.storage, &lender)
        .unwrap_or_default()
        .iter()
        .skip(start)
        .map(|x| OfferResponse {
            lender: lender.to_string(),
            borrower: x.0.to_string(),
            loan_id: x.1,
            offer_id: x.2,
        })
        .take(limit)
        .collect())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use cosmwasm_std::{
        coin, coins,
        testing::{mock_dependencies, mock_env, mock_info},
        Api, Coin, SubMsg,
    };

    fn init_helper(deps: DepsMut) {
        let instantiate_msg = InstantiateMsg {
            name: "nft-loan".to_string(),
            owner: None,
            fee_distributor: "T".to_string(),
            fee_rate: Uint128::new(5_000u128),
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();

        instantiate(deps, env, info, instantiate_msg).unwrap();
    }

    #[test]
    fn test_init_sanity() {
        let mut deps = mock_dependencies();
        let instantiate_msg = InstantiateMsg {
            name: "p2p-trading".to_string(),
            owner: Some("this_address".to_string()),
            fee_distributor: "T".to_string(),
            fee_rate: Uint128::new(5_000u128),
        };
        let info = mock_info("owner", &[]);
        let env = mock_env();

        let res_init = instantiate(deps.as_mut(), env.clone(), info, instantiate_msg).unwrap();
        assert_eq!(0, res_init.messages.len());

        let contract = CONTRACT_INFO.load(&deps.storage).unwrap();
        assert_eq!(
            contract,
            ContractInfo {
                name: "p2p-trading".to_string(),
                owner: deps.api.addr_validate("this_address").unwrap(),
                fee_distributor: "T".to_string(),
                fee_rate: Uint128::new(5_000u128),
            }
        );

        let info = mock_info("this_address", &[]);
        let bad_info = mock_info("bad_person", &[]);
        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::SetFeeDistributor {
                fee_depositor: "S".to_string(),
            },
        )
        .unwrap();
        assert_eq!(
            CONTRACT_INFO.load(&deps.storage).unwrap().fee_distributor,
            "S".to_string()
        );

        let unauthorized = execute(
            deps.as_mut(),
            env.clone(),
            bad_info.clone(),
            ExecuteMsg::SetFeeDistributor {
                fee_depositor: "S".to_string(),
            },
        )
        .unwrap_err();
        assert_eq!(unauthorized, ContractError::Unauthorized {});

        // We test changing the owner
        let unauthorized = execute(
            deps.as_mut(),
            env.clone(),
            bad_info.clone(),
            ExecuteMsg::SetOwner {
                owner: "new_owner".to_string(),
            },
        )
        .unwrap_err();
        assert_eq!(unauthorized, ContractError::Unauthorized {});

        // We test changing the owner
        execute(
            deps.as_mut(),
            env.clone(),
            info,
            ExecuteMsg::SetOwner {
                owner: "new_owner".to_string(),
            },
        )
        .unwrap();
        assert_eq!(
            CONTRACT_INFO.load(&deps.storage).unwrap().owner,
            "new_owner".to_string()
        );

        let info = mock_info("new_owner", &[]);

        let unauthorized = execute(
            deps.as_mut(),
            env.clone(),
            bad_info,
            ExecuteMsg::SetFeeRate {
                fee_rate: Uint128::new(500u128),
            },
        )
        .unwrap_err();
        assert_eq!(unauthorized, ContractError::Unauthorized {});

        execute(
            deps.as_mut(),
            env,
            info,
            ExecuteMsg::SetFeeRate {
                fee_rate: Uint128::new(500u128),
            },
        )
        .unwrap();
        assert_eq!(
            CONTRACT_INFO.load(&deps.storage).unwrap().fee_rate,
            Uint128::new(500u128)
        );
    }

    fn add_collateral_helper(
        deps: DepsMut,
        creator: &str,
        address: &str,
        token_id: &str,
        value: Option<Uint128>,
        terms: Option<LoanTerms>,
    ) -> Result<Response, ContractError> {
        let info = mock_info(creator, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::DepositCollateral {
                address: address.to_string(),
                token_id: token_id.to_string(),
                value,
                terms,
            },
        )
    }

    fn set_terms_helper(
        deps: DepsMut,
        borrower: &str,
        loan_id: u64,
        terms: LoanTerms,
    ) -> Result<Response, ContractError> {
        let info = mock_info(borrower, &[]);
        let env = mock_env();

        execute(deps, env, info, ExecuteMsg::SetTerms { loan_id, terms })
    }

    fn make_offer_helper(
        deps: DepsMut,
        lender: &str,
        borrower: &str,
        loan_id: u64,
        terms: LoanTerms,
        coins: Vec<Coin>,
    ) -> Result<Response, ContractError> {
        let info = mock_info(lender, &coins);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::MakeOffer {
                borrower: borrower.to_string(),
                loan_id,
                terms,
            },
        )
    }

    fn cancel_offer_helper(
        deps: DepsMut,
        lender: &str,
        borrower: &str,
        loan_id: u64,
        offer_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(lender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::CancelOffer {
                borrower: borrower.to_string(),
                loan_id,
                offer_id,
            },
        )
    }

    fn refuse_offer_helper(
        deps: DepsMut,
        borrower: &str,
        loan_id: u64,
        offer_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(borrower, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::RefuseOffer { loan_id, offer_id },
        )
    }

    fn accept_loan_helper(
        deps: DepsMut,
        lender: &str,
        borrower: &str,
        loan_id: u64,
        coins: Vec<Coin>,
    ) -> Result<Response, ContractError> {
        let info = mock_info(lender, &coins);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::AcceptLoan {
                borrower: borrower.to_string(),
                loan_id,
            },
        )
    }

    fn accept_offer_helper(
        deps: DepsMut,
        borrower: &str,
        loan_id: u64,
        offer_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(borrower, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::AcceptOffer { loan_id, offer_id },
        )
    }

    fn withdraw_collateral_helper(
        deps: DepsMut,
        creator: &str,
        loan_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(creator, &[]);
        let env = mock_env();

        execute(deps, env, info, ExecuteMsg::WithdrawCollateral { loan_id })
    }

    fn withdraw_refused_offer_helper(
        deps: DepsMut,
        lender: &str,
        borrower: &str,
        loan_id: u64,
        offer_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(lender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::WithdrawRefusedOffer {
                borrower: borrower.to_string(),
                loan_id,
                offer_id,
            },
        )
    }
    fn repay_borrowed_funds_helper(
        deps: DepsMut,
        borrower: &str,
        loan_id: u64,
        funds: Vec<Coin>,
        env: Env,
    ) -> Result<Response, ContractError> {
        let info = mock_info(borrower, &funds);

        execute(deps, env, info, ExecuteMsg::RepayBorrowedFunds { loan_id })
    }
    fn withdraw_defaulted_loan_helper(
        deps: DepsMut,
        lender: &str,
        borrower: &str,
        loan_id: u64,
        env: Env,
    ) -> Result<Response, ContractError> {
        let info = mock_info(lender, &[]);

        execute(
            deps,
            env,
            info,
            ExecuteMsg::WithdrawDefaultedLoan {
                borrower: borrower.to_string(),
                loan_id,
            },
        )
    }

    #[test]
    fn test_add_collateral() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        // We make sure the collateral is deposited correctly
        let res = add_collateral_helper(deps.as_mut(), "creator", "nft", "58", None, None).unwrap();
        assert_eq!(1, res.messages.len());

        // Other collaterals
        add_collateral_helper(deps.as_mut(), "creator", "nft", "59", None, None).unwrap();
        add_collateral_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "59",
            Some(Uint128::from(459u128)),
            None,
        )
        .unwrap();

        let creator_addr = deps.api.addr_validate("creator").unwrap();
        let coll_info = COLLATERAL_INFO
            .load(&deps.storage, (&creator_addr, 0))
            .unwrap();
        assert_eq!(
            coll_info,
            CollateralInfo {
                associated_asset: AssetInfo::Cw721Coin(Cw721Coin {
                    address: "nft".to_string(),
                    token_id: "58".to_string()
                }),
                ..Default::default()
            }
        );

        let coll_info = COLLATERAL_INFO
            .load(&deps.storage, (&creator_addr, 1))
            .unwrap();
        assert_eq!(
            coll_info,
            CollateralInfo {
                associated_asset: AssetInfo::Cw721Coin(Cw721Coin {
                    address: "nft".to_string(),
                    token_id: "59".to_string()
                }),
                ..Default::default()
            }
        );

        let coll_info = COLLATERAL_INFO
            .load(&deps.storage, (&creator_addr, 2))
            .unwrap();
        assert_eq!(
            coll_info,
            CollateralInfo {
                associated_asset: AssetInfo::Cw1155Coin(Cw1155Coin {
                    address: "nft".to_string(),
                    token_id: "59".to_string(),
                    value: Uint128::from(459u128)
                }),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_withdraw_collateral() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        add_collateral_helper(deps.as_mut(), "creator", "nft", "58", None, None).unwrap();
        add_collateral_helper(deps.as_mut(), "creator", "nft", "59", None, None).unwrap();
        add_collateral_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "59",
            Some(Uint128::from(459u128)),
            None,
        )
        .unwrap();

        withdraw_collateral_helper(deps.as_mut(), "creator", 1).unwrap();
        withdraw_collateral_helper(deps.as_mut(), "creator", 0).unwrap();

        let creator_addr = deps.api.addr_validate("creator").unwrap();
        let coll_info = COLLATERAL_INFO
            .load(&deps.storage, (&creator_addr, 0))
            .unwrap();
        assert_eq!(
            coll_info,
            CollateralInfo {
                associated_asset: AssetInfo::Cw721Coin(Cw721Coin {
                    address: "nft".to_string(),
                    token_id: "58".to_string()
                }),
                state: LoanState::AssetWithdrawn,
                ..Default::default()
            }
        );

        let coll_info = COLLATERAL_INFO
            .load(&deps.storage, (&creator_addr, 1))
            .unwrap();
        assert_eq!(
            coll_info,
            CollateralInfo {
                associated_asset: AssetInfo::Cw721Coin(Cw721Coin {
                    address: "nft".to_string(),
                    token_id: "59".to_string()
                }),
                state: LoanState::AssetWithdrawn,
                ..Default::default()
            }
        );

        let coll_info = COLLATERAL_INFO
            .load(&deps.storage, (&creator_addr, 2))
            .unwrap();
        assert_eq!(
            coll_info,
            CollateralInfo {
                terms: None,
                associated_asset: AssetInfo::Cw1155Coin(Cw1155Coin {
                    address: "nft".to_string(),
                    token_id: "59".to_string(),
                    value: Uint128::from(459u128)
                }),
                ..Default::default()
            }
        );
        // You shouldn't be able to repay the loan now
        let repay_err = repay_borrowed_funds_helper(
            deps.as_mut(),
            "creator",
            0,
            coins(506, "luna"),
            mock_env(),
        )
        .unwrap_err();
        assert_eq!(
            repay_err,
            ContractError::WrongLoanState {
                state: LoanState::AssetWithdrawn
            }
        )
    }

    #[test]
    fn test_accept_loan() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        add_collateral_helper(deps.as_mut(), "creator", "nft", "58", None, None).unwrap();
        add_collateral_helper(deps.as_mut(), "creator", "nft", "59", None, None).unwrap();
        add_collateral_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "59",
            Some(Uint128::from(459u128)),
            None,
        )
        .unwrap();

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(0),
            duration_in_blocks: 0,
        };
        set_terms_helper(deps.as_mut(), "creator", 0, terms.clone()).unwrap();

        // The funds have to match the terms
        let err = accept_loan_helper(deps.as_mut(), "anyone", "creator", 0, coins(123, "luna"))
            .unwrap_err();
        assert_eq!(err, ContractError::FundsDontMatchTerms {});
        let err = accept_loan_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            vec![coin(123, "luna"), coin(457, "uusd")],
        )
        .unwrap_err();
        assert_eq!(err, ContractError::MultipleCoins {});

        accept_loan_helper(deps.as_mut(), "anyone", "creator", 0, coins(456, "luna")).unwrap();
        accept_loan_helper(
            deps.as_mut(),
            "anyone_else",
            "creator",
            0,
            coins(456, "luna"),
        )
        .unwrap_err();
        let creator_addr = deps.api.addr_validate("creator").unwrap();
        let coll_info = COLLATERAL_INFO
            .load(&deps.storage, (&creator_addr, 0))
            .unwrap();

        assert_eq!(
            coll_info,
            CollateralInfo {
                terms: Some(terms.clone()),
                associated_asset: AssetInfo::Cw721Coin(Cw721Coin {
                    address: "nft".to_string(),
                    token_id: "58".to_string()
                }),
                state: LoanState::Started,
                active_loan: Some(0),
                start_block: Some(12345),
                offers: vec![OfferInfo {
                    lender: deps.api.addr_validate("anyone").unwrap(),
                    terms,
                    state: OfferState::Accepted,
                    deposited_funds: Some(coin(456, "luna")),
                }]
            }
        );
    }

    #[test]
    fn test_accept_loan_and_modify() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(0),
            duration_in_blocks: 0,
        };
        add_collateral_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "58",
            Some(Uint128::from(8_u128)),
            Some(terms.clone()),
        )
        .unwrap();
        accept_loan_helper(deps.as_mut(), "anyone", "creator", 0, coins(456, "luna")).unwrap();

        // We try to modify the loan
        let modify_err = set_terms_helper(deps.as_mut(), "creator", 0, terms.clone()).unwrap_err();
        assert_eq!(modify_err, ContractError::NotModifiable {});

        // We try to counter the loan, and propose new terms
        let offer_err = make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms,
            coins(456, "luna"),
        )
        .unwrap_err();

        assert_eq!(offer_err, ContractError::NotCounterable {});
    }

    #[test]
    fn test_repay_loan_early() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(0),
            duration_in_blocks: 0,
        };
        add_collateral_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "58",
            Some(Uint128::from(8_u128)),
            Some(terms),
        )
        .unwrap();
        let repay_err = repay_borrowed_funds_helper(
            deps.as_mut(),
            "creator",
            0,
            coins(506, "luna"),
            mock_env(),
        )
        .unwrap_err();
        assert_eq!(
            repay_err,
            ContractError::WrongLoanState {
                state: LoanState::Published
            }
        )
    }

    #[test]
    fn test_make_offer() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        add_collateral_helper(deps.as_mut(), "creator", "nft", "58", None, None).unwrap();
        add_collateral_helper(deps.as_mut(), "creator", "nft", "59", None, None).unwrap();
        add_collateral_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "59",
            Some(Uint128::from(459u128)),
            None,
        )
        .unwrap();

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(0),
            duration_in_blocks: 0,
        };

        let err = make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms.clone(),
            coins(6765, "luna"),
        )
        .unwrap_err();
        assert_eq!(err, ContractError::FundsDontMatchTerms {});

        let err = make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms.clone(),
            vec![coin(456, "luna"), coin(456, "luna")],
        )
        .unwrap_err();
        assert_eq!(err, ContractError::MultipleCoins {});

        make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms,
            coins(456, "luna"),
        )
        .unwrap();
    }

    #[test]
    fn test_cancel_offer() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        add_collateral_helper(deps.as_mut(), "creator", "nft", "58", None, None).unwrap();
        add_collateral_helper(deps.as_mut(), "creator", "nft", "59", None, None).unwrap();
        add_collateral_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "59",
            Some(Uint128::from(459u128)),
            None,
        )
        .unwrap();

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(0),
            duration_in_blocks: 0,
        };

        make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms,
            coins(456, "luna"),
        )
        .unwrap();

        cancel_offer_helper(deps.as_mut(), "anyone_else", "creator", 0, 0).unwrap_err();

        let res = cancel_offer_helper(deps.as_mut(), "anyone", "creator", 0, 0).unwrap();

        assert_eq!(
            res.messages,
            vec![SubMsg::new(BankMsg::Send {
                to_address: "anyone".to_string(),
                amount: coins(456, "luna"),
            }),]
        );

        cancel_offer_helper(deps.as_mut(), "anyone", "creator", 0, 0).unwrap_err();
    }

    #[test]
    fn test_refuse_offer() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        add_collateral_helper(deps.as_mut(), "creator", "nft", "58", None, None).unwrap();
        add_collateral_helper(deps.as_mut(), "creator", "nft", "59", None, None).unwrap();
        add_collateral_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "59",
            Some(Uint128::from(459u128)),
            None,
        )
        .unwrap();

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(0),
            duration_in_blocks: 0,
        };

        make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms,
            coins(456, "luna"),
        )
        .unwrap();

        refuse_offer_helper(deps.as_mut(), "bad_person", 0, 0).unwrap_err();
        refuse_offer_helper(deps.as_mut(), "creator", 0, 0).unwrap();

        let offer = COLLATERAL_INFO
            .load(
                &deps.storage,
                (&deps.api.addr_validate("creator").unwrap(), 0u64),
            )
            .unwrap()
            .offers[0]
            .clone();

        assert_eq!(offer.state, OfferState::Refused);
    }

    #[test]
    fn test_cancel_accepted() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(0),
            duration_in_blocks: 0,
        };

        add_collateral_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "58",
            Some(Uint128::new(45u128)),
            Some(terms),
        )
        .unwrap();

        accept_loan_helper(deps.as_mut(), "anyone", "creator", 0, coins(456, "luna")).unwrap();

        withdraw_collateral_helper(deps.as_mut(), "creator", 0).unwrap_err();
        cancel_offer_helper(deps.as_mut(), "anyone", "creator", 0, 0).unwrap_err();
    }

    #[test]
    fn test_withdraw_refused() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(0),
            duration_in_blocks: 0,
        };

        add_collateral_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "58",
            Some(Uint128::new(45u128)),
            Some(terms.clone()),
        )
        .unwrap();
        make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms.clone(),
            coins(456, "luna"),
        )
        .unwrap();

        make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms,
            coins(456, "luna"),
        )
        .unwrap();

        withdraw_refused_offer_helper(deps.as_mut(), "anyone", "creator", 0, 0).unwrap_err();
        withdraw_refused_offer_helper(deps.as_mut(), "anyone", "creator", 0, 1).unwrap_err();
        let err =
            withdraw_refused_offer_helper(deps.as_mut(), "anyone", "creator", 0, 2).unwrap_err();
        assert_eq!(err, ContractError::OfferNotFound {});

        let err = accept_offer_helper(deps.as_mut(), "creator", 0, 87).unwrap_err();
        assert_eq!(err, ContractError::OfferNotFound {});
        accept_offer_helper(deps.as_mut(), "creator", 0, 0).unwrap();

        withdraw_refused_offer_helper(deps.as_mut(), "anyone", "creator", 0, 0).unwrap_err();
        withdraw_refused_offer_helper(deps.as_mut(), "anyone_else", "creator", 0, 1).unwrap_err();
        withdraw_refused_offer_helper(deps.as_mut(), "anyone", "creator", 0, 1).unwrap();
        withdraw_refused_offer_helper(deps.as_mut(), "anyone", "creator", 0, 1).unwrap_err();
    }
    #[test]
    fn test_accept_cancelled_offer() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        add_collateral_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "58",
            Some(Uint128::new(45u128)),
            None,
        )
        .unwrap();

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(0),
            duration_in_blocks: 0,
        };

        make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms,
            coins(456, "luna"),
        )
        .unwrap();

        cancel_offer_helper(deps.as_mut(), "anyone", "creator", 0, 0).unwrap();
        let err = accept_offer_helper(deps.as_mut(), "creator", 0, 0).unwrap_err();
        assert_eq!(err, ContractError::OfferNotFound {})
    }

    #[test]
    fn test_normal_flow() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(50),
            duration_in_blocks: 1,
        };

        add_collateral_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "58",
            Some(Uint128::new(45u128)),
            Some(terms.clone()),
        )
        .unwrap();
        make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms,
            coins(456, "luna"),
        )
        .unwrap();

        accept_offer_helper(deps.as_mut(), "creator", 0, 0).unwrap();
        // Loan starts

        let env = mock_env();
        let not_now_err =
            withdraw_defaulted_loan_helper(deps.as_mut(), "anyone", "creator", 0, env.clone())
                .unwrap_err();
        assert_eq!(
            not_now_err,
            ContractError::WrongLoanState {
                state: LoanState::Started
            }
        );

        let err = repay_borrowed_funds_helper(
            deps.as_mut(),
            "creator",
            0,
            coins(456, "luna"),
            env.clone(),
        )
        .unwrap_err();
        assert_eq!(err, ContractError::Std(
            StdError::generic_err("Fund sent do not match the loan terms (principle + interests). Needed : 506, Received : 456")
        ));
        let err = repay_borrowed_funds_helper(
            deps.as_mut(),
            "creator",
            0,
            vec![coin(456, "luna"), coin(456, "luna")],
            env.clone(),
        )
        .unwrap_err();
        assert_eq!(err, ContractError::MultipleCoins {});
        let err = repay_borrowed_funds_helper(
            deps.as_mut(),
            "creator",
            0,
            coins(456, "uust"),
            env.clone(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            ContractError::Std(StdError::generic_err(
                "You didn't send the right kind of funds",
            ))
        );

        repay_borrowed_funds_helper(
            deps.as_mut(),
            "bad_person",
            0,
            coins(506, "luna"),
            env.clone(),
        )
        .unwrap_err();

        let res = repay_borrowed_funds_helper(deps.as_mut(), "creator", 0, coins(506, "luna"), env)
            .unwrap();
        let env = mock_env();
        assert_eq!(
            res.messages,
            vec![
                SubMsg::new(BankMsg::Send {
                    to_address: "anyone".to_string(),
                    amount: coins(503, "luna"),
                }),
                SubMsg::new(
                    into_cosmos_msg(
                        Cw1155ExecuteMsg::SendFrom {
                            from: env.contract.address.to_string(),
                            to: "creator".to_string(),
                            token_id: "58".to_string(),
                            value: Uint128::new(45u128),
                            msg: None,
                        },
                        "nft",
                        None
                    )
                    .unwrap()
                ),
                SubMsg::new(
                    into_cosmos_msg(
                        FeeDistributorMsg::DepositFees {
                            addresses: vec!["nft".to_string()],
                            fee_type: FeeType::Funds
                        },
                        "T",
                        Some(coins(3, "luna"))
                    )
                    .unwrap()
                )
            ]
        );
    }

    #[test]
    fn test_defaulted_flow() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(0),
            duration_in_blocks: 0,
        };

        add_collateral_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "58",
            Some(Uint128::new(45u128)),
            None,
        )
        .unwrap();
        make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms,
            coins(456, "luna"),
        )
        .unwrap();

        accept_offer_helper(deps.as_mut(), "creator", 0, 0).unwrap();
        let mut env = mock_env();
        env.block.height = 12346;
        let err = repay_borrowed_funds_helper(
            deps.as_mut(),
            "creator",
            0,
            coins(456, "luna"),
            env.clone(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            ContractError::WrongLoanState {
                state: LoanState::Defaulted {},
            }
        );

        let err =
            withdraw_defaulted_loan_helper(deps.as_mut(), "bad_person", "creator", 0, env.clone())
                .unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});
        withdraw_defaulted_loan_helper(deps.as_mut(), "anyone", "creator", 0, env.clone()).unwrap();
        withdraw_defaulted_loan_helper(deps.as_mut(), "anyone", "creator", 0, env).unwrap_err();
    }
}
