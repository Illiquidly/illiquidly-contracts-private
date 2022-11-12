use crate::state::get_offer;
use crate::state::lender_offers;
use crate::state::BORROWER_INFO;
use crate::state::COLLATERAL_INFO;
use cosmwasm_std::StdError;
use cosmwasm_std::{Deps, Order, StdResult};
use cw_storage_plus::Bound;
use nft_loans_export::msg::CollateralResponse;
use nft_loans_export::msg::MultipleCollateralsAllResponse;
use nft_loans_export::msg::MultipleCollateralsResponse;
use nft_loans_export::msg::MultipleOffersResponse;
use nft_loans_export::msg::OfferResponse;
use nft_loans_export::state::BorrowerInfo;
use nft_loans_export::state::CollateralInfo;
#[cfg(not(feature = "library"))]
use nft_loans_export::state::ContractInfo;

use crate::state::CONTRACT_INFO;
use anyhow::{anyhow, Result};
// settings for pagination
const MAX_QUERY_LIMIT: u32 = 30;
const DEFAULT_QUERY_LIMIT: u32 = 10;

/*
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct TradeResponse {
    pub trade_id: u64,
    pub counter_id: Option<u64>,
    pub trade_info: TradeInfo,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct AllTradesResponse {
    pub trades: Vec<TradeResponse>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct AllCounterTradesResponse {
    pub counter_trades: Vec<TradeResponse>,
}

pub fn query_contract_info(deps: Deps) -> StdResult<ContractInfo> {
    CONTRACT_INFO.load(deps.storage)
}

// parse trades to human readable format
fn parse_trades(_: &dyn Api, item: StdResult<Pair<TradeInfo>>) -> StdResult<TradeResponse> {
    item.map(|(k, trade)| {
        let trade_id = k.try_into().unwrap();
        TradeResponse {
            trade_id: u64::from_be_bytes(trade_id),
            counter_id: None,
            trade_info: trade,
        }
    })
}

pub fn loan_filter(
    api: &dyn Api,
    trade_info: &StdResult<TradeResponse>,
    filters: &Option<QueryFilters>,
) -> bool {
    if let Some(filters) = filters {
        let trade = trade_info.as_ref().unwrap();

        (match &filters.states {
            Some(state) => state.contains(&trade.trade_info.state.to_string()),
            None => true,
        } && match &filters.owner {
            Some(owner) => trade.trade_info.owner == owner.clone(),
            None => true,
        } && match &filters.whitelisted_user {
            Some(whitelisted_user) => trade
                .trade_info
                .whitelisted_users
                .contains(&api.addr_validate(whitelisted_user).unwrap()),
            None => true,
        } && match &filters.wanted_nft {
            Some(wanted_nft) => trade
                .trade_info
                .additionnal_info
                .nfts_wanted
                .contains(&api.addr_validate(wanted_nft).unwrap()),
            None => true,
        } && match &filters.contains_token {
            Some(token) => trade
                .trade_info
                .associated_assets
                .iter()
                .any(|asset| match asset {
                    AssetInfo::Cw20Coin(x) => x.address == token.as_ref(),
                    AssetInfo::Cw721Coin(x) => x.address == token.as_ref(),
                    AssetInfo::Cw1155Coin(x) => x.address == token.as_ref(),
                }),
            None => true,
        })
    } else {
        true
    }
}

pub fn query_all_trades(
    deps: Deps,
    start_after: Option<u64>,
    limit: Option<u32>,
    filters: Option<QueryFilters>,
) -> StdResult<AllTradesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(|s| Bound::Exclusive(U64Key::new(s).joined_key()));

    let trades: StdResult<Vec<TradeResponse>> = TRADE_INFO
        .range(deps.storage, None, start, Order::Descending)
        .map(|kv_item| parse_trades(deps.api, kv_item))
        .filter(|response| loan_filter(deps.api, response, &filters))
        .take(limit)
        .collect();

    Ok(AllTradesResponse { trades: trades? })
}

// parse counter trades to human readable format
fn parse_all_counter_trades(
    _: &dyn Api,
    item: StdResult<Pair<TradeInfo>>,
) -> StdResult<TradeResponse> {
    item.map(|(ck, trade)| {
        // First two bytes define size [0,8] since we know it's u64 skip it.
        let (trade_id, counter_id) = (&ck[2..10], &ck[10..]);
        let trade_id = trade_id.try_into().unwrap();
        let counter_id = counter_id.try_into().unwrap();

        TradeResponse {
            trade_id: u64::from_be_bytes(trade_id),
            counter_id: Some(u64::from_be_bytes(counter_id)),
            trade_info: trade,
        }
    })
}

pub fn query_all_counter_trades(
    deps: Deps,
    start_after: Option<CounterTradeInfo>,
    limit: Option<u32>,
    filters: Option<QueryFilters>,
) -> StdResult<AllCounterTradesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let start = start_after.map(|s| {
        Bound::Exclusive((U64Key::new(s.trade_id), U64Key::new(s.counter_id)).joined_key())
    });

    let counter_trades: StdResult<Vec<TradeResponse>> = COUNTER_TRADE_INFO
        .range(deps.storage, None, start, Order::Descending)
        .map(|kv_item| parse_all_counter_trades(deps.api, kv_item))
        .filter(|response| trade_filter(deps.api, response, &filters))
        .take(limit)
        .collect();

    Ok(AllCounterTradesResponse {
        counter_trades: counter_trades?,
    })
}

// parse counter trades to human readable format
fn parse_counter_trades(
    _: &dyn Api,
    item: StdResult<Pair<TradeInfo>>,
    trade_id: Vec<u8>,
) -> StdResult<TradeResponse> {
    item.map(|(counter_id, trade)| {
        let trade_id = trade_id.try_into().unwrap();
        let counter_id = counter_id.try_into().unwrap();

        TradeResponse {
            trade_id: u64::from_be_bytes(trade_id),
            counter_id: Some(u64::from_be_bytes(counter_id)),
            trade_info: trade,
        }
    })
}

pub fn query_counter_trades(deps: Deps, trade_id: u64) -> StdResult<AllCounterTradesResponse> {
    let counter_trades: StdResult<Vec<TradeResponse>> = COUNTER_TRADE_INFO
        .prefix(trade_id.into())
        .range(deps.storage, None, None, Order::Descending)
        .map(|kv_item| parse_counter_trades(deps.api, kv_item, U64Key::new(trade_id).joined_key()))
        .collect();

    Ok(AllCounterTradesResponse {
        counter_trades: counter_trades?,
    })
}


*/
// TODO we need more queries, to query loan by user
pub fn query_contract_info(deps: Deps) -> Result<ContractInfo> {
    CONTRACT_INFO.load(deps.storage).map_err(|err| anyhow!(err))
}

