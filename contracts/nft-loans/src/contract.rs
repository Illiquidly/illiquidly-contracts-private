#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_binary, Addr, BankMsg, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, Uint128,
};

use crate::error::ContractError;

use crate::state::{
    can_repay_loan, get_active_loan, get_loan, is_active_lender, is_collateral_withdrawable,
    is_lender, is_loan_acceptable, is_loan_counterable, is_loan_modifiable, is_owner,
    BORROWER_INFO, COLLATERAL_INFO, CONTRACT_INFO,
};

use cw1155::Cw1155ExecuteMsg;
use cw721::Cw721ExecuteMsg;

use nft_loans_export::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use nft_loans_export::state::{
    BorrowerInfo, CollateralInfo, ContractInfo, LoanState, LoanTerms, OfferInfo, OfferState,
};
use utils::msg::into_cosmos_msg;
use utils::state::{AssetInfo, Cw1155Coin, Cw721Coin};

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
        treasury: msg.treasury,
        fee_rate: msg.fee_rate,
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    Ok(Response::default().add_attribute("p2p-contract", "init"))
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
        /*
        ExecuteMsg::ForceDefault { borrower, loan_id } => {
            force_default(deps, env, info, borrower, loan_id)
        },
        */
        ExecuteMsg::WithdrawDefaultedLoan { borrower, loan_id } => {
            withdraw_defaulted_loan(deps, env, info, borrower, loan_id)
        }

        // Internal Contract Logic
        ExecuteMsg::SetOwner { owner } => set_owner(deps, env, info, owner),

        ExecuteMsg::SetTreasury { treasury } => set_treasury(deps, env, info, treasury),

        ExecuteMsg::SetFeeRate { fee_rate } => set_fee_rate(deps, env, info, fee_rate),

        // Generic (will have to remove at the end of development)
        _ => Err(ContractError::Std(StdError::generic_err(
            "Ow whaou, please wait just a bit, it's not implemented yet !",
        ))),
    }
}

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

    Ok(Response::new()
        .add_attribute("changed", "owner")
        .add_attribute("new_owner", new_owner))
}

pub fn set_treasury(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    treasury: String,
) -> Result<Response, ContractError> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    contract_info.treasury = treasury.clone();
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("changed", "treasury")
        .add_attribute("treasury", treasury))
}

pub fn set_fee_rate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    fee_rate: Uint128,
) -> Result<Response, ContractError> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    contract_info.fee_rate = fee_rate;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("changed", "fee_rate")
        .add_attribute("fee_rate", fee_rate))
}

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
    // First we save the collateral
    let borrower_info =
        BORROWER_INFO
            .load(deps.storage, &borrower)
            .map_or(BorrowerInfo::default(), |mut info| {
                info.last_collateral_id += 1;
                info
            });

    // We prepare for storing and transfering the token
    let transfer_message;
    let asset_info;
    if let Some(value) = value {
        // In case of a Cw1155
        asset_info = AssetInfo::Cw1155Coin(Cw1155Coin {
            address: address.clone(),
            token_id: token_id.clone(),
            value,
        });
        transfer_message = into_cosmos_msg(
            Cw1155ExecuteMsg::SendFrom {
                from: borrower.to_string(),
                to: env.contract.address.into(),
                token_id,
                value,
                msg: None,
            },
            address,
        )?;
    } else {
        // In case of a CW721
        asset_info = AssetInfo::Cw721Coin(Cw721Coin {
            address: address.clone(),
            token_id: token_id.clone(),
        });
        transfer_message = into_cosmos_msg(
            Cw721ExecuteMsg::TransferNft {
                recipient: env.contract.address.into(),
                token_id,
            },
            address,
        )?;
    }

    COLLATERAL_INFO.save(
        deps.storage,
        (&borrower, borrower_info.last_collateral_id.into()),
        &CollateralInfo {
            terms,
            associated_asset: asset_info,
            ..Default::default()
        },
    )?;

    BORROWER_INFO.save(deps.storage, &borrower.clone(), &borrower_info)?;

    Ok(Response::new()
        .add_message(transfer_message)
        .add_attribute("deposited", "collateral")
        .add_attribute("loan_id", borrower_info.last_collateral_id.to_string()))
}

