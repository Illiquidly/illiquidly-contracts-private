use cosmwasm_std::{
    to_binary, Addr, BalanceResponse, BankQuery, Deps, Env, QueryRequest, StdError, StdResult,
    Uint128, WasmQuery,
};
use cw20::{Cw20QueryMsg, TokenInfoResponse};
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const STATE: Item<State> = Item::new("contract_state");

/// EIP specific 4626 info
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct State {
    pub underlying_asset: AssetInfo,
    pub total_underlying_asset_supply: Uint128,
    pub total_assets_borrowed: Uint128,
    pub borrower: Option<Addr>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct MinterData {
    pub minter: Addr,
    /// cap is how many more tokens can be issued by the minter
    pub cap: Option<Uint128>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfo {
    Coin(String),
    Cw20(String),
}

pub fn query_asset_balance(deps: Deps, env: Env) -> Result<Uint128, StdError> {
    let state = STATE.load(deps.storage)?;

    match state.underlying_asset {
        AssetInfo::Coin(denom) => query_fund_balance(deps, env.contract.address, denom),
        AssetInfo::Cw20(address) => query_cw20_supply(deps, deps.api.addr_validate(&address)?),
    }
}

pub fn query_asset_liabilities(deps: Deps, _env: Env) -> Result<Uint128, StdError> {
    let state = STATE.load(deps.storage)?;
    Ok(state.total_assets_borrowed)
}

pub fn query_fund_balance(deps: Deps, account_addr: Addr, denom: String) -> StdResult<Uint128> {
    // load price form the oracle
    let balance: BalanceResponse = deps.querier.query(&QueryRequest::Bank(BankQuery::Balance {
        address: account_addr.to_string(),
        denom,
    }))?;
    Ok(balance.amount.amount)
}

pub fn query_cw20_supply(deps: Deps, contract_addr: Addr) -> StdResult<Uint128> {
    // load price form the oracle
    let token_info: TokenInfoResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: contract_addr.to_string(),
            msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
        }))?;

    Ok(token_info.total_supply)
}
