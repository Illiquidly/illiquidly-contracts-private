use crate::error::ContractError;
#[cfg(not(feature = "library"))]
use anyhow::{anyhow, Result, bail};
use cosmwasm_std::{
    BankMsg, Coin, DepsMut, Env, MessageInfo, Response, StdResult, Storage, Uint128,
};
use cw20::Cw20ExecuteMsg;
use cw20_base::allowances::execute_burn_from;
use cw20_base::contract::execute_burn;
use cw20_base::state::{BALANCES, TOKEN_INFO};
use cw_4626::state::{AssetInfo, State, STATE};
use utils::msg::into_cosmos_msg;

use crate::query::{convert_to_assets, convert_to_shares};

pub fn deposit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    receiver: String,
    deposit_amount: Uint128,
) -> Result<Response> {
    let state = STATE.load(deps.storage)?;

    // Cannot deposit 0 amount
    if deposit_amount == Uint128::zero() {
        bail!(ContractError::ZeroDeposit {});
    };
    let mint_amount = convert_to_shares(
        deps.as_ref(),
        env.clone(),
        deposit_amount,
        info.funds.get(0).map(|x| x.amount),
    )?;

    // We mint new tokens to the receiver, in return for the deposit
    _execute_mint(deps, receiver.clone(), mint_amount)?;

    // Then we make sure the funds are correctly deposited to the contract
    let res = Response::new();
    let res = match state.underlying_asset {
        AssetInfo::Coin(x) => {
            // We need to check if the funds sent match the AssetInfo we have
            if info.funds.len() != 1 || info.funds[0].denom != x {
                bail!(ContractError::WrongAssetDeposited {
                    sent: info.funds[0].denom.clone(),
                    expected: x,
                });
            } else if deposit_amount != info.funds[0].amount {
                bail!(ContractError::InsufficientAssetDeposited {
                    sent: info.funds[0].amount,
                    expected: deposit_amount,
                });
            }
            res
        }
        AssetInfo::Cw20(x) => {
            // If the vault relies on a CW20, we create a CW20 transferFrom message
            res.add_message(into_cosmos_msg(
                Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.to_string(),
                    recipient: env.contract.address.into(),
                    amount: deposit_amount,
                },
                x,
                None,
            )?)
        }
    };

    Ok(res
        .add_attribute("action", "deposit")
        .add_attribute("caller", info.sender)
        .add_attribute("owner", receiver)
        .add_attribute("assets", deposit_amount.to_string())
        .add_attribute("shares", mint_amount.to_string()))
}

pub fn mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    receiver: String,
    mint_amount: Uint128,
) -> Result<Response> {
    let state = STATE.load(deps.storage)?;

    // Cannot deposit 0 amount
    if mint_amount == Uint128::zero() {
        bail!(ContractError::ZeroDeposit {});
    }

    // Computing the necessary deposit amount to get that number of shares
    let deposit_amount = convert_to_assets(
        deps.as_ref(),
        env.clone(),
        mint_amount,
        info.funds.get(0).map(|x| x.amount),
    )?;

    // We mint new tokens to the receiver, in return for the deposit
    _execute_mint(deps, receiver.clone(), mint_amount)?;

    // Then we make sure the funds are correctly deposited to the contract
    let res = Response::new();
    let res = match state.underlying_asset {
        AssetInfo::Coin(x) => {
            // We need to check if the funds sent match the AssetInfo we have
            if info.funds.len() != 1 || info.funds[0].denom != x {
                bail!(ContractError::WrongAssetDeposited {
                    sent: info.funds[0].denom.clone(),
                    expected: x,
                });
            } else if deposit_amount > info.funds[0].amount {
                bail!(ContractError::InsufficientAssetDeposited {
                    sent: info.funds[0].amount,
                    expected: deposit_amount,
                });
            }
            let cashback_amount = info.funds[0].amount - deposit_amount;
            if cashback_amount > Uint128::zero() {
                res.add_message(BankMsg::Send {
                    to_address: receiver.clone(),
                    amount: vec![Coin {
                        denom: info.funds[0].denom.clone(),
                        amount: cashback_amount,
                    }],
                })
            } else {
                res
            }
        }
        AssetInfo::Cw20(x) => {
            // If the vault relies on a CW20, we create a CW20 transferFrom message
            res.add_message(into_cosmos_msg(
                Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.to_string(),
                    recipient: env.contract.address.into(),
                    amount: deposit_amount,
                },
                x,
                None,
            )?)
        }
    };

    Ok(res
        .add_attribute("action", "mint")
        .add_attribute("caller", info.sender)
        .add_attribute("owner", receiver)
        .add_attribute("assets", deposit_amount.to_string())
        .add_attribute("shares", mint_amount.to_string()))
}