pub fn withdraw_collateral(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    loan_id: u64,
) -> Result<Response, ContractError> {
    let borrower = info.sender;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id.into()))?;
    is_collateral_withdrawable(&collateral)?;

    // We start by creating the transfer message
    let transfer_message = _withdraw_asset(
        collateral.associated_asset.clone(),
        env.contract.address,
        borrower.clone(),
    )?;

    // We update the internal state, the token is no longer valid
    collateral.state = LoanState::AssetWithdrawn;
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id.into()), &collateral)?;

    // We return (don't forget the transfer messages)
    Ok(Response::new()
        .add_message(transfer_message)
        .add_attribute("withdrawn", "collateral"))
}

pub fn set_loan_terms(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    loan_id: u64,
    terms: LoanTerms,
) -> Result<Response, ContractError> {
    let borrower = info.sender;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id.into()))?;
    is_loan_modifiable(&collateral)?;

    collateral.terms = Some(terms);
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id.into()), &collateral)?;

    Ok(Response::new()
        .add_attribute("modified", "loan")
        .add_attribute("set", "terms"))
}

pub fn make_offer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
    terms: LoanTerms,
) -> Result<Response, ContractError> {
    let borrower = deps.api.addr_validate(&borrower)?;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id.into()))?;
    is_loan_counterable(&collateral)?;

    // First we make sure the transaction contains funds that match the principle
    if info.funds.len() != 1 {
        return Err(ContractError::MultipleCoins {});
    } else if terms.principle != info.funds[0].clone() {
        return Err(ContractError::FundsDontMatchTerms {});
    }
    // The we can save the new offer
    collateral.offers.push(OfferInfo {
        lender: info.sender.clone(),
        terms,
        state: OfferState::Published,
        deposited_funds: Some(info.funds[0].clone()),
    });

    // We save the changes to the collateral object
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id.into()), &collateral)?;

    Ok(Response::new()
        .add_attribute("made", "offer")
        .add_attribute("offer_id", (collateral.offers.len() - 1).to_string()))
}

pub fn cancel_offer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
    offer_id: u64,
) -> Result<Response, ContractError> {
    let borrower = deps.api.addr_validate(&borrower)?;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id.into()))?;

    // We can cancel an offer only if the Borrower is still searching for a loan
    if collateral.state != LoanState::Published {
        return Err(ContractError::Unauthorized {});
    }
    // We need to verify the offer exists
    let mut offer = is_lender(info.sender, &collateral, offer_id as usize)?;

    if offer.state != OfferState::Published {
        return Err(ContractError::CantChangeOfferState {
            from: offer.state,
            to: OfferState::Cancelled,
        });
    }
    offer.state = OfferState::Cancelled;
    collateral.offers[offer_id as usize] = offer;
    // We save the changes to the collateral object
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id.into()), &collateral)?;

    let withdraw_response = _withdraw_offer_unsafe(deps, borrower, loan_id, offer_id as usize)?;

    Ok(Response::new()
        .add_message(withdraw_response)
        .add_attribute("cancelled", "offer"))
}

pub fn withdraw_refused_offer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
    offer_id: u64,
) -> Result<Response, ContractError> {
    let borrower = deps.api.addr_validate(&borrower)?;
    let collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id.into()))?;

    // We need to verify the offer exists
    let offer = is_lender(info.sender, &collateral, offer_id as usize)?;
    if offer.state != OfferState::Published || collateral.state == LoanState::Published {
        return Err(ContractError::NotWithdrawable {});
    }
    let message = _withdraw_offer_unsafe(deps, borrower, loan_id, offer_id as usize)?;

    Ok(Response::new()
        .add_message(message)
        .add_attribute("withdraw", "funds")
        .add_attribute("offer", offer_id.to_string()))
}

