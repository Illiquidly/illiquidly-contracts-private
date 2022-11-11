use anyhow::{anyhow, Result};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};

use lender_export::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ZonesResponse};
use lender_export::state::{ContractInfo, State, BORROWS, CONTRACT_INFO, STATE};

use crate::error::ContractError;
use crate::execute::{
    _execute_repay, execute_borrow, execute_borrow_more, execute_raise_interest_rate,
};
use crate::query::{
    get_asset_interests, get_asset_price, get_expensive_zone_limit_price,
    get_safe_zone_limit_price, get_vault_token_asset,
};
use crate::state::{set_lock, set_oracle, set_owner};
use cw_4626::state::AssetInfo;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response> {
    // We start by initating the state of the contract
    let initial_state = State {
        borrow_locked: false,
    };

    STATE.save(deps.storage, &initial_state)?;

    let vault_token = deps.api.addr_validate(&msg.vault_token)?;
    // Then the contract info
    let contract_info = ContractInfo {
        name: msg.name,
        oracle: msg
            .oracle
            .map(|x| deps.api.addr_validate(&x))
            .transpose()?
            .unwrap_or_else(|| info.sender.clone()),
        owner: msg
            .owner
            .map(|x| deps.api.addr_validate(&x))
            .transpose()?
            .unwrap_or(info.sender),
        vault_token: vault_token.clone(),
        vault_asset: get_vault_token_asset(deps.as_ref(), vault_token.to_string())?,
        increasor_incentives: msg.increasor_incentives,
        interests_fee_rate: msg.interests_fee_rate,
        fee_distributor: deps.api.addr_validate(&msg.fee_distributor)?,
    };
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::default()
        .add_attribute("action", "init")
        .add_attribute("contract", "lender")
        .add_attribute("owner", contract_info.owner))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> Result<Response> {
    match msg {
        // Functions used to borrow funds
        ExecuteMsg::Borrow {
            asset_info,
            assets_to_borrow,
            borrow_mode,
        } => execute_borrow(deps, env, info, asset_info, assets_to_borrow, borrow_mode),
        ExecuteMsg::BorrowMore {
            loan_id,
            assets_to_borrow,
        } => execute_borrow_more(deps, env, info, loan_id, assets_to_borrow),

        // Functions used to repay or liquidate loans
        ExecuteMsg::Receive {
            sender,
            amount,
            msg,
        } => receive_assets(deps, env, info, sender, amount, msg),

        ExecuteMsg::Repay {
            borrower,
            loan_id,
            assets,
        } => execute_repay_native_funds(deps, env, info, borrower, loan_id, assets),

        // Function used to raise the interests rate
        ExecuteMsg::RaiseRate { borrower, loan_id } => {
            execute_raise_interest_rate(deps, env, info, borrower, loan_id)
        }

        // Contract Administration
        ExecuteMsg::SetOwner { owner } => set_owner(deps, info, owner),
        ExecuteMsg::SetOracle { oracle } => set_oracle(deps, info, oracle),
        ExecuteMsg::ToggleLock { lock } => set_lock(deps, info, lock),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::State {} => to_binary(&STATE.load(deps.storage)?),
        QueryMsg::ContratInfo {} => to_binary(&CONTRACT_INFO.load(deps.storage)?),
        QueryMsg::BorrowInfo { borrower, loan_id } => {
            let borrower = deps.api.addr_validate(&borrower)?;
            to_binary(&BORROWS.load(deps.storage, (&borrower, loan_id))?)
        }
        QueryMsg::BorrowZones { asset_info } => {
            let collateral_price = get_asset_price(deps, env, asset_info)?;
            let safe_zone_limit = get_safe_zone_limit_price(collateral_price)?;
            let expensive_zone_limit = get_expensive_zone_limit_price(collateral_price)?;
            to_binary(&ZonesResponse {
                safe_zone_limit,
                expensive_zone_limit,
            })
        }
        QueryMsg::BorrowTerms {
            asset_info,
            borrow_mode,
            borrow_zone,
        } => to_binary(&get_asset_interests(
            deps,
            env,
            asset_info,
            borrow_mode,
            borrow_zone,
        )?),
    }
}

