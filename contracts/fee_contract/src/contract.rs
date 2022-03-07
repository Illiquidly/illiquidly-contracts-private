#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, QueryRequest, Response, StdResult,
    WasmQuery,
};

use fee_contract_export::msg::{
    into_cosmos_msg, ExecuteMsg, FeeResponse, InstantiateMsg, QueryMsg,
};
use fee_contract_export::state::ContractInfo;

use p2p_trading_export::msg::{ExecuteMsg as P2PExecuteMsg, QueryMsg as P2PQueryMsg};

use crate::error::ContractError;
use crate::state::CONTRACT_INFO;

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

    // Then we call withdraw on the p2p contract
    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    let withdraw_message = P2PExecuteMsg::WithdrawPendingAssets {
        trader: info.sender.into(),
        trade_id,
    };
    let message = into_cosmos_msg(withdraw_message, contract_info.p2p_contract)?;

    Ok(Response::new()
        .add_attribute("payed", "fee")
        .add_message(message))
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
    use cosmwasm_std::SubMsg;

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
    ) -> Result<Response, ContractError> {
        let info = mock_info(trader, &[]);
        let env = mock_env();

        let res = execute(deps, env, info, ExecuteMsg::PayFeeAndWithdraw { trade_id });
        return res;
    }

    #[test]
    fn test_pay_fee() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());
        let res = pay_fee_helper(deps.as_mut(), "creator", 0).unwrap();
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