// This withdraws the funds to the lender, without owner checks
// The offer is supposed to exist here
pub fn _withdraw_offer_unsafe(
    deps: DepsMut,
    borrower: Addr,
    loan_id: u64,
    offer_id: usize,
) -> Result<BankMsg, ContractError> {
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id.into()))?;

    // We prepare the transfer message
    let offer = collateral.offers[offer_id].clone();
    let res = Ok(BankMsg::Send {
        to_address: offer.lender.clone().to_string(),
        amount: vec![offer
            .deposited_funds
            .ok_or(ContractError::NoFundsToWithdraw {})?],
    });

    // We mark the deposited funds as withdrawn
    collateral.offers[offer_id].deposited_funds = None;
    // We save the changes
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id.into()), &collateral)?;

    res
}

pub fn refuse_offer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    loan_id: u64,
    offer_id: u64,
) -> Result<Response, ContractError> {
    let borrower = info.sender;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id.into()))?;

    // We need to verify the offer exists
    let mut offer = get_loan(&collateral, offer_id as usize)?;
    offer.state = OfferState::Refused;
    collateral.offers[offer_id as usize] = offer;
    // We save the changes to the collateral object
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id.into()), &collateral)?;

    Ok(Response::new()
        .add_attribute("refused", "offer")
        .add_attribute("offer_id", offer_id.to_string()))
}

pub fn accept_loan(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
) -> Result<Response, ContractError> {
    let borrower = deps.api.addr_validate(&borrower)?;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id.into()))?;
    is_loan_acceptable(&collateral)?;

    let terms: LoanTerms = collateral
        .terms
        .clone()
        .ok_or(ContractError::NoTermsSpecified {})?;
    // We receive the funds sent by the lender
    if info.funds.len() != 1 {
        return Err(ContractError::MultipleCoins {});
    } else if terms.principle != info.funds[0].clone() {
        return Err(ContractError::FundsDontMatchTerms {});
    }

    // Then we can save the original offer as accepted
    collateral.state = LoanState::Started;
    collateral.start_block = Some(env.block.height);

    // We erase all other offers, only the one that was accepted stands
    collateral.offers = vec![OfferInfo {
        lender: info.sender,
        terms,
        state: OfferState::Accepted,
        deposited_funds: Some(info.funds[0].clone()),
    }];
    collateral.active_loan = Some(0);

    // We change the loan state
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id.into()), &collateral)?;

    let messages = _withdraw_borrowed_funds(deps, env, borrower, loan_id)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("accepted", "loan")
        .add_attribute("let's", "go")
        .add_attribute("borrowed funds", "distributed"))
}

pub fn accept_offer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    loan_id: u64,
    offer_id: u64,
) -> Result<Response, ContractError> {
    let borrower = info.sender;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id.into()))?;
    is_loan_acceptable(&collateral)?;

    let mut offer = get_loan(&collateral, offer_id as usize)?;

    if offer.state == OfferState::Published {
        // We can start the loan right away !
        offer.state = OfferState::Accepted;
        collateral.state = LoanState::Started;
        collateral.start_block = Some(env.block.height);
        collateral.active_loan = Some(offer_id);
        collateral.offers[offer_id as usize] = offer;

        COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id.into()), &collateral)?;
    } else {
        return Err(ContractError::OfferNotFound {});
    };

    // We transfer the funds directly when the offer is accepted

    let messages = _withdraw_borrowed_funds(deps, env, borrower, loan_id)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("accepted", "loan")
        .add_attribute("let's", "go")
        .add_attribute("borrowed funds", "distributed"))
}

pub fn _withdraw_borrowed_funds(
    deps: DepsMut,
    _env: Env,
    borrower: Addr,
    loan_id: u64,
) -> Result<Vec<BankMsg>, ContractError> {
    let collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id.into()))?;

    // We can only withdraw borrowed funds when we start the loan
    if collateral.state != LoanState::Started {
        return Err(ContractError::WrongLoanState {
            state: collateral.state,
        });
    }

    let offer = get_active_loan(&collateral)?;

    // We need to prepare the funds transfer to the borrower
    Ok(vec![BankMsg::Send {
        to_address: borrower.to_string(),
        amount: vec![offer
            .deposited_funds
            .ok_or(ContractError::NoFundsToWithdraw {})?],
    }])
}

