#[cfg(not(feature = "library"))]
use std::convert::TryInto;

use cosmwasm_std::{
    entry_point, to_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128,
};
use terra_cosmwasm::{SwapResponse, TerraQuerier};

use fee_contract_export::msg::{
    into_cosmos_msg, ExecuteMsg, FeeResponse, InstantiateMsg, QueryMsg,
};
use fee_contract_export::state::ContractInfo;

use utils::query::load_accepted_trade;

use crate::error::ContractError;
use crate::state::CONTRACT_INFO;
use p2p_trading_export::msg::ExecuteMsg as P2PExecuteMsg;

const MINIMUM_FEE_AMOUNT: u128 = 500_000u128;
const FIRST_TEER_RATE: u128 = 200_000u128;
const FIRST_TEER_MAX: u128 = 2_000_000u128;
const SECOND_TEER_RATE: u128 = 100_000u128;
const SECOND_TEER_MAX: u128 = 5_000_000u128;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // Verify the contract name

    msg.validate()?;
    // store token info
    let data = ContractInfo {
        name: msg.name,
        p2p_contract: deps.api.addr_validate(&msg.p2p_contract)?,
        treasury: deps.api.addr_validate(&msg.treasury)?,
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    Ok(Response::default().add_attribute("fee_contract", "init"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::PayFeeAndWithdraw { trade_id } => {
            pay_fee_and_withdraw(deps, env, info, trade_id)
        }
    }
}

pub fn pay_fee_and_withdraw(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
) -> Result<Response, ContractError> {
    // We first pay the fee, either using pre_approved tokens, or funds
    // Here the fee can be paid in luna or ust

    if info.funds.len() != 1 {
        return Err(ContractError::FeeNotPaid {});
    }
    let funds = info.funds[0].clone();
    let fee_amount = Uint128::from(fee_amount_raw(deps.as_ref(), trade_id)?);

    if funds.denom == "uusd" {
        if funds.amount < fee_amount {
            return Err(ContractError::FeeNotPaidCorrectly {
                required: fee_amount.u128(),
                provided: funds.amount.u128(),
            });
        }
    } else if funds.denom == "uluna" {
        let querier = TerraQuerier::new(&deps.querier);
        let swap_rate: SwapResponse = querier.query_swap(funds, "uusd")?;
        let swap_amount = Uint128::from(swap_rate.receive.amount.u128());
        if swap_amount < fee_amount {
            return Err(ContractError::FeeNotPaidCorrectly {
                required: fee_amount.u128(),
                provided: swap_amount.u128(),
            });
        }
    } else {
        return Err(ContractError::FeeNotPaid {});
    }

    // Then we call withdraw on the p2p contract
    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    let withdraw_message = P2PExecuteMsg::WithdrawPendingAssets {
        trader: info.sender.into(),
        trade_id,
    };
    let message = into_cosmos_msg(withdraw_message, contract_info.p2p_contract)?;

    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    let treasury_message = BankMsg::Send {
        to_address: contract_info.treasury.to_string(),
        amount: info.funds,
    };

    Ok(Response::new()
        .add_attribute("payed", "fee")
        .add_message(message)
        .add_message(treasury_message))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Fee { trade_id } => to_binary(&query_fee_for(deps, trade_id)?),
    }
}

pub fn fee_amount_raw(deps: Deps, trade_id: u64) -> StdResult<u128> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    let (trade_info, counter_info) =
        load_accepted_trade(deps, contract_info.p2p_contract, trade_id)?;

    // If you trade one_to_one, there is a fixed 0.5UST fee per peer.
    // Else, there is a 0.2 UST fee per asset per peer, up to 2USD fee
    // Then the fee is 0.1 UST capped to 5 USD

    let asset_number: u128 = (trade_info.associated_assets.len()
        + counter_info.associated_assets.len())
    .try_into()
    .or::<StdError>(Ok(100u128))?;
    let fee = if asset_number == 2 {
        MINIMUM_FEE_AMOUNT
    } else {
        let amount = asset_number * FIRST_TEER_RATE;
        if amount > FIRST_TEER_MAX {
            (asset_number * SECOND_TEER_RATE).max(SECOND_TEER_MAX)
        } else {
            amount
        }
    };

    Ok(fee)
}

pub fn query_fee_for(deps: Deps, trade_id: u64) -> StdResult<FeeResponse> {
    let fee = fee_amount_raw(deps, trade_id)?;

    Ok(FeeResponse {
        fee: Uint128::from(fee),
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    //use cosmwasm_std::{coins, Coin, SubMsg};

    fn init_helper(deps: DepsMut) -> Response {
        let instantiate_msg = InstantiateMsg {
            name: "fee_contract".to_string(),
            p2p_contract: "p2p".to_string(),
            treasury: "treasury".to_string(),
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();

        instantiate(deps, env, info, instantiate_msg).unwrap()
    }

    #[test]
    fn test_init_sanity() {
        let mut deps = mock_dependencies(&[]);
        let res = init_helper(deps.as_mut());
        assert_eq!(0, res.messages.len());
    }

    /*

    fn pay_fee_helper(
        deps: DepsMut,
        trader: &str,
        trade_id: u64,
        c: Vec<Coin>,
    ) -> Result<Response, ContractError> {
        let info = mock_info(trader, &c);
        let env = mock_env();

        let res = execute(deps, env, info, ExecuteMsg::PayFeeAndWithdraw { trade_id });
        return res;
    }
    #[test]
    // Not working because the standard mock querier is shit
    fn test_pay_fee() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());
        let err = pay_fee_helper(deps.as_mut(), "creator", 0, coins(500u128, "uusd")).unwrap_err();
        assert_eq!(
            err,
            ContractError::FeeNotPaidCorrectly {
                required: 500_000u128,
                provided: 500u128,
            }
        );

        let err = pay_fee_helper(deps.as_mut(), "creator", 0, vec![]).unwrap_err();
        assert_eq!(err, ContractError::FeeNotPaid {});

        let res = pay_fee_helper(deps.as_mut(), "creator", 0, coins(500000u128, "uusd")).unwrap();
        assert_eq!(
            res.messages,
            vec![SubMsg::new(
                into_cosmos_msg(
                    P2PExecuteMsg::WithdrawPendingAssets {
                        trader: "creator".to_string(),
                        trade_id: 0
                    },
                    "p2p"
                )
                .unwrap()
            ),]
        );
    }
    */
    /*
    fn query_fee_helper(deps: Deps)-> StdResult<Binary> {
        let env = mock_env();

        let res = query(
            deps,
            env,
             QueryMsg::Fee {
                trade_id: 0,
                counter_id: Some(0),
            },
        );
        return res;

    }
    #[test]
    fn test_query_fee() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());
        query_fee_helper(deps.as_ref()).unwrap();
    }
    */
}
