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
use fee_contract_export::state::{ContractInfo, FeeInfo};

use utils::query::{load_accepted_trade, load_trade};

use crate::error::ContractError;
use crate::state::{is_admin, CONTRACT_INFO, FEE_RATES};
use p2p_trading_export::msg::ExecuteMsg as P2PExecuteMsg;
use p2p_trading_export::state::AssetInfo;

const ASSET_FEE_RATE: u128 = 40u128; // In thousands
const FEE_MAX: u128 = 10_000_000u128;
const FIRST_TEER_RATE: u128 = 500_000u128;
const FIRST_TEER_LIMIT: u128 = 4u128;
const SECOND_TEER_RATE: u128 = 200_000u128;
const SECOND_TEER_LIMIT: u128 = 15u128;
const THIRD_TEER_RATE: u128 = 50_000u128;
const ACCEPTABLE_FEE_DEVIATION: u128 = 50u128; // In thousands

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
        owner: msg
            .owner
            .map(|x| deps.api.addr_validate(&x))
            .unwrap_or(Ok(info.sender))?,
        p2p_contract: deps.api.addr_validate(&msg.p2p_contract)?,
        treasury: deps.api.addr_validate(&msg.treasury)?,
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    FEE_RATES.save(
        deps.storage,
        &FeeInfo {
            asset_fee_rate: Uint128::from(ASSET_FEE_RATE), // In thousandths
            fee_max: Uint128::from(FEE_MAX),               // In uusd
            first_teer_limit: Uint128::from(FIRST_TEER_LIMIT),
            first_teer_rate: Uint128::from(FIRST_TEER_RATE),
            second_teer_limit: Uint128::from(SECOND_TEER_LIMIT),
            second_teer_rate: Uint128::from(SECOND_TEER_RATE),
            third_teer_rate: Uint128::from(THIRD_TEER_RATE),
            acceptable_fee_deviation: Uint128::from(ACCEPTABLE_FEE_DEVIATION),
        },
    )?;
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
        ExecuteMsg::UpdateFeeRates {
            asset_fee_rate,
            fee_max,
            first_teer_limit,
            first_teer_rate,
            second_teer_limit,
            second_teer_rate,
            third_teer_rate,
            acceptable_fee_deviation,
        } => update_fee_rates(
            deps,
            env,
            info,
            asset_fee_rate,
            fee_max,
            first_teer_limit,
            first_teer_rate,
            second_teer_limit,
            second_teer_rate,
            third_teer_rate,
            acceptable_fee_deviation
        ),
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
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    let (trade_info, counter_info) =
        load_accepted_trade(deps.as_ref(), contract_info.p2p_contract, trade_id, None)?;

    let fee_amount = Uint128::from(fee_amount_raw(
        deps.as_ref(),
        trade_info.associated_assets,
        counter_info.associated_assets,
    )?);

    let acceptable_fee_deviation = FEE_RATES.load(deps.storage)?.acceptable_fee_deviation;
    if funds.denom == "uusd" {
        if funds.amount + funds.amount*acceptable_fee_deviation/Uint128::from(1_000u128) < fee_amount {
            return Err(ContractError::FeeNotPaidCorrectly {
                required: fee_amount.u128(),
                provided: funds.amount.u128(),
            });
        }
    } else if funds.denom == "uluna" {
        let querier = TerraQuerier::new(&deps.querier);
        let swap_rate: SwapResponse = querier.query_swap(funds, "uusd")?;
        let swap_amount = swap_rate.receive.amount;
        if swap_amount + swap_amount*acceptable_fee_deviation/Uint128::from(1_000u128) < fee_amount {
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

pub fn update_fee_rates(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    asset_fee_rate: Option<Uint128>,
    fee_max: Option<Uint128>,
    first_teer_limit: Option<Uint128>,
    first_teer_rate: Option<Uint128>,
    second_teer_limit: Option<Uint128>,
    second_teer_rate: Option<Uint128>,
    third_teer_rate: Option<Uint128>,
    acceptable_fee_deviation: Option<Uint128>,
) -> Result<Response, ContractError> {
    is_admin(deps.as_ref(), info.sender)?;

    FEE_RATES.update::<_, StdError>(deps.storage, |x| {
        Ok(FeeInfo {
            asset_fee_rate: asset_fee_rate.unwrap_or(x.asset_fee_rate),
            fee_max: fee_max.unwrap_or(x.fee_max),
            first_teer_limit: first_teer_limit.unwrap_or(x.first_teer_limit),
            first_teer_rate: first_teer_rate.unwrap_or(x.first_teer_rate),
            second_teer_limit: second_teer_limit.unwrap_or(x.second_teer_limit),
            second_teer_rate: second_teer_rate.unwrap_or(x.second_teer_rate),
            third_teer_rate: third_teer_rate.unwrap_or(x.third_teer_rate),
            acceptable_fee_deviation: acceptable_fee_deviation.unwrap_or(x.acceptable_fee_deviation),
        })
    })?;

    Ok(Response::new().add_attribute("updated", "fee_rates"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ContractInfo{} => to_binary(&contract_info(deps)?),
        QueryMsg::FeeRates{} => to_binary(&fee_rates(deps)?),
        QueryMsg::Fee {
            trade_id,
            counter_id,
        } => to_binary(&query_fee_for(deps, trade_id, counter_id)?),
        QueryMsg::SimulateFee {
            trade_id,
            counter_assets,
        } => to_binary(&simulate_fee(deps, trade_id, counter_assets)?),
    }
}

pub fn fee_amount_raw(
    deps: Deps,
    trade_assets: Vec<AssetInfo>,
    counter_assets: Vec<AssetInfo>,
) -> StdResult<u128> {
    let fee_info = FEE_RATES.load(deps.storage)?;
    // If you trade one_to_one, there is a fixed 0.5UST fee per peer.
    // Else, there is a 0.2 UST fee per asset per peer, up to 2USD fee
    // Then the fee is 0.1 UST capped to 5 USD
    let querier = TerraQuerier::new(&deps.querier);
    let (asset_number, fund_fee) = trade_assets.iter().chain(counter_assets.iter()).try_fold(
        (0u128, 0u128),
        |(asset_number, fund_fee), x| -> StdResult<(u128,u128)> 
        {
            match x {
                AssetInfo::Coin(coin) => {
                    let usd_value_result = querier.query_swap(coin.clone(), "uusd")?;
                    let ust_equivalent = usd_value_result.receive.amount.u128();
                    let fee = ust_equivalent * fee_info.asset_fee_rate.u128() / 1_000;
                    Ok((asset_number, fund_fee + fee))
                }
                _ => Ok((asset_number + 1, fund_fee)),
            }
        }
    )?;

    let fee = if asset_number <= fee_info.first_teer_limit.u128() {
        asset_number * fee_info.first_teer_rate.u128()
    } else if asset_number <= fee_info.first_teer_limit.u128() {
        fee_info.first_teer_limit.u128() * fee_info.first_teer_rate.u128()
            + (asset_number - fee_info.first_teer_limit.u128()) * fee_info.second_teer_rate.u128()
    } else {
        fee_info.first_teer_limit.u128() * fee_info.first_teer_rate.u128()
            + (fee_info.second_teer_limit.u128() - fee_info.first_teer_limit.u128())
                * fee_info.second_teer_rate.u128()
            + (asset_number - fee_info.second_teer_limit.u128()) * fee_info.third_teer_rate.u128()
    }.min(fee_info.fee_max.u128());

    Ok((fee + fund_fee)/2u128)
}

pub fn contract_info(deps: Deps) -> StdResult<ContractInfo>{
    CONTRACT_INFO.load(deps.storage)
}

pub fn fee_rates(deps: Deps) -> StdResult<FeeInfo>{
    FEE_RATES.load(deps.storage)
}

pub fn query_fee_for(deps: Deps, trade_id: u64, counter_id: Option<u64>) -> StdResult<FeeResponse> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    let (trade_info, counter_info) =
        load_accepted_trade(deps, contract_info.p2p_contract, trade_id, counter_id)?;
    let fee = fee_amount_raw(
        deps,
        trade_info.associated_assets,
        counter_info.associated_assets,
    )?;

    Ok(FeeResponse {
        fee: Uint128::from(fee),
    })
}

pub fn simulate_fee(
    deps: Deps,
    trade_id: u64,
    counter_assets: Vec<AssetInfo>,
) -> StdResult<FeeResponse> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    let trade_info = load_trade(deps, contract_info.p2p_contract, trade_id)?;
    let fee = fee_amount_raw(deps, trade_info.associated_assets, counter_assets)?;

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
            owner: None,
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

    #[test]
    fn test_update_fee_rates() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let info = mock_info("creator", &[]);
        let env = mock_env();
        execute(
            deps.as_mut(),
            env,
            info,
            ExecuteMsg::UpdateFeeRates {
                asset_fee_rate: Some(Uint128::from(5u128)), // In thousandths
                fee_max: Some(Uint128::from(6u128)),        // In uusd
                first_teer_limit: Some(Uint128::from(7u128)),
                first_teer_rate: Some(Uint128::from(8u128)),
                second_teer_limit: Some(Uint128::from(9u128)),
                second_teer_rate: Some(Uint128::from(10u128)),
                third_teer_rate: Some(Uint128::from(11u128)),
                acceptable_fee_deviation: Some(Uint128::from(12u128))
            },
        )
        .unwrap();

        let fee_rate = FEE_RATES.load(&deps.storage).unwrap();
        assert_eq!(
            fee_rate,
            FeeInfo {
                asset_fee_rate: Uint128::from(5u128), // In thousandths
                fee_max: Uint128::from(6u128),        // In uusd
                first_teer_limit: Uint128::from(7u128),
                first_teer_rate: Uint128::from(8u128),
                second_teer_limit: Uint128::from(9u128),
                second_teer_rate: Uint128::from(10u128),
                third_teer_rate: Uint128::from(11u128),
                acceptable_fee_deviation: Uint128::from(12u128),
            }
        );
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
