#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128,
};
use terra_cosmwasm::{SwapResponse, TerraQuerier};

use fee_contract_export::error::ContractError;
use fee_contract_export::msg::{ExecuteMsg, FeeResponse, InstantiateMsg, MigrateMsg, QueryMsg};
use fee_contract_export::state::{ContractInfo, FeeInfo};

use utils::query::{load_trade, load_trade_and_accepted_counter_trade};

use crate::state::{is_admin, CONTRACT_INFO, FEE_RATES};
use fee_distributor_export::msg::ExecuteMsg as FeeDistributorMsg;
use p2p_trading_export::msg::ExecuteMsg as P2PExecuteMsg;
use p2p_trading_export::state::AssetInfo;
use utils::msg::into_cosmos_msg;

const ASSET_FEE_RATE: u128 = 40u128; // In thousands
const FEE_MAX: u128 = 10_000_000u128;
const FIRST_TEER_RATE: u128 = 500_000u128;
const FIRST_TEER_LIMIT: u128 = 4u128;
const SECOND_TEER_RATE: u128 = 200_000u128;
const SECOND_TEER_LIMIT: u128 = 14u128;
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
        fee_distributor: deps.api.addr_validate(&msg.fee_distributor)?,
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    // Initialisation with fixed rates
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
            acceptable_fee_deviation,
        ),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // No state migrations performed, just returned a Response
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::ContractInfo {} => {
            to_binary(&contract_info(deps)?).map_err(|_| ContractError::BinaryEncodingError {})
        }
        QueryMsg::FeeRates {} => {
            to_binary(&fee_rates(deps)?).map_err(|_| ContractError::BinaryEncodingError {})
        }
        QueryMsg::Fee {
            trade_id,
            counter_id,
        } => to_binary(&query_fee_for(deps, trade_id, counter_id)?)
            .map_err(|_| ContractError::BinaryEncodingError {}),
        QueryMsg::SimulateFee {
            trade_id,
            counter_assets,
        } => to_binary(&simulate_fee(deps, trade_id, counter_assets)?)
            .map_err(|_| ContractError::BinaryEncodingError {}),
    }
}

/// This function is used to withdraw funds from an accepted trade.
/// It uses information from the trades and counter trades to determine how much needs to be paid
/// If the fee is sufficient, it sends the fee to the fee_depositor contract (responsible for fee distribution)
pub fn pay_fee_and_withdraw(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
) -> Result<Response, ContractError> {
    // The fee can be paid in any Terra native currency.
    // It needs to be paid in a single currency
    if info.funds.len() != 1 {
        return Err(ContractError::FeeNotPaid {});
    }

    let funds = info.funds[0].clone();
    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    let (trade_info, counter_info) = load_trade_and_accepted_counter_trade(
        deps.as_ref(),
        contract_info.p2p_contract.clone(),
        trade_id,
        None,
    )?;
    // Querying the required fee amount in "uusd"
    let fee_amount = fee_amount_raw(
        deps.as_ref(),
        &trade_info.associated_assets,
        &counter_info.associated_assets,
    )?;
    // We accept a small fee deviation, in case the exchange rates fluctuate a bit between the query and the paiement.
    let acceptable_fee_deviation = FEE_RATES.load(deps.storage)?.acceptable_fee_deviation;

    if funds.denom == "uusd" {
        if funds.amount + funds.amount * acceptable_fee_deviation / Uint128::from(1_000u128)
            < fee_amount
        {
            return Err(ContractError::FeeNotPaidCorrectly {
                required: fee_amount.u128(),
                provided: funds.amount.u128(),
            });
        }
    } else {
        let querier = TerraQuerier::new(&deps.querier);
        let swap_rate: SwapResponse = querier.query_swap(funds, "uusd")?;
        let swap_amount = swap_rate.receive.amount;
        if swap_amount + swap_amount * acceptable_fee_deviation / Uint128::from(1_000u128)
            < fee_amount
        {
            return Err(ContractError::FeeNotPaidCorrectly {
                required: fee_amount.u128(),
                provided: swap_amount.u128(),
            });
        }
    }

    // Then we distribute the funds to the fee_distributor contract
    let contract_addresses: Vec<String> = trade_info
        .associated_assets
        .iter()
        .chain(counter_info.associated_assets.iter())
        .filter_map(|x| match x {
            AssetInfo::Cw721Coin(cw721) => Some(cw721.address.clone()),
            AssetInfo::Cw1155Coin(cw1155) => Some(cw1155.address.clone()),
            _ => None,
        })
        .collect();
    let distribute_message = into_cosmos_msg(
        FeeDistributorMsg::DepositFees {
            addresses: contract_addresses,
        },
        contract_info.fee_distributor,
        Some(info.funds),
    )?;

    // Then we call withdraw on the p2p contract
    let withdraw_message = P2PExecuteMsg::WithdrawPendingAssets {
        trader: info.sender.into(),
        trade_id,
    };
    let message = into_cosmos_msg(withdraw_message, contract_info.p2p_contract, None)?;

    Ok(Response::new()
        .add_attribute("action", "payed_trade_fee")
        .add_message(message)
        .add_message(distribute_message))
}

