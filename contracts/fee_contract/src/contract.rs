#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, QueryRequest, Response, StdResult,
    WasmQuery, Uint128
};
use terra_cosmwasm::{ TerraQuerier, SwapResponse };


use fee_contract_export::msg::{
    into_cosmos_msg, ExecuteMsg, FeeResponse, InstantiateMsg, QueryMsg,
};
use fee_contract_export::state::ContractInfo;

use p2p_trading_export::msg::{ExecuteMsg as P2PExecuteMsg, QueryMsg as P2PQueryMsg};

use crate::error::ContractError;
use crate::state::CONTRACT_INFO;


const FIXED_FEE_AMOUNT: u64 = 500_000u64;

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
    // Here the fee can be paid in luna or ust and will cost 0.5 UST in total anyway
    if info.funds.len() != 1{
        return Err(ContractError::FeeNotPaid{});
    }
    let funds = info.funds[0].clone();
    let fee_amount = Uint128::from(FIXED_FEE_AMOUNT);
    if funds.denom == "uusd"{
        if funds.amount < fee_amount{
            return Err(ContractError::FeeNotPaidCorrectly{required: fee_amount.u128(), provided: funds.amount.u128()});
        }
    }else if funds.denom == "uluna"{
        let querier = TerraQuerier::new(&deps.querier);
        let swap_rate: SwapResponse = querier.query_swap(funds.clone(), "uusd")?;
        let swap_amount = Uint128::from(swap_rate.receive.amount.u128());
        if swap_amount < fee_amount{
            return Err(ContractError::FeeNotPaidCorrectly{required: fee_amount.u128(), provided: swap_amount.u128()});
        }
    }else{
        return Err(ContractError::FeeNotPaid{});
    }


    // Then we call withdraw on the p2p contract
    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    let withdraw_message = P2PExecuteMsg::WithdrawPendingAssets {
        trader: info.sender.into(),
        trade_id,
    };
    let message = into_cosmos_msg(withdraw_message, contract_info.p2p_contract)?;

    Ok(Response::new()
        .add_attribute("payed", "fee")
        .add_message(message)
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Fee {
            trade_id,
            counter_id,
        } => to_binary(&query_fee_for(deps, trade_id, counter_id)?),
    }
}

pub fn query_fee_for(deps: Deps, trade_id: u64, counter_id: Option<u64>) -> StdResult<FeeResponse> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    let _trade_info = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: contract_info.p2p_contract.to_string(),
        msg: to_binary(&P2PQueryMsg::TradeInfo { trade_id })?,
    }))?;

    if let Some(counter_id) = counter_id {
        let _counter_info = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: contract_info.p2p_contract.to_string(),
            msg: to_binary(&P2PQueryMsg::CounterTradeInfo {
                trade_id,
                counter_id,
            })?,
        }))?;
    }

    Ok(FeeResponse {
        fee: "none".to_string(),
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{SubMsg, Coin, coins};

    fn init_helper(deps: DepsMut) -> Response {
        let instantiate_msg = InstantiateMsg {
            name: "fee_contract".to_string(),
            p2p_contract: "p2p".to_string(),
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

    fn pay_fee_helper(
        deps: DepsMut,
        trader: &str,
        trade_id: u64,
        c: Vec<Coin>
    ) -> Result<Response, ContractError> {
        let info = mock_info(trader, &c);
        let env = mock_env();

        let res = execute(deps, env, info, ExecuteMsg::PayFeeAndWithdraw { trade_id });
        return res;
    }

    #[test]
    fn test_pay_fee() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());
        let err = pay_fee_helper(deps.as_mut(), "creator", 0, coins(500u128, "uusd")).unwrap_err();
        assert_eq!(err, ContractError::FeeNotPaidCorrectly{
            required: 500_000u128,
            provided: 500u128,
        });

        let err = pay_fee_helper(deps.as_mut(), "creator", 0, vec![]).unwrap_err();
        assert_eq!(err, ContractError::FeeNotPaid{});

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
    /*
    // Not working because the standard mock querier is shit
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
