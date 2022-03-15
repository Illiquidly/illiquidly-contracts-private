#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Uint128,
};

use crate::error::ContractError;

use crate::state::{
    is_collateral_withdrawable, is_loan_acceptable, is_loan_counterable, is_loan_modifiable,
    is_owner, BORROWER_INFO, COLLATERAL_INFO, CONTRACT_INFO,
};

use cw1155::Cw1155ExecuteMsg;
use cw721::Cw721ExecuteMsg;

use nft_loans_export::msg::{ExecuteMsg, InstantiateMsg, LoanTerms, QueryMsg};
use nft_loans_export::state::{
    BorrowerInfo, CollateralInfo, ContractInfo, LoanState, OfferInfo, OfferState,
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
        fee_contract: None,
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
        // Internal Contract Logic
        ExecuteMsg::SetNewOwner { owner } => set_new_owner(deps, env, info, owner),

        // Generic (will have to remove at the end of development)
        _ => Err(ContractError::Std(StdError::generic_err(
            "Ow whaou, please wait just a bit, it's not implemented yet !",
        ))),
    }
}

pub fn set_new_owner(
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

    // We prepare for storgin and transfering the token
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
                token_id: token_id.clone(),
                value,
                msg: None,
            },
            address.clone(),
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
                token_id: token_id.clone(),
            },
            address.clone(),
        )?;
    }

    COLLATERAL_INFO.save(
        deps.storage,
        (&borrower, borrower_info.last_collateral_id.into()),
        &CollateralInfo {
            borrower: borrower.clone(),
            terms,
            associated_asset: asset_info,
            ..Default::default()
        },
    )?;
    // Yes, I could do that with update, but update is a mess to understand
    BORROWER_INFO.save(deps.storage, &borrower.clone(), &borrower_info)?;

    Ok(Response::new()
        .add_message(transfer_message)
        .add_attribute("deposited", "collateral")
        .add_attribute("address", address)
        .add_attribute("token_id", token_id))
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
    let (address, token_id);
    let transfer_message = match collateral.associated_asset.clone() {
        AssetInfo::Cw1155Coin(cw1155) => {
            address = cw1155.address;
            token_id = cw1155.token_id;
            into_cosmos_msg(
                Cw1155ExecuteMsg::SendFrom {
                    from: env.contract.address.to_string(),
                    to: borrower.to_string(),
                    token_id: token_id.clone(),
                    value: cw1155.value,
                    msg: None,
                },
                address.clone(),
            )
        }

        AssetInfo::Cw721Coin(cw721) => {
            address = cw721.address;
            token_id = cw721.token_id;
            into_cosmos_msg(
                Cw721ExecuteMsg::TransferNft {
                    recipient: borrower.to_string(),
                    token_id: token_id.clone(),
                },
                address.clone(),
            )
        }
        _ => {
            address = "".to_string();
            token_id = "".to_string();
            Err(StdError::generic_err("Unreachable error"))
        }
    }?;

    // We update the internal state, the token is no longer valid
    collateral.state = LoanState::AssetWithdrawn;
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id.into()), &collateral)?;

    // We return (don't forget the transfer messages)
    Ok(Response::new()
        .add_message(transfer_message)
        .add_attribute("withdrawn", "collateral")
        .add_attribute("address", address)
        .add_attribute("token_id", token_id))
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
        return Err(ContractError::Std(StdError::generic_err(
            "You have to send exactly one coin with this transaction",
        )));
    } else if terms.principle != Some(info.funds[0].clone()) {
        return Err(ContractError::Std(StdError::generic_err(
            "Fund sent do not match the loan terms",
        )));
    }
    // The we can same the new offer

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

    // We need to verify the offer exists
    let offer_id = offer_id as usize;
    let cancel_response = if offer_id < collateral.offers.len() {
        if info.sender != collateral.offers[offer_id].lender {
            Err(ContractError::Unauthorized {})
        } else {
            collateral.offers[offer_id].state = OfferState::Cancelled;

            // We save the changes to the collateral object
            COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id.into()), &collateral)?;
            Ok(Response::new()
                .add_attribute("cancelled", "offer")
                .add_attribute("offer_id", offer_id.to_string()))
        }
    } else {
        return Err(ContractError::OfferNotFound {});
    };
    cancel_response
    //withdraw_cancelled_offer(deps, env, info, borrower, loan_id, offer_id)
    //TODO we need to make the funds withdrawable
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
    let offer_id = offer_id as usize;
    if offer_id < collateral.offers.len() {
        collateral.offers[offer_id as usize].state = OfferState::Refused;

        // We save the changes to the collateral object
        COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id.into()), &collateral)?;
        Ok(Response::new()
            .add_attribute("refused", "offer")
            .add_attribute("offer_id", offer_id.to_string()))
    } else {
        Err(ContractError::Std(StdError::generic_err(
            "No such offer_id",
        )))
    }
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
        return Err(ContractError::Std(StdError::generic_err(
            "You have to send exactly one coin with this transaction",
        )));
    } else if terms.principle == Some(info.funds[0].clone()) {
        // The loan can start, the deposit is sufficient
        collateral.state = LoanState::Started;
        collateral.start_block = Some(env.block.height);
        collateral.offers = vec![OfferInfo {
            lender: info.sender,
            terms,
            state: OfferState::Published,
            deposited_funds: None,
        }];
        collateral.active_loan = Some(0);
    } else {
        return Err(ContractError::Std(StdError::generic_err(
            "Fund sent do not match the loan terms",
        )));
    }

    // We change the loan state
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id.into()), &collateral)?;

    Ok(Response::new()
        .add_attribute("accepted", "loan")
        .add_attribute("let's", "go"))
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

    // We can start the loan right away !
    collateral.state = LoanState::Started;
    collateral.start_block = Some(env.block.height);
    collateral.active_loan = Some(offer_id);

    // We change the loan state
    COLLATERAL_INFO.save(deps.storage, (&borrower, loan_id.into()), &collateral)?;

    Ok(Response::new()
        .add_attribute("accepted", "loan")
        .add_attribute("let's", "go"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        /*
        QueryMsg::ContractInfo {} => to_binary(&query_contract_info(deps)?),
        QueryMsg::TradeInfo { trade_id } => to_binary(
            &load_trade(deps.storage, trade_id)
                .map_err(|e| StdError::generic_err(e.to_string()))?,
        )*/
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use cosmwasm_std::{
        coin,
        testing::{mock_dependencies, mock_env, mock_info},
        Api, Coin,
    };

    fn init_helper(deps: DepsMut) {
        let instantiate_msg = InstantiateMsg {
            name: "nft-loan".to_string(),
            owner: None,
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
        };
        let info = mock_info("owner", &[]);
        let env = mock_env();

        let res_init = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
        assert_eq!(0, res_init.messages.len());
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
        coin: Coin,
    ) -> Result<Response, ContractError> {
        let info = mock_info(lender, &[coin]);
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

    fn accept_loan_helper(
        deps: DepsMut,
        lender: &str,
        borrower: &str,
        loan_id: u64,
        coin: Coin,
    ) -> Result<Response, ContractError> {
        let info = mock_info(lender, &[coin]);
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

    fn withdraw_collateral_helper(
        deps: DepsMut,
        creator: &str,
        loan_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(creator, &[]);
        let env = mock_env();

        execute(deps, env, info, ExecuteMsg::WithdrawCollateral { loan_id })
    }

    #[test]
    fn test_add_collateral() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());
        let res = add_collateral_helper(deps.as_mut(), "creator", "nft", "58", None, None).unwrap();
        assert_eq!(1, res.messages.len());

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
                borrower: creator_addr.clone(),
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
                borrower: creator_addr.clone(),
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
                borrower: creator_addr.clone(),
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
                borrower: creator_addr.clone(),
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
                borrower: creator_addr.clone(),
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
                borrower: creator_addr.clone(),
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
            principle: Some(coin(456, "luna")),
            rate: None,
            duration_in_block: None,
            default_terms: None,
        };
        set_terms_helper(deps.as_mut(), "creator", 0, terms.clone()).unwrap();

        let err = accept_loan_helper(deps.as_mut(), "anyone", "creator", 0, coin(123, "luna"))
            .unwrap_err();
        assert_eq!(
            err,
            ContractError::Std(StdError::generic_err(
                "Fund sent do not match the loan terms",
            ))
        );
        accept_loan_helper(deps.as_mut(), "anyone", "creator", 0, coin(456, "luna")).unwrap();

        let creator_addr = deps.api.addr_validate("creator").unwrap();
        let coll_info = COLLATERAL_INFO
            .load(&deps.storage, (&creator_addr, 0.into()))
            .unwrap();
        assert_eq!(
            coll_info,
            CollateralInfo {
                borrower: creator_addr.clone(),
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
                    state: OfferState::Published,
                    deposited_funds: None,
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
            principle: Some(coin(456, "luna")),
            rate: None,
            duration_in_block: None,
            default_terms: None,
        };

        let err = make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms.clone(),
            coin(6765, "luna"),
        )
        .unwrap_err();
        assert_eq!(
            err,
            ContractError::Std(StdError::generic_err(
                "Fund sent do not match the loan terms",
            ))
        );

        make_offer_helper(
            deps.as_mut(),
            "anyone",
            "creator",
            0,
            terms.clone(),
            coin(456, "luna"),
        )
        .unwrap();
    }
}
