use anyhow::Result;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, 
};


use lender_export::msg::{ExecuteMsg, QueryMsg, InstantiateMsg};
use lender_export::state::{State, STATE, ContractInfo, CONTRACT_INFO};

use crate::execute::{execute_borrow, execute_repay};
use crate::state::{set_owner, set_oracle, set_lock};


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response> {
    // We start by initating the state of the contract
    let initial_state = State {
        borrow_locked: false
    };

    STATE.save(deps.storage, &initial_state)?;

    // Then the contract info
    let contract_info = ContractInfo {
        name: msg.name,
        oracle: msg.oracle.map(|x| deps.api.addr_validate(&x)).transpose()?.unwrap_or_else(||info.sender.clone()),
        owner: msg.owner.map(|x| deps.api.addr_validate(&x)).transpose()?.unwrap_or(info.sender),
        vault_token: deps.api.addr_validate(&msg.vault_token)?,

    };
    CONTRACT_INFO.save(deps.storage, &contract_info)?;


    Ok(Response::default()
        .add_attribute("action", "init")
        .add_attribute("contract", "lender")
        .add_attribute("owner", contract_info.owner)
    )

}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> Result<Response> {
    match msg {
        ExecuteMsg::Borrow { 
            asset_info,
            wanted_terms,
            principle_slippage
        } => {
            execute_borrow(deps, env, info, asset_info, wanted_terms, principle_slippage).map_err(|x| anyhow::anyhow!(x))
        }
        ExecuteMsg::Repay { 
            borrower,
            loan_id,
            assets
        } => {
            execute_repay(deps, env, info, borrower, loan_id, assets).map_err(|x| anyhow::anyhow!(x))
        }

        // Contract Administration
        ExecuteMsg::SetOwner {owner} => set_owner(deps, info, owner),
        ExecuteMsg::SetOracle {oracle} => set_oracle(deps, info, oracle),
        ExecuteMsg::ToggleLock {lock} => set_lock(deps, info, lock),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::State {} => to_binary(
            &STATE.load(deps.storage)?
        ),
        QueryMsg::ContratInfo {}=> to_binary(
            &CONTRACT_INFO.load(deps.storage)?
        ),
        
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::contract::instantiate;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_env, mock_info
    };
    use cosmwasm_std::{Coin, DepsMut,Uint128, OwnedDeps};
    use cw721::Cw721ExecuteMsg;
    use cw_4626::msg::ExecuteMsg as Cw4626ExecuteMsg;
    use utils::msg::into_cosmos_msg;
    use lender_export::msg::InstantiateMsg;
    use lender_export::state::{ BorrowTerms, Cw721Info, InterestType, FixedInterests};
    use crate::error::ContractError;


    fn init_helper(deps: DepsMut) {
        let instantiate_msg = InstantiateMsg {
            name: "Iliq treasury token".to_string(),
            owner: Some("creator".to_string()),
            oracle: Some("oracle".to_string()),
            vault_token: "vault_token".to_string()
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
        principle: u128
    ) -> Result<Response> {
        let info = mock_info(sender, &assets);
        let env = mock_env();
        execute(
            deps,
            env,
            info,
            ExecuteMsg::Borrow {
                asset_info: Cw721Info{
                    nft_address: nft_address.to_string(),
                    token_id: token_id.to_string()
                },
                wanted_terms: BorrowTerms{
                    principle:  Uint128::from(principle),
                    interests: InterestType::Fixed(FixedInterests{
                        duration: 54u64,
                        interests: Uint128::from(67u128)
                    })
                },
                principle_slippage: Uint128::from(45u128)
            },
        )
    }

    fn repay_helper(deps: DepsMut, address: &str, borrower: &str, loan_id: u64, assets: u128, block_height_delay: Option<u64>) -> Result<Response> {
        let mut env = mock_env();
        if let Some(block_height) = block_height_delay{
            env.block.height += block_height
        }
        
        let info = mock_info(address, &[]);
        execute(
            deps,
            env,
            info,
            ExecuteMsg::Repay{
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
    fn test_borrow_sanity(){
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let err = borrow_helper(deps.as_mut(), "creator","nft","token_id",vec![], 8600u128).unwrap_err();
        assert_eq!(err.downcast::<ContractError>().unwrap(), ContractError::TooMuchSlippage{});

        let res = borrow_helper(deps.as_mut(), "creator","nft","token_id",vec![], 8742u128).unwrap();
        assert_eq!(res, 
            Response::new()
                .add_message(into_cosmos_msg(
                    Cw721ExecuteMsg::TransferNft {
                        recipient: "cosmos2contract".to_string(),
                        token_id: "token_id".to_string(),
                    },
                    "nft".to_string(),
                    None
                ).unwrap())
                .add_message(into_cosmos_msg(
                    Cw4626ExecuteMsg::Borrow{
                        receiver: "creator".to_string(),
                        assets: Uint128::from(8742u128)

                    },
                    "vault_token".to_string(),
                    None
                ).unwrap())
                .add_attribute("action", "borrow")
                .add_attribute("collateral_address","nft")
                .add_attribute("collateral_token_id", "token_id")
                .add_attribute("borrower", "creator")
        );
    }

    #[test]
    fn test_repay_sanity() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        // If the person is not the creator, it can't liquidate before the duration ends
        borrow_helper(deps.as_mut(), "creator","nft","token_id",vec![], 8742u128).unwrap();
        let err = repay_helper(
            deps.as_mut(),
            "bad_guy",
            "creator",
            0u64, 
            8742u128 + 67u128,
            None
        ).unwrap_err();
        assert_eq!(err.downcast::<ContractError>().unwrap(), ContractError::CannotLiquidateBeforeDefault{});

        // If the loan doesn't exist it should return an error
        repay_helper(
            deps.as_mut(),
            "creator",
            "anyone",
            0u64, 
            8742u128 + 67u128,
            None
        ).unwrap_err();

        // We should return the asset to the borrower at then end of the loan
        let res = repay_helper(
            deps.as_mut(),
            "creator",
            "creator",
            0u64, 
            8742u128 + 67u128,
            None
        ).unwrap();
        assert_eq!(res, 
            Response::new()
                .add_message(into_cosmos_msg(
                    Cw721ExecuteMsg::TransferNft {
                        recipient: "creator".to_string(),
                        token_id: "token_id".to_string(),
                    },
                    "nft".to_string(),
                    None
                ).unwrap())
                .add_message(into_cosmos_msg(
                    Cw4626ExecuteMsg::Repay{
                        owner: Some("creator".to_string()),
                        assets: Uint128::from(8809u128)

                    },
                    "vault_token".to_string(),
                    None
                ).unwrap())
                .add_attribute("action", "repay")
                .add_attribute("caller", "creator")
                .add_attribute("borrower", "creator")
                .add_attribute("assets", 8809u128.to_string())
                .add_attribute("collateral_withdrawn","true")
        );
        let err = repay_helper(
            deps.as_mut(),
            "creator",
            "creator",
            0u64, 
            8742u128 + 67u128,
            None
        ).unwrap_err();
        assert_eq!(err.downcast::<ContractError>().unwrap(), ContractError::AssetAlreadyWithdrawn{});
    }

    #[test]
    fn test_liquidate_sanity() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        // If the person is not the creator, it can't liquidate before the duration ends
        borrow_helper(deps.as_mut(), "creator","nft","token_id",vec![], 8742u128).unwrap();
        let env = mock_env();
        println!("{:?}", env);

        repay_helper(
            deps.as_mut(),
            "bad_guy",
            "creator",
            0u64, 
            8742u128 + 67u128,
            None
        ).unwrap_err();


        let err = repay_helper(
            deps.as_mut(),
            "bad_guy",
            "creator",
            0u64, 
            8u128,
            Some(765u64)
        ).unwrap_err();
        assert_eq!(err.downcast::<ContractError>().unwrap(), ContractError::CanOnlyLiquidateWholeLoan{});


        let res = repay_helper(
            deps.as_mut(),
            "bad_guy",
            "creator",
            0u64, 
            8742u128 + 67u128,
            Some(765u64)
        ).unwrap();
        assert_eq!(res, 
            Response::new()
                .add_message(into_cosmos_msg(
                    Cw721ExecuteMsg::TransferNft {
                        recipient: "bad_guy".to_string(),
                        token_id: "token_id".to_string(),
                    },
                    "nft".to_string(),
                    None
                ).unwrap())
                .add_message(into_cosmos_msg(
                    Cw4626ExecuteMsg::Repay{
                        owner: Some("bad_guy".to_string()),
                        assets: Uint128::from(8809u128)

                    },
                    "vault_token".to_string(),
                    None
                ).unwrap())
                .add_attribute("action", "repay")
                .add_attribute("caller", "bad_guy")
                .add_attribute("borrower", "creator")
                .add_attribute("assets", 8809u128.to_string())
                .add_attribute("collateral_withdrawn","true")
        );
    }

}