pub fn modify_contract_owner(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    owner: String,
) -> Result<Response, ContractError> {
    is_admin(deps.as_ref(), info.sender)?;

    let owner_addr = deps.api.addr_validate(&owner)?;
    CONTRACT_INFO.update::<_, StdError>(deps.storage, |mut x| {
        x.owner = owner_addr;
        Ok(x)
    })?;

    Ok(Response::new()
        .add_attribute("action", "parameter_update")
        .add_attribute("parameter", "owner"))
}

#[allow(clippy::too_many_arguments)]
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
            acceptable_fee_deviation: acceptable_fee_deviation
                .unwrap_or(x.acceptable_fee_deviation),
        })
    })?;

    // We verify the rates are ordered
    let new_fee_rates = FEE_RATES.load(deps.storage)?;
    if new_fee_rates.second_teer_limit <= new_fee_rates.first_teer_limit {
        return Err(ContractError::TeersNotOrdered {});
    }

    Ok(Response::new().add_attribute("updated", "fee_rates"))
}

/// Compute the fee amount for trade and counter_trade assets
/// This function contains 2 parts
/// 1. Compute a fee relative to the number of tokens exchanged in the transaction (cw20, cw721 and cw1155)
/// 2. Compute a percentage fee amount for all terra native funds
pub fn fee_amount_raw(
    deps: Deps,
    trade_assets: &[AssetInfo],
    counter_assets: &[AssetInfo],
) -> Result<Uint128, ContractError> {
    let fee_info = FEE_RATES.load(deps.storage)?;

    // Accumulate results to compute
    // 1. The percentage fee for terra native tokens
    // 2. The number of exchanged tokens in the transaction
    let querier = TerraQuerier::new(&deps.querier);
    let (fund_fee, asset_number) = trade_assets.iter().chain(counter_assets.iter()).try_fold(
        (Uint128::zero(), Uint128::zero()),
        |(fund_fee, asset_number), x| -> StdResult<(Uint128, Uint128)> {
            match x {
                AssetInfo::Coin(coin) => {
                    let usd_value = if coin.denom != "uusd" {
                        querier.query_swap(coin.clone(), "uusd")?.receive.amount
                    } else {
                        coin.amount
                    };
                    let fee = usd_value * fee_info.asset_fee_rate / Uint128::from(1_000u128);
                    Ok((fund_fee + fee, asset_number))
                }
                _ => Ok((fund_fee, asset_number + Uint128::from(1u128))),
            }
        },
    )?;

    // We compute the fee dependant on the number of exchanged tokens (in teers, just like taxes)
    let fee = fee_info.first_teer_rate * asset_number.min(fee_info.first_teer_limit)
        + fee_info.second_teer_rate
            * (asset_number
                .min(fee_info.second_teer_limit)
                .max(fee_info.first_teer_limit)
                - fee_info.first_teer_limit)
        + fee_info.third_teer_rate
            * (asset_number.max(fee_info.second_teer_limit) - fee_info.second_teer_limit)
                .min(fee_info.fee_max);

    Ok((fee + fund_fee) / Uint128::from(2u128))
}

