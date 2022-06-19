use anyhow::{anyhow, Result};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};

use cw20_base::allowances::{
    execute_burn_from, execute_decrease_allowance, execute_increase_allowance, execute_send_from,
    execute_transfer_from, query_allowance,
};
use cw20_base::contract::{
    execute_burn, execute_send, execute_transfer, execute_update_marketing, execute_upload_logo,
};
use cw20_base::contract::{
    query_balance, query_download_logo, query_marketing_info, query_minter, query_token_info,
};

use cw20_base::enumerable::{query_all_accounts, query_all_allowances};

use cw20_base::msg::InstantiateMsg as CW20InstantiateMsg;
use cw_4626::msg::{ExecuteMsg, InstantiateMsg};
use cw_4626::query::QueryMsg;
use cw_4626::state::{AssetInfo, State, STATE};

use crate::moving::{_repay, borrow, deposit, mint, redeem, repay, withdraw};
use crate::query::{
    convert_to_assets, convert_to_shares, max_deposit, max_mint, max_redeem, max_withdraw,
    preview_deposit, preview_mint, preview_redeem, preview_withdraw, query_asset,
    query_total_assets,
};

use crate::error::ContractError;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response> {
    // We start by initating the state of the contract
    let borrower = msg
        .borrower
        .map(|x| deps.api.addr_validate(&x))
        .transpose()?;
    let initial_state = State {
        underlying_asset: msg.asset,
        total_underlying_asset_supply: Uint128::zero(),
        total_assets_borrowed: Uint128::zero(),
        borrower,
    };

    STATE.save(deps.storage, &initial_state)?;

    let base_instantiate_msg = CW20InstantiateMsg {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
        initial_balances: vec![],
        mint: msg.mint,
        marketing: msg.marketing,
    };

    cw20_base::contract::instantiate(deps, env, info, base_instantiate_msg).map_err(|x| anyhow!(x))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> Result<Response> {
    match msg {
        ExecuteMsg::Transfer { recipient, amount } => {
            execute_transfer(deps, env, info, recipient, amount).map_err(|x| anyhow!(x))
        }
        ExecuteMsg::Burn { amount } => {
            execute_burn(deps, env, info, amount).map_err(|x| anyhow!(x))
        }
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => execute_send(deps, env, info, contract, amount, msg).map_err(|x| anyhow!(x)),
        ExecuteMsg::IncreaseAllowance {
            spender,
            amount,
            expires,
        } => execute_increase_allowance(deps, env, info, spender, amount, expires)
            .map_err(|x| anyhow!(x)),
        ExecuteMsg::DecreaseAllowance {
            spender,
            amount,
            expires,
        } => execute_decrease_allowance(deps, env, info, spender, amount, expires)
            .map_err(|x| anyhow!(x)),
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => {
            execute_transfer_from(deps, env, info, owner, recipient, amount).map_err(|x| anyhow!(x))
        }
        ExecuteMsg::BurnFrom { owner, amount } => {
            execute_burn_from(deps, env, info, owner, amount).map_err(|x| anyhow!(x))
        }
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            msg,
        } => {
            execute_send_from(deps, env, info, owner, contract, amount, msg).map_err(|x| anyhow!(x))
        }
        ExecuteMsg::UpdateMarketing {
            project,
            description,
            marketing,
        } => execute_update_marketing(deps, env, info, project, description, marketing)
            .map_err(|x| anyhow!(x)),
        ExecuteMsg::UploadLogo(logo) => {
            execute_upload_logo(deps, env, info, logo).map_err(|x| anyhow!(x))
        }

        // CW4626 specific functions
        ExecuteMsg::Deposit { assets, receiver } => {
            deposit(deps, env, info, receiver, assets).map_err(|x| anyhow!(x))
        }
        ExecuteMsg::Mint { shares, receiver } => {
            mint(deps, env, info, receiver, shares).map_err(|x| anyhow!(x))
        }
        ExecuteMsg::Withdraw {
            assets,
            owner,
            receiver,
        } => withdraw(deps, env, info, owner, receiver, assets).map_err(|x| anyhow!(x)),
        ExecuteMsg::Redeem {
            shares,
            owner,
            receiver,
        } => redeem(deps, env, info, owner, receiver, shares).map_err(|x| anyhow!(x)),
        ExecuteMsg::Borrow { assets, receiver } => {
            borrow(deps, env, info, receiver, assets).map_err(|x| anyhow!(x))
        }
        ExecuteMsg::Repay { owner, assets } => {
            repay(deps, env, info, owner, assets).map_err(|x| anyhow!(x))
        }
        ExecuteMsg::Receive {
            sender,
            amount,
            msg,
        } => receive_assets(deps, env, info, sender, amount, msg).map_err(|x| anyhow!(x)),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::TokenInfo {} => to_binary(&query_token_info(deps)?),
        QueryMsg::Minter {} => to_binary(&query_minter(deps)?),
        QueryMsg::Allowance { owner, spender } => {
            to_binary(&query_allowance(deps, owner, spender)?)
        }
        QueryMsg::AllAllowances {
            owner,
            start_after,
            limit,
        } => to_binary(&query_all_allowances(deps, owner, start_after, limit)?),
        QueryMsg::AllAccounts { start_after, limit } => {
            to_binary(&query_all_accounts(deps, start_after, limit)?)
        }
        QueryMsg::MarketingInfo {} => to_binary(&query_marketing_info(deps)?),
        QueryMsg::DownloadLogo {} => to_binary(&query_download_logo(deps)?),

        // 4626 specific functions
        QueryMsg::Asset {} => to_binary(&query_asset(deps)?),
        QueryMsg::TotalAssets {} => to_binary(&query_total_assets(deps)?),
        QueryMsg::ConvertToShares { assets } => {
            to_binary(&convert_to_shares(deps, env, assets, None)?)
        }
        QueryMsg::ConvertToAssets { shares } => {
            to_binary(&convert_to_assets(deps, env, shares, None)?)
        }
        QueryMsg::MaxDeposit { receiver } => to_binary(&max_deposit(deps, env, receiver)?),
        QueryMsg::PreviewDeposit { assets } => to_binary(&preview_deposit(deps, env, assets)?),
        QueryMsg::MaxMint { receiver } => to_binary(&max_mint(deps, env, receiver)?),
        QueryMsg::PreviewMint { shares } => to_binary(&preview_mint(deps, env, shares)?),
        QueryMsg::MaxWithdraw { owner } => to_binary(&max_withdraw(deps, env, owner)?),
        QueryMsg::PreviewWithdraw { assets } => to_binary(&preview_withdraw(deps, env, assets)?),
        QueryMsg::MaxRedeem { owner } => to_binary(&max_redeem(deps, env, owner)?),
        QueryMsg::PreviewRedeem { shares } => to_binary(&preview_redeem(deps, env, shares)?),
    }
}