pub fn _add_asset_transfer_message(
    state: &State,
    res: Response,
    receiver: String,
    assets: Uint128,
) -> Result<Response> {
    Ok(match state.underlying_asset.clone() {
        AssetInfo::Coin(x) => res.add_message(BankMsg::Send {
            to_address: receiver,
            amount: vec![Coin {
                denom: x,
                amount: assets,
            }],
        }),
        AssetInfo::Cw20(x) => {
            // If the vault relies on a CW20, we create a CW20 transferFrom message
            res.add_message(into_cosmos_msg(
                Cw20ExecuteMsg::Transfer {
                    recipient: receiver,
                    amount: assets,
                },
                x,
                None,
            )?)
        }
    })
}

pub fn withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: String,
    receiver: String,
    assets: Uint128,
) -> Result<Response> {
    let state = STATE.load(deps.storage)?;
    // Cannot withdraw 0 assets
    if assets == Uint128::zero() {
        bail!(anyhow::anyhow!(ContractError::ZeroDeposit {}));
    }

    let shares_needed = convert_to_shares(deps.as_ref(), env.clone(), assets, None)?;

    // We burn shares_needed token from the owner balance in the info.sender's name
    if info.sender == deps.api.addr_validate(&owner)? {
        execute_burn(deps, env, info.clone(), shares_needed)
    } else {
        execute_burn_from(deps, env, info.clone(), owner.clone(), shares_needed)
    }?;

    // We transfer the underlying asset to the receiver
    let res = _add_asset_transfer_message(&state, Response::new(), receiver.clone(), assets)?;
    Ok(res
        .add_attribute("action", "withdraw")
        .add_attribute("caller", info.sender)
        .add_attribute("receiver", receiver)
        .add_attribute("owner", owner)
        .add_attribute("assets", assets)
        .add_attribute("shares", shares_needed))
}

pub fn redeem(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: String,
    receiver: String,
    shares: Uint128,
) -> Result<Response> {
    let state = STATE.load(deps.storage)?;

    // Cannot withdraw 0 assets
    if shares == Uint128::zero() {
        bail!(anyhow::anyhow!(ContractError::ZeroDeposit {}));
    }

    let assets_needed = convert_to_shares(deps.as_ref(), env.clone(), shares, None)?;

    // We burn shares_needed token from the owner balance in the info.sender's name
    if info.sender == deps.api.addr_validate(&owner)? {
        execute_burn(deps, env, info.clone(), shares)
    } else {
        execute_burn_from(deps, env, info.clone(), owner.clone(), shares)
    }?;

    // We transfer the underlying asset to the reciever
    let res =
        _add_asset_transfer_message(&state, Response::new(), receiver.clone(), assets_needed)?;
    Ok(res
        .add_attribute("action", "redeem")
        .add_attribute("caller", info.sender)
        .add_attribute("receiver", receiver)
        .add_attribute("owner", owner)
        .add_attribute("assets", assets_needed.to_string())
        .add_attribute("shares", shares))
}