pub fn contract_info(deps: Deps) -> StdResult<ContractInfo> {
    CONTRACT_INFO.load(deps.storage)
}

pub fn fee_rates(deps: Deps) -> StdResult<FeeInfo> {
    FEE_RATES.load(deps.storage)
}

/// Allows to simulate the fee that will need to be paid when withdrawing assets
/// If `counter_id` is not specified, the accepted counter_trade will be considered for computing the fee
/// If it is specified, the counter_id provided will be considered
pub fn query_fee_for(
    deps: Deps,
    trade_id: u64,
    counter_id: Option<u64>,
) -> Result<FeeResponse, ContractError> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    let (trade_info, counter_info) = load_trade_and_accepted_counter_trade(
        deps,
        contract_info.p2p_contract,
        trade_id,
        counter_id,
    )?;
    let fee = fee_amount_raw(
        deps,
        &trade_info.associated_assets,
        &counter_info.associated_assets,
    )?;

    Ok(FeeResponse { fee })
}

/// Allows to simulate the fee that will need to be paid if the submitted assets are those of the accepted counter trade
pub fn simulate_fee(
    deps: Deps,
    trade_id: u64,
    counter_assets: Vec<AssetInfo>,
) -> Result<FeeResponse, ContractError> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    let trade_info = load_trade(deps, contract_info.p2p_contract, trade_id)?;
    let fee = fee_amount_raw(deps, &trade_info.associated_assets, &counter_assets)?;

    Ok(FeeResponse { fee })
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use p2p_trading_export::state::Cw20Coin;
    //use cosmwasm_std::{coins, Coin, SubMsg};

    fn init_helper(deps: DepsMut) -> Response {
        let instantiate_msg = InstantiateMsg {
            name: "fee_contract".to_string(),
            owner: None,
            p2p_contract: "p2p".to_string(),
            fee_distributor: "treasury".to_string(),
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
                acceptable_fee_deviation: Some(Uint128::from(12u128)),
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

    #[test]
    fn test_fee_amount() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let fee = fee_amount_raw(deps.as_ref(), &[], &[]).unwrap();
        assert_eq!(fee, Uint128::zero());

        let fee = fee_amount_raw(
            deps.as_ref(),
            &[AssetInfo::Cw20Coin(Cw20Coin {
                amount: Uint128::from(42u64),
                address: "token".to_string(),
            })],
            &[],
        )
        .unwrap();
        assert_eq!(fee, Uint128::new(250_000u128));

        let fee = fee_amount_raw(
            deps.as_ref(),
            &[AssetInfo::Cw20Coin(Cw20Coin {
                amount: Uint128::from(42u64),
                address: "token".to_string(),
            })],
            &[AssetInfo::Cw20Coin(Cw20Coin {
                amount: Uint128::from(42u64),
                address: "token".to_string(),
            })],
        )
        .unwrap();
        assert_eq!(fee, Uint128::new(500_000u128));

        let fee = fee_amount_raw(
            deps.as_ref(),
            &[
                AssetInfo::Cw20Coin(Cw20Coin {
                    amount: Uint128::from(42u64),
                    address: "token".to_string(),
                }),
                AssetInfo::Cw20Coin(Cw20Coin {
                    amount: Uint128::from(42u64),
                    address: "token".to_string(),
                }),
            ],
            &[],
        )
        .unwrap();
        assert_eq!(fee, Uint128::new(500_000u128));
    }
}