pub fn repay_borrowed_funds(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    loan_id: u64,
) -> Result<Response, ContractError> {
    let borrower = info.sender;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id.into()))?;
    can_repay_loan(env.clone(), &collateral)?;

    let offer = get_active_loan(&collateral)?;

    // We compute the necessary additionnal interest if the borrower pays late
    let late_interest = if offer.terms.default_terms.is_some()
        && collateral.start_block.unwrap() + offer.terms.duration_in_blocks < env.block.height
    {
        offer.terms.interest
            * Uint128::new(
                (env.block.height - collateral.start_block.unwrap()
                    + offer.terms.duration_in_blocks)
                    .into(),
            )
            * offer.terms.default_terms.unwrap().late_payback_rate
            / Uint128::new(10_000_000u128)
    } else {
        Uint128::new(0)
    };

    let interests = offer.terms.interest + late_interest;

    // We verify the sent funds correspond to the principle + interests
    if info.funds.len() != 1 {
        return Err(ContractError::MultipleCoins {});
    } else if offer.terms.principle.denom != info.funds[0].denom.clone() {
        return Err(ContractError::Std(StdError::generic_err(
            "You didn't send the right kind of funds",
        )));
    } else if offer.terms.principle.amount + interests > info.funds[0].amount{
        return Err(ContractError::Std(StdError::generic_err(
            format!(
                "Fund sent do not match the loan terms (principle + interests). Needed : {needed}, Received : {received}", 
                needed = offer.terms.principle.amount + interests,
                received = info.funds[0].amount.clone()
            )
        )));
    }

    collateral.state = LoanState::Ended;

    let contract = CONTRACT_INFO.load(deps.storage)?;

    let lender_payback = offer.terms.principle.amount
        + interests * (Uint128::new(100_000u128) - contract.fee_rate) / Uint128::new(100_000u128);

    let treasury_payback = info.funds[0].amount - lender_payback;

    let mut res = Response::new();
    // We pay the lender back
    res = res.add_message(BankMsg::Send {
        to_address: offer.lender.to_string(),
        amount: coins(lender_payback.u128(), info.funds[0].denom.clone()),
    });
    // We pay the fee to the treasury
    res = res.add_message(BankMsg::Send {
        to_address: contract.treasury,
        amount: coins(treasury_payback.u128(), info.funds[0].denom.clone()),
    });
    res = res.add_message(_withdraw_asset(
        collateral.associated_asset,
        env.contract.address,
        borrower,
    )?);

    Ok(res
        .add_attribute("pay_back", "funds")
        .add_attribute("type", "borrowed"))
}

pub fn withdraw_defaulted_loan(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
) -> Result<Response, ContractError> {
    let borrower = deps.api.addr_validate(&borrower)?;
    let mut collateral = COLLATERAL_INFO.load(deps.storage, (&borrower, loan_id.into()))?;
    let offer = is_active_lender(info.sender, &collateral)?;

    // We need to test if we can default the loan.
    // This is different from the is_loan_defaulted function
    if collateral.state != LoanState::Started
        || collateral.start_block.unwrap() + offer.terms.duration_in_blocks >= env.block.height
    {
        return Err(ContractError::WrongLoanState {
            state: collateral.state,
        });
    }

    collateral.state = LoanState::Defaulted;

    let lender = offer.lender;
    let withdraw_message = _withdraw_asset(
        collateral.associated_asset.clone(),
        env.contract.address,
        lender,
    )?;
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id.into()), &collateral)?;

    Ok(Response::new()
        .add_message(withdraw_message)
        .add_attribute("withdraw", "collateral"))
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
            )
        }
        _ => Err(StdError::generic_err("Unreachable error")),
    }
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
        .load(deps.storage, (&borrower, loan_id.into()))
        .map_err(|_| StdError::generic_err("LoanNotFound"))
}