pub fn query_collateral_info(deps: Deps, borrower: String, loan_id: u64) -> Result<CollateralInfo> {
    let borrower = deps.api.addr_validate(&borrower)?;
    COLLATERAL_INFO
        .load(deps.storage, (borrower, loan_id))
        .map_err(|err| anyhow!(err))
}

pub fn query_offer_info(deps: Deps, global_offer_id: String) -> Result<OfferResponse> {
    let offer_info = get_offer(deps.storage, &global_offer_id)?;

    Ok(OfferResponse {
        global_offer_id,
        offer_info,
    })
}

pub fn query_borrower_info(deps: Deps, borrower: String) -> StdResult<BorrowerInfo> {
    let borrower = deps.api.addr_validate(&borrower)?;
    BORROWER_INFO
        .load(deps.storage, &borrower)
        .map_err(|_| StdError::generic_err("UnknownBorrower"))
}

pub fn query_collaterals(
    deps: Deps,
    borrower: String,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> Result<MultipleCollateralsResponse> {
    let borrower = deps.api.addr_validate(&borrower)?;
    let limit = limit.unwrap_or(DEFAULT_QUERY_LIMIT).min(MAX_QUERY_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    let collaterals: Vec<CollateralResponse> = COLLATERAL_INFO
        .prefix(borrower.clone())
        .range(deps.storage, None, start, Order::Descending)
        .map(|result| {
            result
                .map(|(loan_id, el)| CollateralResponse {
                    borrower: borrower.to_string(),
                    loan_id,
                    collateral: el,
                })
                .map_err(|err| anyhow!(err))
        })
        .take(limit)
        .collect::<Result<Vec<CollateralResponse>>>()?;

    Ok(MultipleCollateralsResponse {
        next_collateral: if collaterals.len() == limit {
            collaterals.last().map(|last| last.loan_id)
        } else {
            None
        },
        collaterals,
    })
}

pub fn query_all_collaterals(
    deps: Deps,
    start_after: Option<(String, u64)>,
    limit: Option<u32>,
) -> Result<MultipleCollateralsAllResponse> {
    let limit = limit.unwrap_or(DEFAULT_QUERY_LIMIT).min(MAX_QUERY_LIMIT) as usize;
    let start = start_after
        .map::<Result<Bound<_>>, _>(|start_after| {
            let borrower = deps.api.addr_validate(&start_after.0)?;
            Ok(Bound::exclusive((borrower, start_after.1)))
        })
        .transpose()?;

    let collaterals: Vec<CollateralResponse> = COLLATERAL_INFO
        .range(deps.storage, None, start, Order::Descending)
        .map(|result| {
            result
                .map(|(loan_id, el)| CollateralResponse {
                    borrower: loan_id.0.to_string(),
                    loan_id: loan_id.1,
                    collateral: el,
                })
                .map_err(|err| anyhow!(err))
        })
        .take(limit)
        .collect::<Result<Vec<CollateralResponse>>>()?;

    Ok(MultipleCollateralsAllResponse {
        next_collateral: collaterals
            .last()
            .map(|last| (last.borrower.clone(), last.loan_id)),
        collaterals,
    })
}

pub fn query_offers(
    deps: Deps,
    borrower: String,
    loan_id: u64,
    start_after: Option<String>,
    limit: Option<u32>,
) -> Result<MultipleOffersResponse> {
    let borrower = deps.api.addr_validate(&borrower)?;
    let limit = limit.unwrap_or(DEFAULT_QUERY_LIMIT).min(MAX_QUERY_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    let offers: Vec<OfferResponse> = lender_offers()
        .idx
        .loan
        .prefix((borrower, loan_id))
        .range(deps.storage, None, start, Order::Descending)
        .map(|x| {
            x.map(|(key, offer_info)| OfferResponse {
                offer_info,
                global_offer_id: key,
            })
            .map_err(|err| anyhow!(err))
        })
        .take(limit)
        .collect::<Result<Vec<OfferResponse>>>()?;

    Ok(MultipleOffersResponse {
        next_offer: offers.last().map(|last| last.global_offer_id.clone()),
        offers,
    })
}

pub fn query_lender_offers(
    deps: Deps,
    lender: String,
    start_after: Option<String>,
    limit: Option<u32>,
) -> Result<MultipleOffersResponse> {
    let lender = deps.api.addr_validate(&lender)?;
    let limit = limit.unwrap_or(DEFAULT_QUERY_LIMIT).min(MAX_QUERY_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    let offers: Vec<OfferResponse> = lender_offers()
        .idx
        .lender
        .prefix(lender)
        .range(deps.storage, None, start, Order::Descending)
        .map(|x| {
            x.map(|(key, offer_info)| OfferResponse {
                offer_info,
                global_offer_id: key,
            })
            .map_err(|err| anyhow!(err))
        })
        .take(limit)
        .collect::<Result<Vec<OfferResponse>>>()?;

    Ok(MultipleOffersResponse {
        next_offer: offers.last().map(|last| last.global_offer_id.clone()),
        offers,
    })
}

/*
QueryMsg::LenderOffers { lender, start_after, limit } => {
    to_anyhow_binary(&query_lender_offers(deps, lender, start_after, limit)?)
}
*/