pub fn receive_assets(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response> {
    match from_binary(&msg)? {
        ExecuteMsg::Repay {
            borrower,
            loan_id,
            assets,
        } => {
            // This function can accept a Cw20 Token solely
            // We make sure the sent assets correspond to the vault saved
            let contract_info = CONTRACT_INFO.load(deps.storage)?;
            if let AssetInfo::Cw20(x) = contract_info.vault_asset {
                if deps.api.addr_validate(&x)? != info.sender {
                    return Err(anyhow!(ContractError::AssetsSentDontMatch {}));
                }
            } else {
                return Err(anyhow!(ContractError::AssetsSentDontMatch {}));
            }

            // We make sure the amount sent is the amount specified in the repay message
            if assets != amount {
                return Err(anyhow!(ContractError::AssetsSentDontMatch {}));
            }

            let sender = deps.api.addr_validate(&sender)?;
            // We now call the repay function accordingly
            _execute_repay(deps, env, sender, borrower, loan_id, assets)
        }
        _ => Err(anyhow!(ContractError::ReceiveMsgNotAccepted {})),
    }
}

pub fn execute_repay_native_funds(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    borrower: String,
    loan_id: u64,
    assets: Uint128,
) -> Result<Response> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    // We test that the funds sent match the assets
    if info.funds.len() != 1 {
        return Err(anyhow!(ContractError::AssetsSentDontMatch {}));
    }

    // This function can accept a native tokens solely
    // We make sure the sent assets correspond to the vault saved
    if let AssetInfo::Coin(x) = contract_info.vault_asset {
        if x != info.funds[0].denom {
            return Err(anyhow!(ContractError::AssetsSentDontMatch {}));
        }
    } else {
        return Err(anyhow!(ContractError::AssetsSentDontMatch {}));
    }
    // We make sure the amount sent is the amount specified in the repay message
    if assets != info.funds[0].amount {
        return Err(anyhow!(ContractError::AssetsSentDontMatch {}));
    }

    // We now call the repay function accordingly
    _execute_repay(deps, env, info.sender, borrower, loan_id, assets)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::contract::instantiate;
    use crate::custom_mock_querier::tests::mock_dependencies;
    use crate::error::ContractError;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{coins, Api, Coin, CosmosMsg, DepsMut, Uint128, WasmMsg};
    use cw721::Cw721ExecuteMsg;
    use cw_4626::msg::ExecuteMsg as Cw4626ExecuteMsg;
    use fee_contract_export::state::FeeType;
    use fee_distributor_export::msg::ExecuteMsg as DistributorExecuteMsg;
    use lender_export::msg::InstantiateMsg;
    use lender_export::state::{BorrowMode, Cw721Info};
    use utils::msg::into_cosmos_msg;

    fn init_helper(deps: DepsMut) {
        let instantiate_msg = InstantiateMsg {
            name: "Iliq treasury token".to_string(),
            owner: Some("creator".to_string()),
            oracle: Some("oracle".to_string()),
            vault_token: "vault_token".to_string(),
            increasor_incentives: Uint128::from(100u128),
            interests_fee_rate: Uint128::from(1_000u128),
            fee_distributor: "distributor".to_string(),
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();

        instantiate(deps, env, info, instantiate_msg).unwrap();
    }

    fn borrow_helper(
        deps: DepsMut,
        sender: &str,
        nft_address: &str,
        token_id: &str,
        assets: Vec<Coin>,
        principle: u128,
    ) -> Result<Response> {
        let info = mock_info(sender, &assets);
        let env = mock_env();
        execute(
            deps,
            env,
            info,
            ExecuteMsg::Borrow {
                asset_info: Cw721Info {
                    nft_address: nft_address.to_string(),
                    token_id: token_id.to_string(),
                },
                assets_to_borrow: Uint128::from(principle),
                borrow_mode: BorrowMode::Fixed,
            },
        )
    }

    fn borrow_continuous_helper(
        deps: DepsMut,
        sender: &str,
        nft_address: &str,
        token_id: &str,
        assets: Vec<Coin>,
        principle: u128,
    ) -> Result<Response> {
        let info = mock_info(sender, &assets);
        let env = mock_env();
        execute(
            deps,
            env,
            info,
            ExecuteMsg::Borrow {
                asset_info: Cw721Info {
                    nft_address: nft_address.to_string(),
                    token_id: token_id.to_string(),
                },
                assets_to_borrow: Uint128::from(principle),
                borrow_mode: BorrowMode::Continuous,
            },
        )
    }

    fn borrow_more_helper(
        deps: DepsMut,
        sender: &str,
        loan_id: u64,
        assets: Vec<Coin>,
        principle: u128,
    ) -> Result<Response> {
        let info = mock_info(sender, &assets);
        let env = mock_env();
        execute(
            deps,
            env,
            info,
            ExecuteMsg::BorrowMore {
                loan_id,
                assets_to_borrow: Uint128::from(principle),
            },
        )
    }

    fn repay_helper(
        deps: DepsMut,
        address: &str,
        borrower: &str,
        loan_id: u64,
        assets: u128,
        block_height_delay: Option<u64>,
    ) -> Result<Response> {
        let mut env = mock_env();
        if let Some(block_height) = block_height_delay {
            env.block.height += block_height
        }

        let info = mock_info(address, &coins(assets, "utest"));
        execute(
            deps,
            env,
            info,
            ExecuteMsg::Repay {
                borrower: borrower.to_string(),
                loan_id,
                assets: Uint128::from(assets),
            },
        )
    }

    #[test]
    fn test_init_sanity() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());
    }

    #[test]
    fn test_borrow_sanity() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let err = borrow_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "token_id",
            vec![],
            8_600_000_000u128,
        )
        .unwrap_err();
        assert_eq!(
            err.downcast::<ContractError>().unwrap(),
            ContractError::TooMuchBorrowed {
                collateral_address: "nft".to_string(),
                wanted: Uint128::from(8_600_000_000u128),
                limit: Uint128::from(53661300u128)
            }
        );

        let res = borrow_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "token_id",
            vec![],
            8742u128,
        )
        .unwrap();
        assert_eq!(
            res,
            Response::new()
                .add_message(
                    into_cosmos_msg(
                        Cw721ExecuteMsg::TransferNft {
                            recipient: "cosmos2contract".to_string(),
                            token_id: "token_id".to_string(),
                        },
                        "nft".to_string(),
                        None
                    )
                    .unwrap()
                )
                .add_message(
                    into_cosmos_msg(
                        Cw4626ExecuteMsg::Borrow {
                            receiver: "creator".to_string(),
                            assets: Uint128::from(8742u128)
                        },
                        "vault_token".to_string(),
                        None
                    )
                    .unwrap()
                )
                .add_attribute("action", "borrow")
                .add_attribute("collateral_address", "nft")
                .add_attribute("collateral_token_id", "token_id")
                .add_attribute("borrower", "creator")
        );

        // We verify the internal structure has changed
        let borrower = deps.api.addr_validate("creator").unwrap();
        assert_eq!(
            BORROWS
                .load(&deps.storage, (&borrower, 0u64))
                .unwrap()
                .principle,
            Uint128::from(8742u128)
        )
    }

    #[test]
    fn test_repay_sanity() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        // If the person is not the creator, it can't liquidate before the duration ends
        borrow_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "token_id",
            vec![],
            8742u128,
        )
        .unwrap();
        let err = repay_helper(
            deps.as_mut(),
            "bad_guy",
            "creator",
            0u64,
            8742u128 + 67u128,
            None,
        )
        .unwrap_err();
        assert_eq!(
            err.downcast::<ContractError>().unwrap(),
            ContractError::CannotLiquidateBeforeDefault {}
        );

        // If the loan doesn't exist it should return an error
        repay_helper(
            deps.as_mut(),
            "creator",
            "anyone",
            0u64,
            8742u128 + 67u128,
            None,
        )
        .unwrap_err();

        // We should return the asset to the borrower at then end of the loan
        let res = repay_helper(
            deps.as_mut(),
            "creator",
            "creator",
            0u64,
            8742u128 + 67u128,
            None,
        )
        .unwrap();

        // We print the messages for debugging
        res.messages.iter().for_each(|x| {
            if let CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) = x.msg.clone() {
                println!("{:?}", std::str::from_utf8(msg.as_slice()));
            }
        });

        assert_eq!(
            res,
            Response::new()
                .add_message(
                    into_cosmos_msg(
                        Cw721ExecuteMsg::TransferNft {
                            recipient: "creator".to_string(),
                            token_id: "token_id".to_string(),
                        },
                        "nft".to_string(),
                        None
                    )
                    .unwrap()
                )
                .add_message(
                    into_cosmos_msg(
                        DistributorExecuteMsg::DepositFees {
                            addresses: vec!["nft".to_string()],
                            fee_type: FeeType::Funds
                        },
                        "distributor".to_string(),
                        Some(coins(6u128, "utest"))
                    )
                    .unwrap()
                )
                .add_message(
                    into_cosmos_msg(
                        Cw4626ExecuteMsg::Repay {
                            owner: None,
                            assets: Uint128::from(8803u128)
                        },
                        "vault_token".to_string(),
                        Some(coins(8803u128, "utest"))
                    )
                    .unwrap()
                )
                .add_attribute("action", "repay")
                .add_attribute("caller", "creator")
                .add_attribute("borrower", "creator")
                .add_attribute("assets", 8809u128.to_string())
                .add_attribute("collateral_withdrawn", "true")
        );
        let err = repay_helper(
            deps.as_mut(),
            "creator",
            "creator",
            0u64,
            8742u128 + 67u128,
            None,
        )
        .unwrap_err();
        assert_eq!(
            err.downcast::<ContractError>().unwrap(),
            ContractError::AssetAlreadyWithdrawn {}
        );
    }

    #[test]
    fn test_liquidate_sanity() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        // If the person is not the creator, it can't liquidate before the duration ends
        borrow_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "token_id",
            vec![],
            8742u128,
        )
        .unwrap();

        repay_helper(
            deps.as_mut(),
            "bad_guy",
            "creator",
            0u64,
            8742u128 + 67u128,
            None,
        )
        .unwrap_err();

        // We allow the querier to deliver response from the vault token
        let err = repay_helper(
            deps.as_mut(),
            "bad_guy",
            "creator",
            0u64,
            8u128,
            Some(765u64),
        )
        .unwrap_err();
        assert_eq!(
            err.downcast::<ContractError>().unwrap(),
            ContractError::CanOnlyLiquidateWholeLoan {}
        );

        let res = repay_helper(
            deps.as_mut(),
            "bad_guy",
            "creator",
            0u64,
            8742u128 + 67u128,
            Some(765u64),
        )
        .unwrap();
        // We print the messages for debugging
        res.messages.iter().for_each(|x| {
            if let CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) = x.msg.clone() {
                println!("{:?}", std::str::from_utf8(msg.as_slice()));
            }
        });

        assert_eq!(
            res,
            Response::new()
                .add_message(
                    into_cosmos_msg(
                        Cw721ExecuteMsg::TransferNft {
                            recipient: "bad_guy".to_string(),
                            token_id: "token_id".to_string(),
                        },
                        "nft".to_string(),
                        None
                    )
                    .unwrap()
                )
                .add_message(
                    into_cosmos_msg(
                        DistributorExecuteMsg::DepositFees {
                            addresses: vec!["nft".to_string()],
                            fee_type: FeeType::Funds
                        },
                        "distributor".to_string(),
                        Some(coins(6u128, "utest"))
                    )
                    .unwrap()
                )
                .add_message(
                    into_cosmos_msg(
                        Cw4626ExecuteMsg::Repay {
                            owner: None,
                            assets: Uint128::from(8803u128)
                        },
                        "vault_token".to_string(),
                        Some(coins(8803u128, "utest"))
                    )
                    .unwrap()
                )
                .add_attribute("action", "repay")
                .add_attribute("caller", "bad_guy")
                .add_attribute("borrower", "creator")
                .add_attribute("assets", 8809u128.to_string())
                .add_attribute("collateral_withdrawn", "true")
        );
    }

    #[test]
    fn test_no_borrow_fixed() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        borrow_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "token_id",
            vec![],
            50_000_000u128,
        )
        .unwrap();

        // You can't borrow too much funds for a unique collateral
        borrow_more_helper(deps.as_mut(), "creator", 0u64, vec![], 50_000_000u128).unwrap_err();

        borrow_more_helper(deps.as_mut(), "creator", 0u64, vec![], 1_000_000u128).unwrap_err();
    }

    #[test]
    fn test_borrow_more_sanity() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        borrow_continuous_helper(
            deps.as_mut(),
            "creator",
            "nft",
            "token_id",
            vec![],
            50_000_000u128,
        )
        .unwrap();

        // You can't borrow too much funds for a unique collateral
        borrow_more_helper(deps.as_mut(), "creator", 0u64, vec![], 50_000_000u128).unwrap_err();

        let res =
            borrow_more_helper(deps.as_mut(), "creator", 0u64, vec![], 1_000_000u128).unwrap();

        assert_eq!(
            res,
            Response::new()
                .add_message(
                    into_cosmos_msg(
                        Cw4626ExecuteMsg::Borrow {
                            receiver: "creator".to_string(),
                            assets: Uint128::from(1_000_000u128),
                        },
                        "vault_token".to_string(),
                        None,
                    )
                    .unwrap()
                )
                .add_attribute("action", "borrow")
                .add_attribute("borrower", "creator")
                .add_attribute("loan_id", "0")
                .add_attribute("asset_borrowed", "1000000")
        );

        // We verify the internal structure has changed
        let borrower = deps.api.addr_validate("creator").unwrap();
        assert_eq!(
            BORROWS
                .load(&deps.storage, (&borrower, 0u64))
                .unwrap()
                .principle,
            Uint128::from(51_000_000u128)
        )
    }
}