pub fn query_borrower_info(deps: Deps, borrower: String) -> StdResult<BorrowerInfo> {
    let borrower = deps.api.addr_validate(&borrower)?;
    BORROWER_INFO
        .load(deps.storage, &borrower)
        .map_err(|_| StdError::generic_err("UnknownBorrower"))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use cosmwasm_std::{
        coin, coins,
        testing::{mock_dependencies, mock_env, mock_info},
        Api, Coin, SubMsg,
    };
    use cw_storage_plus::U64Key;
    use nft_loans_export::state::DefaultTerms;

    fn init_helper(deps: DepsMut) {
        let instantiate_msg = InstantiateMsg {
            name: "nft-loan".to_string(),
            owner: None,
            treasury: "T".to_string(),
            fee_rate: Uint128::new(5_000u128),
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();

        instantiate(deps, env, info, instantiate_msg).unwrap();
    }

    #[test]
    fn test_init_sanity() {
        let mut deps = mock_dependencies(&[]);
        let instantiate_msg = InstantiateMsg {
            name: "p2p-trading".to_string(),
            owner: Some("this_address".to_string()),
            treasury: "T".to_string(),
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
                treasury: "T".to_string(),
                fee_rate: Uint128::new(5_000u128),
            }
        );

        let info = mock_info("this_address", &[]);
        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::SetTreasury {
                treasury: "S".to_string(),
            },
        )
        .unwrap();
        assert_eq!(
            CONTRACT_INFO.load(&deps.storage).unwrap().treasury,
            "S".to_string()
        );

        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
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
        funds: Coin,
        env: Env,
    ) -> Result<Response, ContractError> {
        let info = mock_info(borrower, &[funds]);

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
        let mut deps = mock_dependencies(&[]);
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
            .load(&deps.storage, (&creator_addr, 0.into()))
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
            .load(&deps.storage, (&creator_addr, 1.into()))
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
            .load(&deps.storage, (&creator_addr, 2.into()))
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
        let mut deps = mock_dependencies(&[]);
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
            .load(&deps.storage, (&creator_addr, 0.into()))
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
            .load(&deps.storage, (&creator_addr, 1.into()))
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
            .load(&deps.storage, (&creator_addr, 2.into()))
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
    }

    #[test]
    fn test_accept_loan() {
        let mut deps = mock_dependencies(&[]);
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
            default_terms: None,
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
            .load(&deps.storage, (&creator_addr, 0.into()))
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
    fn test_make_offer() {
        let mut deps = mock_dependencies(&[]);
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
            default_terms: None,
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
            terms.clone(),
            coins(456, "luna"),
        )
        .unwrap();
    }

    #[test]
    fn test_cancel_offer() {
        let mut deps = mock_dependencies(&[]);
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
            default_terms: None,
        };

        make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms.clone(),
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
        let mut deps = mock_dependencies(&[]);
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
            default_terms: None,
        };

        make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms.clone(),
            coins(456, "luna"),
        )
        .unwrap();

        refuse_offer_helper(deps.as_mut(), "bad_person", 0, 0).unwrap_err();
        refuse_offer_helper(deps.as_mut(), "creator", 0, 0).unwrap();

        let offer = COLLATERAL_INFO
            .load(
                &deps.storage,
                (&deps.api.addr_validate("creator").unwrap(), U64Key::new(0)),
            )
            .unwrap()
            .offers[0]
            .clone();

        assert_eq!(offer.state, OfferState::Refused);
    }

    #[test]
    fn test_cancel_accepted() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(0),
            duration_in_blocks: 0,
            default_terms: None,
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

        accept_loan_helper(deps.as_mut(), "anyone", "creator", 0, coins(456, "luna")).unwrap();

        withdraw_collateral_helper(deps.as_mut(), "creator", 0).unwrap_err();
        cancel_offer_helper(deps.as_mut(), "anyone", "creator", 0, 0).unwrap_err();
    }

    #[test]
    fn test_withdraw_refused() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(0),
            duration_in_blocks: 0,
            default_terms: None,
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
            terms.clone(),
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
    fn test_normal_flow() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(50),
            duration_in_blocks: 1,
            default_terms: None,
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

        accept_offer_helper(deps.as_mut(), "creator", 0, 0).unwrap();
        // Loan starts

        let env = mock_env();
        withdraw_defaulted_loan_helper(deps.as_mut(), "anyone", "creator", 0, env.clone())
            .unwrap_err();

        let err = repay_borrowed_funds_helper(
            deps.as_mut(),
            "creator",
            0,
            coin(456, "luna"),
            env.clone(),
        )
        .unwrap_err();
        assert_eq!(err, ContractError::Std(
            StdError::generic_err("Fund sent do not match the loan terms (principle + interests). Needed : 506, Received : 456")
        ));
        repay_borrowed_funds_helper(
            deps.as_mut(),
            "bad_person",
            0,
            coin(506, "luna"),
            env.clone(),
        )
        .unwrap_err();

        let res = repay_borrowed_funds_helper(deps.as_mut(), "creator", 0, coin(506, "luna"), env)
            .unwrap();
        let env = mock_env();
        assert_eq!(
            res.messages,
            vec![
                SubMsg::new(BankMsg::Send {
                    to_address: "anyone".to_string(),
                    amount: coins(503, "luna"),
                }),
                SubMsg::new(BankMsg::Send {
                    to_address: "T".to_string(),
                    amount: coins(3, "luna"),
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
                        "nft"
                    )
                    .unwrap()
                ),
            ]
        );
    }

    #[test]
    fn test_defaulted_flow() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(0),
            duration_in_blocks: 0,
            default_terms: None,
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
            terms.clone(),
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
            coin(456, "luna"),
            env.clone(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            ContractError::WrongLoanState {
                state: LoanState::Defaulted {},
            }
        );

        withdraw_defaulted_loan_helper(deps.as_mut(), "anyone", "creator", 0, env.clone()).unwrap();
        withdraw_defaulted_loan_helper(deps.as_mut(), "anyone", "creator", 0, env).unwrap_err();
    }

    #[test]
    fn test_late_repay_flow() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let terms = LoanTerms {
            principle: coin(456, "luna"),
            interest: Uint128::new(50),
            duration_in_blocks: 0,
            default_terms: Some(DefaultTerms {
                late_payback_rate: Uint128::new(10_000_000u128),
            }),
        };

        add_collateral_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "58",
            Some(Uint128::new(50u128)),
            Some(terms.clone()),
        )
        .unwrap();
        accept_loan_helper(deps.as_mut(), "anyone", "creator", 0, coins(456, "luna")).unwrap();

        let mut env = mock_env();
        env.block.height = 12346;
        let err = repay_borrowed_funds_helper(
            deps.as_mut(),
            "creator",
            0,
            coin(506, "luna"),
            env.clone(),
        )
        .unwrap_err();
        assert_eq!(err, ContractError::Std(
            StdError::generic_err("Fund sent do not match the loan terms (principle + interests). Needed : 556, Received : 506")
        ));
        let res = repay_borrowed_funds_helper(
            deps.as_mut(),
            "creator",
            0,
            coin(556, "luna"),
            env.clone(),
        )
        .unwrap();
        assert_eq!(
            res.messages,
            vec![
                SubMsg::new(BankMsg::Send {
                    to_address: "anyone".to_string(),
                    amount: coins(551, "luna"),
                }),
                SubMsg::new(BankMsg::Send {
                    to_address: "T".to_string(),
                    amount: coins(5, "luna"),
                }),
                SubMsg::new(
                    into_cosmos_msg(
                        Cw1155ExecuteMsg::SendFrom {
                            from: env.contract.address.to_string(),
                            to: "creator".to_string(),
                            token_id: "58".to_string(),
                            value: Uint128::new(50u128),
                            msg: None,
                        },
                        "nft"
                    )
                    .unwrap()
                ),
            ]
        );
    }
}
