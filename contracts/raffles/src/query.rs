use anyhow::Result;
#[cfg(not(feature = "library"))]
use cosmwasm_std::{Addr, Api, Deps, Env, Order, StdResult};

use cw_storage_plus::Bound;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{get_raffle_state, CONTRACT_INFO, RAFFLE_INFO};
use raffles_export::msg::QueryFilters;
use raffles_export::state::{AssetInfo, ContractInfo, RaffleInfo};

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;
const BASE_LIMIT: usize = 100;

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct RaffleResponse {
    pub raffle_id: u64,
    pub raffle_info: Option<RaffleInfo>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct AllRafflesResponse {
    pub raffles: Vec<RaffleResponse>,
}

pub fn query_contract_info(deps: Deps) -> StdResult<ContractInfo> {
    CONTRACT_INFO.load(deps.storage)
}

/*

         QueryMsg::GetTickets {
            raffle_id,
            start_after,
            limit,
            filters,
        } => query_tickets(deps, env, raffle_id, start_after, limit, filters),
         QueryMsg::GetAllTickets {
            start_after,
            limit,
            filters,
        } => query_all_tickets(deps, env, start_after, limit, filters),

*/

// parse raffles to human readable format
fn parse_raffles(_: &dyn Api, item: StdResult<(u64, RaffleInfo)>) -> StdResult<RaffleResponse> {
    item.map(|(raffle_id, raffle)| RaffleResponse {
        raffle_id,
        raffle_info: Some(raffle),
    })
}

pub fn raffle_filter(
    _api: &dyn Api,
    env: Env,
    raffle_info: &StdResult<RaffleResponse>,
    filters: &Option<QueryFilters>,
) -> bool {
    if let Some(filters) = filters {
        let raffle = raffle_info.as_ref().unwrap();

        (match &filters.states {
            Some(state) => state
                .contains(&get_raffle_state(env, raffle.raffle_info.clone().unwrap()).to_string()),
            None => true,
        } && match &filters.owner {
            Some(owner) => raffle.raffle_info.as_ref().unwrap().owner == owner.clone(),
            None => true,
        } && match &filters.ticket_depositor {
            Some(ticket_depositor) => raffle
                .raffle_info
                .as_ref()
                .unwrap()
                .tickets
                .contains(ticket_depositor),
            None => true,
        } && match &filters.contains_token {
            Some(token) => match raffle.raffle_info.clone().unwrap().asset {
                AssetInfo::Coin(x) => x.denom == token.as_ref(),
                AssetInfo::Cw20Coin(x) => x.address == token.as_ref(),
                AssetInfo::Cw721Coin(x) => x.address == token.as_ref(),
                AssetInfo::Cw1155Coin(x) => x.address == token.as_ref(),
            },
            None => true,
        })
    } else {
        true
    }
}

pub fn query_ticket_number(
    deps: Deps,
    _env: Env,
    raffle_id: u64,
    ticket_depositor: Addr,
) -> Result<u64> {
    let raffle_info = RAFFLE_INFO.load(deps.storage, raffle_id)?;
    Ok(raffle_info
        .tickets
        .iter()
        .filter(|&t| *t == ticket_depositor)
        .count() as u64)
}
pub fn query_all_raffles(
    deps: Deps,
    env: Env,
    start_after: Option<u64>,
    limit: Option<u32>,
    filters: Option<QueryFilters>,
) -> StdResult<AllRafflesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    let mut raffles: Vec<RaffleResponse> = RAFFLE_INFO
        .range(deps.storage, None, start.clone(), Order::Descending)
        .take(BASE_LIMIT)
        .map(|kv_item| parse_raffles(deps.api, kv_item))
        .filter(|response| raffle_filter(deps.api, env.clone(), response, &filters))
        .take(limit)
        .collect::<StdResult<Vec<RaffleResponse>>>()?;

    if raffles.is_empty() {
        let raffle_id = RAFFLE_INFO
            .keys(deps.storage, None, start, Order::Descending)
            .take(BASE_LIMIT)
            .last();

        if let Some(Ok(raffle_id)) = raffle_id {
            if raffle_id != 0 {
                raffles = vec![RaffleResponse {
                    raffle_id,
                    raffle_info: None,
                }]
            }
        }
    }
    Ok(AllRafflesResponse { raffles })
}
