use cosmwasm_std::{to_binary, Addr, Deps, QueryRequest, StdError, StdResult, WasmQuery};

use crate::msg::TradeInfoResponse;
use p2p_trading_export::msg::QueryMsg as P2PQueryMsg;
use p2p_trading_export::state::TradeInfo;

pub fn load_accepted_trade(
    deps: Deps,
    p2p_contract: Addr,
    trade_id: u64,
) -> StdResult<(TradeInfo, TradeInfo)> {
    let trade_info: TradeInfo = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: p2p_contract.to_string(),
        msg: to_binary(&P2PQueryMsg::TradeInfo { trade_id })?,
    }))?;

    let counter_id = trade_info
        .clone()
        .accepted_info
        .ok_or_else(|| StdError::generic_err("Trade not accepted"))?
        .counter_id;

    let counter_info_response: TradeInfoResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: p2p_contract.to_string(),
            msg: to_binary(&P2PQueryMsg::CounterTradeInfo {
                trade_id,
                counter_id,
            })?,
        }))?;

    let counter_info = counter_info_response.trade_info;

    Ok((trade_info, counter_info))
}