/// Mint new tokens without checks (eveyone can mint)
pub fn _execute_mint(
    deps: DepsMut,
    recipient: String,
    amount: Uint128,
) -> Result<()> {
    if amount == Uint128::zero() {
        bail!(ContractError::InvalidZeroAmount {});
    }

    let mut config = TOKEN_INFO.load(deps.storage)?;

    // update supply and enforce cap
    config.total_supply += amount;
    if let Some(limit) = config.get_cap() {
        if config.total_supply > limit {
            bail!(ContractError::CannotExceedCap {});
        }
    }
    TOKEN_INFO.save(deps.storage, &config)?;

    // add amount to recipient balance
    let rcpt_addr = deps.api.addr_validate(&recipient)?;
    BALANCES.update(
        deps.storage,
        &rcpt_addr,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    Ok(())
}

// Borrow mecanism
/// Withdraw some underlying asset, with no repercussion.
/// A configuration error could lead to draining of assets
pub fn borrow(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    receiver: String,
    assets: Uint128,
) -> Result<Response> {
    let mut state = STATE.load(deps.storage)?;
    // Only the authorized address can borrow assets (this usually is a contract address)
    if state.borrower.is_none() || info.sender != state.borrower.clone().unwrap() {
        bail!(anyhow::anyhow!(ContractError::Unauthorized {}));
    }

    // We update the internal state of the contract, more assets were borrowed
    state.total_assets_borrowed += assets;
    STATE.save(deps.storage, &state)?;

    let res = _add_asset_transfer_message(&state, Response::new(), receiver.clone(), assets)?;

    // We send the funds to the receiver
    Ok(res
        .add_attribute("action", "borrower")
        .add_attribute("caller", info.sender)
        .add_attribute("receiver", receiver)
        .add_attribute("assets", assets.to_string()))
}

// Borrow mecanism
/// Repay funds to lower the amount of debt of the contract
/// If more funds than the current debt are sent via this mecanism, assets are deposited in the vault but no extra tokens are minted
pub fn repay(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: Option<String>,
    assets: Uint128,
) -> Result<Response> {
    let debt_repaid = _repay(deps.storage, assets)?;
    let state = STATE.load(deps.storage)?;

    // Then we make sure the funds are correctly deposited to the contract
    let res = Response::new();
    let res = match state.underlying_asset {
        AssetInfo::Coin(x) => {
            // We need to check if the funds sent match the AssetInfo we have
            if info.funds.len() != 1 || info.funds[0].denom != x {
                bail!(anyhow!(ContractError::WrongAssetDeposited {
                    sent: info.funds[0].denom.clone(),
                    expected: x,
                }));
            } else if assets != info.funds[0].amount {
                bail!(anyhow!(ContractError::InsufficientAssetDeposited {
                    sent: info.funds[0].amount,
                    expected: assets,
                }));
            }
            res
        }
        AssetInfo::Cw20(x) => {
            // If the vault relies on a CW20, we create a CW20 transferFrom message
            res.add_message(into_cosmos_msg(
                Cw20ExecuteMsg::TransferFrom {
                    owner: owner.unwrap_or_else(|| info.sender.to_string()),
                    recipient: env.contract.address.into(),
                    amount: assets,
                },
                x,
                None,
            )?)
        }
    };

    Ok(res
        .add_attribute("action", "repay")
        .add_attribute("caller", info.sender)
        .add_attribute("assets", assets.to_string())
        .add_attribute("debt_repaid", debt_repaid.to_string())
        .add_attribute("raw_deposit", (assets - debt_repaid).to_string()))
}

pub fn _repay(storage: &mut dyn Storage, assets: Uint128) -> Result<Uint128> {
    let mut state = STATE.load(storage)?;

    // Cannot deposit 0 amount
    if assets == Uint128::zero() {
        bail!(anyhow!(ContractError::ZeroDeposit {}));
    };
    // Then we update the total debt
    // If the current debt is higher than the repay amount, we repay some of the debt with the deposit
    // Else we repay all the debt and simply deposit the rest in the contract without minting new vault tokens
    let debt_repaid = if state.total_assets_borrowed > assets {
        state.total_assets_borrowed -= assets;
        assets
    } else {
        state.total_assets_borrowed = Uint128::zero();
        state.total_assets_borrowed
    };
    STATE.save(storage, &state)?;
    Ok(debt_repaid)
}