pub fn receive_assets(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _sender: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response> {
    match from_binary(&msg)? {
        ExecuteMsg::Repay { assets, .. } => {
            let state = STATE.load(deps.storage)?;
            match state.underlying_asset {
                AssetInfo::Cw20(x) => {
                    if x != info.sender {
                        return Err(anyhow!(ContractError::WrongAssetDeposited {
                            sent: info.sender.to_string(),
                            expected: x
                        },));
                    } else if amount != assets {
                        return Err(anyhow!(ContractError::InsufficientAssetDeposited {
                            sent: amount,
                            expected: assets
                        },));
                    }
                    let debt_repaid = _repay(deps.storage, amount)?;

                    Ok(Response::new()
                        .add_attribute("action", "repay")
                        .add_attribute("caller", info.sender)
                        .add_attribute("assets", assets.to_string())
                        .add_attribute("debt_repaid", debt_repaid.to_string())
                        .add_attribute("raw_deposit", (assets - debt_repaid).to_string()))
                }
                AssetInfo::Coin(x) => Err(anyhow!(ContractError::WrongAssetDeposited {
                    sent: info.sender.to_string(),
                    expected: x
                },)),
            }
        }
        _ => Err(anyhow!(ContractError::InvalidMessage {})),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::contract::instantiate;
    use crate::query::convert_to_assets;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_env, mock_info, MockQuerier, MOCK_CONTRACT_ADDR,
    };
    use cosmwasm_std::{coins, from_binary, BankMsg, Coin, DepsMut};
    use cw20::BalanceResponse;
    use cw_4626::msg::InstantiateMsg;
    use cw_4626::state::AssetInfo;

    use rand::Rng;

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

    fn deposit_helper(
        deps: DepsMut,
        address: &str,
        receiver: &str,
        assets: Vec<Coin>,
    ) -> Result<Response> {
        let info = mock_info(address, &assets);
        let env = mock_env();
        execute(
            deps,
            env,
            info,
            ExecuteMsg::Deposit {
                assets: assets[0].amount,
                receiver: receiver.to_string(),
            },
        )
    }

    fn mint_helper(
        deps: DepsMut,
        address: &str,
        receiver: &str,
        assets: Vec<Coin>,
        shares: Uint128,
    ) -> Result<Response> {
        let info = mock_info(address, &assets);
        let env = mock_env();
        execute(
            deps,
            env,
            info,
            ExecuteMsg::Mint {
                shares,
                receiver: receiver.to_string(),
            },
        )
    }

    fn withdraw_helper(
        deps: DepsMut,
        address: &str,
        receiver: &str,
        assets: u128,
    ) -> Result<Response> {
        let info = mock_info(address, &[]);
        let env = mock_env();
        execute(
            deps,
            env,
            info,
            ExecuteMsg::Withdraw {
                assets: Uint128::from(assets),
                owner: address.to_string(),
                receiver: receiver.to_string(),
            },
        )
    }

    fn redeem_helper(
        deps: DepsMut,
        address: &str,
        receiver: &str,
        shares: Uint128,
    ) -> Result<Response> {
        let info = mock_info(address, &[]);
        let env = mock_env();
        execute(
            deps,
            env,
            info,
            ExecuteMsg::Redeem {
                shares,
                owner: address.to_string(),
                receiver: receiver.to_string(),
            },
        )
    }

    fn burn_helper(deps: DepsMut, address: &str, assets: u128) -> Result<Response> {
        let env = mock_env();
        let info = mock_info(address, &[]);
        execute(
            deps,
            env,
            info,
            ExecuteMsg::Burn {
                amount: Uint128::from(assets),
            },
        )
    }

    fn equal_or_not_far_below(init: Uint128, fin: Uint128) -> bool {
        (init == fin) || ((init >= fin) && (init <= fin + Uint128::from(2u128)))
    }

    fn get_share_balance(deps: Deps, env: Env, address: String) -> Uint128 {
        // We need to check that in that specific case of a unique depositor, the underlying funds are unchanged
        let balance = query(deps, env, QueryMsg::Balance { address }).unwrap();

        from_binary::<BalanceResponse>(&balance).unwrap().balance
    }

    fn get_asset_balance(deps: Deps, env: Env, address: String) -> Uint128 {
        // We need to check that in that specific case of a unique depositor, the underlying funds are unchanged
        let share_balance = get_share_balance(deps, env.clone(), address);
        convert_to_assets(deps, env, share_balance, None).unwrap()
    }

    #[test]
    fn test_deposit_sanity() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let mut rng = rand::thread_rng();
        let assets = rng.gen::<u128>();
        let res =
            deposit_helper(deps.as_mut(), "depositor", "nicoco", coins(assets, "uluna")).unwrap();

        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "deposit")
                .add_attribute("caller", "depositor")
                .add_attribute("owner", "nicoco")
                .add_attribute("assets", assets.to_string())
                .add_attribute("shares", assets.to_string())
        );

        // Now we check the internal to be sure it updated
        assert_eq!(
            query_balance(deps.as_ref(), "nicoco".to_string()).unwrap(),
            BalanceResponse {
                balance: Uint128::from(assets)
            }
        );
    }

    #[test]
    fn test_multiple_swaps_sanity() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        init_helper(deps.as_mut());

        let mut rng = rand::thread_rng();
        let assets1 = rng.gen_range(0u128..1000000u128);
        // The funds are sent, we have to update the internal fund balance
        deps.querier = MockQuerier::new(&[(MOCK_CONTRACT_ADDR, &coins(assets1, "uluna"))]);
        deposit_helper(
            deps.as_mut(),
            "depositor",
            "nicoco",
            coins(assets1, "uluna"),
        )
        .unwrap();

        // We burn some of the tokens to make the ratio increase
        burn_helper(deps.as_mut(), "nicoco", assets1 / 2u128).unwrap();

        // We need to check that in that specific case of a unique depositor, the underlying funds are unchanged

        let asset_balance = get_asset_balance(deps.as_ref(), env.clone(), "nicoco".to_string());
        assert!(asset_balance.u128() <= assets1);

        // We check it's impossible (on those 10_000 examples) to mint some free tokens
        for _i in 0..5_000 {
            let initial_assets = Uint128::from(rng.gen_range(0u128..1000000u128));
            let shares =
                convert_to_shares(deps.as_ref(), env.clone(), initial_assets, None).unwrap();
            let final_assets = convert_to_assets(deps.as_ref(), env.clone(), shares, None).unwrap();
            assert!(equal_or_not_far_below(initial_assets, final_assets));
        }

        for _i in 0..5_000 {
            let initial_shares = Uint128::from(rng.gen_range(0u128..1000000u128));
            let assets =
                convert_to_assets(deps.as_ref(), env.clone(), initial_shares, None).unwrap();
            let final_shares = convert_to_shares(deps.as_ref(), env.clone(), assets, None).unwrap();
            assert!(equal_or_not_far_below(initial_shares, final_shares));
        }
    }

    #[test]
    fn test_multiple_deposits_sanity() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        init_helper(deps.as_mut());

        let mut rng = rand::thread_rng();
        let assets1 = rng.gen_range(0u128..1000000u128);
        // The funds are sent, we have to update the internal fund balance
        deps.querier = MockQuerier::new(&[(MOCK_CONTRACT_ADDR, &coins(assets1, "uluna"))]);
        deposit_helper(
            deps.as_mut(),
            "depositor",
            "nicoco",
            coins(assets1, "uluna"),
        )
        .unwrap();

        // We burn some of the tokens to make the ratio increase
        burn_helper(deps.as_mut(), "nicoco", assets1 / 2u128).unwrap();

        let assets2 = rng.gen_range(0u128..1000000u128);
        // The funds are sent, we have to update the internal fund balance
        deps.querier =
            MockQuerier::new(&[(MOCK_CONTRACT_ADDR, &coins(assets1 + assets2, "uluna"))]);
        deposit_helper(
            deps.as_mut(),
            "depositor",
            "nicoco",
            coins(assets2, "uluna"),
        )
        .unwrap();

        let asset_balance = get_asset_balance(deps.as_ref(), env, "nicoco".to_string());
        assert!(
            equal_or_not_far_below(Uint128::from(assets1 + assets2), asset_balance),
            "first: {:?}, second: {:?}",
            asset_balance,
            assets1 + assets2
        );
    }

    #[test]
    fn test_simple_withdraw_sanity() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let mut rng = rand::thread_rng();
        let assets1 = rng.gen_range(0u128..1000000u128);
        // The funds are sent, we have to update the internal fund balance
        deps.querier = MockQuerier::new(&[(MOCK_CONTRACT_ADDR, &coins(assets1, "uluna"))]);
        deposit_helper(
            deps.as_mut(),
            "depositor",
            "nicoco",
            coins(assets1, "uluna"),
        )
        .unwrap();
        let res = withdraw_helper(deps.as_mut(), "nicoco", "depositor", assets1).unwrap();

        assert_eq!(
            res,
            Response::new()
                .add_message(BankMsg::Send {
                    to_address: "depositor".to_string(),
                    amount: coins(assets1, "uluna")
                })
                .add_attribute("action", "withdraw")
                .add_attribute("caller", "nicoco")
                .add_attribute("receiver", "depositor")
                .add_attribute("owner", "nicoco")
                .add_attribute("assets", assets1.to_string())
                .add_attribute("shares", assets1.to_string())
        );
    }

    #[test]
    fn test_mint_sanity() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let mut rng = rand::thread_rng();
        let shares = rng.gen::<u128>();
        let res = mint_helper(
            deps.as_mut(),
            "depositor",
            "nicoco",
            coins(shares, "uluna"),
            Uint128::from(shares),
        )
        .unwrap();

        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "mint")
                .add_attribute("caller", "depositor")
                .add_attribute("owner", "nicoco")
                .add_attribute("assets", shares.to_string())
                .add_attribute("shares", shares.to_string())
        );

        // Now we check the internal to be sure it updated
        assert_eq!(
            query_balance(deps.as_ref(), "nicoco".to_string()).unwrap(),
            BalanceResponse {
                balance: Uint128::from(shares)
            }
        );
    }

    #[test]
    fn test_mint_give_back_sanity() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let mut rng = rand::thread_rng();
        let shares = rng.gen::<u128>();
        let supplemental_assets = 69238u128;
        let res = mint_helper(
            deps.as_mut(),
            "depositor",
            "nicoco",
            coins(shares + supplemental_assets, "uluna"),
            Uint128::from(shares),
        )
        .unwrap();

        assert_eq!(
            res,
            Response::new()
                .add_message(BankMsg::Send {
                    to_address: "nicoco".to_string(),
                    amount: coins(supplemental_assets, "uluna")
                })
                .add_attribute("action", "mint")
                .add_attribute("caller", "depositor")
                .add_attribute("owner", "nicoco")
                .add_attribute("assets", shares.to_string())
                .add_attribute("shares", shares.to_string())
        );

        // Now we check the internal to be sure it updated
        assert_eq!(
            query_balance(deps.as_ref(), "nicoco".to_string()).unwrap(),
            BalanceResponse {
                balance: Uint128::from(shares)
            }
        );
    }

    #[test]
    fn test_multiple_swaps_after_mint_sanity() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        init_helper(deps.as_mut());

        let mut rng = rand::thread_rng();
        let assets1 = rng.gen_range(0u128..1000000u128);
        // The funds are sent, we have to update the internal fund balance
        deps.querier = MockQuerier::new(&[(MOCK_CONTRACT_ADDR, &coins(assets1, "uluna"))]);
        mint_helper(
            deps.as_mut(),
            "depositor",
            "nicoco",
            coins(assets1, "uluna"),
            Uint128::from(assets1),
        )
        .unwrap();

        // We burn some of the tokens to make the ratio increase
        burn_helper(deps.as_mut(), "nicoco", assets1 / 2u128).unwrap();

        // We need to check that in that specific case of a unique depositor, the underlying funds are unchanged

        let asset_balance = get_asset_balance(deps.as_ref(), env.clone(), "nicoco".to_string());
        assert!(asset_balance <= Uint128::from(assets1));

        // We check it's impossible (on those 10_000 examples) to mint some free tokens
        for _i in 0..5_000 {
            let initial_assets = Uint128::from(rng.gen_range(0u128..1000000u128));
            let shares =
                convert_to_shares(deps.as_ref(), env.clone(), initial_assets, None).unwrap();
            let final_assets = convert_to_assets(deps.as_ref(), env.clone(), shares, None).unwrap();
            assert!(equal_or_not_far_below(initial_assets, final_assets));
        }

        for _i in 0..5_000 {
            let initial_shares = Uint128::from(rng.gen_range(0u128..1000000u128));
            let assets =
                convert_to_assets(deps.as_ref(), env.clone(), initial_shares, None).unwrap();
            let final_shares = convert_to_shares(deps.as_ref(), env.clone(), assets, None).unwrap();
            assert!(equal_or_not_far_below(initial_shares, final_shares));
        }
    }

    #[test]
    fn test_multiple_mints_sanity() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        init_helper(deps.as_mut());

        let mut rng = rand::thread_rng();
        let assets1 = rng.gen_range(0u128..1000000u128);
        // The funds are sent, we have to update the internal fund balance
        deps.querier = MockQuerier::new(&[(MOCK_CONTRACT_ADDR, &coins(assets1, "uluna"))]);
        mint_helper(
            deps.as_mut(),
            "depositor",
            "nicoco",
            coins(assets1, "uluna"),
            Uint128::from(assets1),
        )
        .unwrap();

        // We burn some of the tokens to make the ratio increase
        burn_helper(deps.as_mut(), "nicoco", assets1 / 2u128).unwrap();

        // We want to deposit at least some assets
        let shares2 = rng.gen_range(0u128..1000000u128);
        let assets2 = assets1 * shares2 / (assets1 - assets1 / 2);
        println!("{:?} - {:?}", shares2, assets2);

        // The funds are sent, we have to update the internal fund balance
        deps.querier =
            MockQuerier::new(&[(MOCK_CONTRACT_ADDR, &coins(assets1 + assets2, "uluna"))]);
        mint_helper(
            deps.as_mut(),
            "depositor",
            "nicoco",
            coins(assets2, "uluna"),
            Uint128::from(shares2),
        )
        .unwrap();

        let asset_balance = get_asset_balance(deps.as_ref(), env, "nicoco".to_string());
        assert!(
            equal_or_not_far_below(Uint128::from(assets1 + assets2), asset_balance),
            "first: {:?}, second: {:?}",
            asset_balance,
            assets1 + assets2
        );
    }

    #[test]
    fn test_simple_redeem_sanity() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let mut rng = rand::thread_rng();
        let assets1 = rng.gen_range(0u128..1000000u128);
        // The funds are sent, we have to update the internal fund balance
        deps.querier = MockQuerier::new(&[(MOCK_CONTRACT_ADDR, &coins(assets1, "uluna"))]);
        mint_helper(
            deps.as_mut(),
            "depositor",
            "nicoco",
            coins(assets1, "uluna"),
            Uint128::from(assets1),
        )
        .unwrap();
        let res =
            redeem_helper(deps.as_mut(), "nicoco", "depositor", Uint128::from(assets1)).unwrap();

        assert_eq!(
            res,
            Response::new()
                .add_message(BankMsg::Send {
                    to_address: "depositor".to_string(),
                    amount: coins(assets1, "uluna")
                })
                .add_attribute("action", "redeem")
                .add_attribute("caller", "nicoco")
                .add_attribute("receiver", "depositor")
                .add_attribute("owner", "nicoco")
                .add_attribute("assets", assets1.to_string())
                .add_attribute("shares", assets1.to_string())
        );
    }
}
