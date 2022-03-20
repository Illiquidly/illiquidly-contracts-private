#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128,
};

use crate::error::ContractError;

use crate::state::{
    is_counter_trader, is_fee_contract, is_owner, is_trader, load_counter_trade, load_trade,
    CONTRACT_INFO, COUNTER_TRADE_INFO, TRADE_INFO,
};
use p2p_trading_export::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use p2p_trading_export::state::{ContractInfo, TradeInfo, TradeState};

use crate::counter_trade::{
    add_cw1155_to_counter_trade, add_cw20_to_counter_trade, add_cw721_to_counter_trade,
    add_funds_to_counter_trade, cancel_counter_trade, confirm_counter_trade, suggest_counter_trade,
    withdraw_counter_trade_assets_while_creating,
};
use crate::trade::{
    accept_trade, add_cw1155_to_trade, add_cw20_to_trade, add_cw721_to_trade, add_funds_to_trade,
    add_nfts_wanted, add_whitelisted_users, cancel_trade, confirm_trade, create_trade,
    create_withdraw_messages, refuse_counter_trade, remove_nfts_wanted, remove_whitelisted_users,
    withdraw_trade_assets_while_creating,
};

use crate::messages::{review_counter_trade, set_comment};
use crate::query::{
    query_all_counter_trades, query_all_trades, query_contract_info, query_counter_trades,
};

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
        owner: deps
            .api
            .addr_validate(&msg.owner.unwrap_or_else(|| info.sender.to_string()))?,
        fee_contract: None,
        last_trade_id: None,
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    Ok(Response::default().add_attribute("p2p-contract", "init"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        // Trade Creation Messages
        ExecuteMsg::CreateTrade {
            whitelisted_users,
            comment,
        } => create_trade(deps, env, info, whitelisted_users, comment),
        ExecuteMsg::AddFundsToTrade { trade_id } => add_funds_to_trade(deps, env, info, trade_id),
        ExecuteMsg::AddCw20 {
            trade_id,
            counter_id,
            address,
            amount,
            to_last_trade,
            to_last_counter,
        } => add_cw20(
            deps,
            env,
            info.sender,
            trade_id,
            counter_id,
            address,
            amount,
            to_last_trade,
            to_last_counter,
        ),

        ExecuteMsg::AddCw721 {
            trade_id,
            counter_id,
            address,
            token_id,
            to_last_trade,
            to_last_counter,
        } => add_cw721(
            deps,
            env,
            info.sender,
            trade_id,
            counter_id,
            address,
            token_id,
            to_last_trade,
            to_last_counter,
        ),
        ExecuteMsg::AddCw1155 {
            trade_id,
            counter_id,
            address,
            token_id,
            value,
            to_last_trade,
            to_last_counter,
        } => add_cw1155(
            deps,
            env,
            info.sender,
            trade_id,
            counter_id,
            address,
            token_id,
            value,
            to_last_trade,
            to_last_counter,
        ),
        ExecuteMsg::RemoveFromTrade {
            trade_id,
            assets,
            funds,
        } => withdraw_trade_assets_while_creating(deps, env, info, trade_id, assets, funds),

        ExecuteMsg::AddWhitelistedUsers {
            trade_id,
            whitelisted_users,
        } => add_whitelisted_users(
            deps.storage,
            deps.api,
            env,
            info,
            trade_id,
            whitelisted_users,
        ),

        ExecuteMsg::RemoveWhitelistedUsers {
            trade_id,
            whitelisted_users,
        } => remove_whitelisted_users(deps, env, info, trade_id, whitelisted_users),

        ExecuteMsg::AddNFTsWanted {
            trade_id,
            nfts_wanted,
        } => add_nfts_wanted(deps, env, info, trade_id, nfts_wanted),

        ExecuteMsg::RemoveNFTsWanted {
            trade_id,
            nfts_wanted,
        } => remove_nfts_wanted(deps, env, info, trade_id, nfts_wanted),

        ExecuteMsg::SetComment {
            trade_id,
            counter_id,
            comment,
        } => set_comment(deps, env, info, trade_id, counter_id, comment),

        ExecuteMsg::ConfirmTrade { trade_id } => confirm_trade(deps, env, info, trade_id),

        //Counter Trade Creation Messages
        ExecuteMsg::SuggestCounterTrade { trade_id, comment } => {
            suggest_counter_trade(deps, env, info, trade_id, comment)
        }

        ExecuteMsg::AddFundsToCounterTrade {
            trade_id,
            counter_id,
        } => add_funds_to_counter_trade(deps, env, info, trade_id, counter_id),

        ExecuteMsg::RemoveFromCounterTrade {
            trade_id,
            counter_id,
            assets,
            funds,
        } => withdraw_counter_trade_assets_while_creating(
            deps, env, info, trade_id, counter_id, assets, funds,
        ),

        ExecuteMsg::ConfirmCounterTrade {
            trade_id,
            counter_id,
        } => confirm_counter_trade(deps, env, info, trade_id, counter_id),

        // After Create Messages
        ExecuteMsg::AcceptTrade {
            trade_id,
            counter_id,
        } => accept_trade(deps, env, info, trade_id, counter_id),

        // After Create Messages
        ExecuteMsg::CancelTrade { trade_id } => cancel_trade(deps, env, info, trade_id),
        ExecuteMsg::CancelCounterTrade {
            trade_id,
            counter_id,
        } => cancel_counter_trade(deps, env, info, trade_id, counter_id),

        ExecuteMsg::RefuseCounterTrade {
            trade_id,
            counter_id,
        } => refuse_counter_trade(deps, env, info, trade_id, counter_id),

        ExecuteMsg::ReviewCounterTrade {
            trade_id,
            counter_id,
            comment,
        } => review_counter_trade(deps, env, info, trade_id, counter_id, comment),

        ExecuteMsg::WithdrawPendingAssets { trader, trade_id } => {
            withdraw_accepted_funds(deps, env, info, trader, trade_id)
        }

        ExecuteMsg::WithdrawCancelledTrade { trade_id } => {
            withdraw_cancelled_trade(deps, env, info, trade_id)
        }

        ExecuteMsg::WithdrawAbortedCounter {
            trade_id,
            counter_id,
        } => withdraw_aborted_counter(deps, env, info, trade_id, counter_id),

        // Contract Variable
        ExecuteMsg::SetNewOwner { owner } => set_new_owner(deps, env, info, owner),

        // Contract Variable
        ExecuteMsg::SetNewFeeContract { fee_contract } => {
            set_new_fee_contract(deps, env, info, fee_contract)
        } /*
          // Generic (will have to remove at the end of development)
          _ => Err(ContractError::Std(StdError::generic_err(
          "Ow whaou, please wait just a bit, it's not implemented yet !",
          ))),
          */
    }
}

pub fn set_new_owner(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_owner: String,
) -> Result<Response, ContractError> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    let new_owner = deps.api.addr_validate(&new_owner)?;
    contract_info.owner = new_owner.clone();
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("changed", "owner")
        .add_attribute("new_owner", new_owner))
}

pub fn set_new_fee_contract(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    fee_contract: String,
) -> Result<Response, ContractError> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;
    let fee_contract = deps.api.addr_validate(&fee_contract)?;
    contract_info.fee_contract = Some(fee_contract.clone());
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("changed", "fee_contract")
        .add_attribute("new_fee_contract", fee_contract))
}

pub fn check_and_create_withdraw_messages(
    env: Env,
    recipient: &Addr,
    trade_info: &TradeInfo,
) -> Result<Response, ContractError> {
    if trade_info.assets_withdrawn {
        return Err(ContractError::TradeAlreadyWithdrawn {});
    }
    create_withdraw_messages(
        &env.contract.address,
        recipient,
        &trade_info.associated_assets,
        &trade_info.associated_funds,
    )
}

pub fn add_cw20(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    trade_id: Option<u64>,
    counter_id: Option<u64>,
    address: String,
    amount: Uint128,
    to_last_trade: Option<bool>,
    to_last_counter: Option<bool>,
) -> Result<Response, ContractError> {
    if to_last_trade.is_some() {
        add_cw20_to_trade(deps, env, sender, None, address, amount)
    } else if to_last_counter.is_some() {
        add_cw20_to_counter_trade(
            deps,
            env,
            sender,
            trade_id.ok_or_else(|| {
                return ContractError::Std(StdError::generic_err("Trade id missing"));
            })?,
            None,
            address,
            amount,
        )
    } else if counter_id.is_some() {
        add_cw20_to_counter_trade(
            deps,
            env,
            sender,
            trade_id.unwrap(),
            counter_id,
            address,
            amount,
        )
    } else {
        add_cw20_to_trade(deps, env, sender, trade_id, address, amount)
    }
}

pub fn add_cw721(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    trade_id: Option<u64>,
    counter_id: Option<u64>,
    address: String,
    token_id: String,
    to_last_trade: Option<bool>,
    to_last_counter: Option<bool>,
) -> Result<Response, ContractError> {
    if to_last_trade.is_some() {
        add_cw721_to_trade(deps, env, sender, None, address, token_id)
    } else if to_last_counter.is_some() {
        add_cw721_to_counter_trade(
            deps,
            env,
            sender,
            trade_id.ok_or_else(|| {
                return ContractError::Std(StdError::generic_err("Trade id missing"));
            })?,
            None,
            address,
            token_id,
        )
    } else if counter_id.is_some() {
        add_cw721_to_counter_trade(
            deps,
            env,
            sender,
            trade_id.unwrap(),
            counter_id,
            address,
            token_id,
        )
    } else {
        add_cw721_to_trade(deps, env, sender, trade_id, address, token_id)
    }
}

pub fn add_cw1155(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    trade_id: Option<u64>,
    counter_id: Option<u64>,
    address: String,
    token_id: String,
    value: Uint128,
    to_last_trade: Option<bool>,
    to_last_counter: Option<bool>,
) -> Result<Response, ContractError> {
    if to_last_trade.is_some() {
        add_cw1155_to_trade(deps, env, sender, None, address, token_id, value)
    } else if to_last_counter.is_some() {
        add_cw1155_to_counter_trade(
            deps,
            env,
            sender,
            trade_id.ok_or_else(|| {
                return ContractError::Std(StdError::generic_err("Trade id missing"));
            })?,
            None,
            address,
            token_id,
            value,
        )
    } else if counter_id.is_some() {
        add_cw1155_to_counter_trade(
            deps,
            env,
            sender,
            trade_id.unwrap(),
            counter_id,
            address,
            token_id,
            value,
        )
    } else {
        add_cw1155_to_trade(deps, env, sender, trade_id, address, token_id, value)
    }
}

pub fn withdraw_accepted_funds(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trader: String,
    trade_id: u64,
) -> Result<Response, ContractError> {
    // The fee contract is the only one responsible for withdrawing assets
    is_fee_contract(deps.storage, info.sender)?;

    // We load the trade and verify it has been accepted
    let mut trade_info = load_trade(deps.storage, trade_id)?;
    if trade_info.state != TradeState::Accepted {
        return Err(ContractError::TradeNotAccepted {});
    }

    let counter_id = trade_info
        .accepted_info
        .clone()
        .ok_or(ContractError::ContractBug {})?
        .counter_id;
    let mut counter_info = load_counter_trade(deps.storage, trade_id, counter_id)?;

    let trader = deps.api.addr_validate(&trader)?;
    let trade_type: &str;
    let res;

    // We need to indentify who the transaction sender is (trader or counter-trader)
    if trade_info.owner == trader {
        // In case the trader wants to withdraw the exchanged funds
        res = check_and_create_withdraw_messages(env, &trader, &counter_info)?;

        trade_type = "counter";
        counter_info.assets_withdrawn = true;
        COUNTER_TRADE_INFO.save(
            deps.storage,
            (trade_id.into(), counter_id.into()),
            &counter_info,
        )?;
    } else if counter_info.owner == trader {
        // In case the counter_trader wants to withdraw the exchanged funds
        res = check_and_create_withdraw_messages(env, &trader, &trade_info)?;

        trade_type = "trade";
        trade_info.assets_withdrawn = true;
        TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;
    } else {
        return Err(ContractError::NotWithdrawableByYou {});
    }

    Ok(res
        .add_attribute("withdraw funds", trade_type)
        .add_attribute("trade", trade_id.to_string())
        .add_attribute("counter", counter_id.to_string()))
}

pub fn withdraw_cancelled_trade(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trade_id: u64,
) -> Result<Response, ContractError> {
    //We load the trade and verify it has been accepted
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;
    if trade_info.state != TradeState::Cancelled {
        return Err(ContractError::TradeNotCancelled {});
    }
    let res = check_and_create_withdraw_messages(env, &info.sender, &trade_info)?;
    trade_info.assets_withdrawn = true;
    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;

    Ok(res
        .add_attribute("withdraw funds", "trade")
        .add_attribute("trade", trade_id.to_string()))
}

pub fn withdraw_aborted_counter(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: u64,
) -> Result<Response, ContractError> {
    //We load the trade and verify it has been accepted
    let trade_info = load_trade(deps.storage, trade_id)?;
    let mut counter_info = is_counter_trader(deps.storage, &info.sender, trade_id, counter_id)?;

    // If the associated trade is accepted and the counter was not selected
    // Or if the counter was refused
    // Or if the associated trade was cancelled
    // Or if this counter was cancelled
    if !((trade_info.state == TradeState::Accepted && counter_info.state != TradeState::Accepted)
        || (counter_info.state == TradeState::Refused)
        || (trade_info.state == TradeState::Cancelled)
        || (counter_info.state == TradeState::Cancelled))
    {
        return Err(ContractError::CounterTradeNotAborted {});
    }
    let res = check_and_create_withdraw_messages(env, &info.sender, &counter_info)?;
    counter_info.assets_withdrawn = true;
    COUNTER_TRADE_INFO.save(
        deps.storage,
        (trade_id.into(), counter_id.into()),
        &counter_info,
    )?;

    Ok(res
        .add_attribute("withdraw funds", "counter")
        .add_attribute("trade", trade_id.to_string())
        .add_attribute("counter", counter_id.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ContractInfo {} => to_binary(&query_contract_info(deps)?),
        QueryMsg::TradeInfo { trade_id } => to_binary(
            &load_trade(deps.storage, trade_id)
                .map_err(|e| StdError::generic_err(e.to_string()))?,
        ),
        QueryMsg::CounterTradeInfo {
            trade_id,
            counter_id,
        } => to_binary(
            &load_counter_trade(deps.storage, trade_id, counter_id)
                .map_err(|e| StdError::generic_err(e.to_string()))?,
        ),
        QueryMsg::GetAllCounterTrades {
            start_after,
            limit,
            filters,
        } => to_binary(&query_all_counter_trades(
            deps,
            start_after,
            limit,
            filters,
        )?),
        QueryMsg::GetCounterTrades { trade_id, start_after, limit, filters } => {
            to_binary(&query_counter_trades(deps, trade_id, start_after, limit, filters)?)
        }
        QueryMsg::GetAllTrades {
            start_after,
            limit,
            filters,
        } => to_binary(&query_all_trades(deps, start_after, limit, filters)?),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::state::load_trade;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, Attribute, BankMsg, Coin, Uint128};
    use cw1155::Cw1155ExecuteMsg;
    use cw20::Cw20ExecuteMsg;
    use cw721::Cw721ExecuteMsg;
    use p2p_trading_export::msg::into_cosmos_msg;
    use p2p_trading_export::state::{AssetInfo, Cw1155Coin, Cw20Coin, Cw721Coin};

    fn init_helper(deps: DepsMut) {
        let instantiate_msg = InstantiateMsg {
            name: "p2p-trading".to_string(),
            owner: None,
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();

        instantiate(deps, env, info, instantiate_msg).unwrap();
    }

    fn set_fee_contract_helper(deps: DepsMut) {
        let info = mock_info("creator", &[]);
        let env = mock_env();
        execute(
            deps,
            env,
            info,
            ExecuteMsg::SetNewFeeContract {
                fee_contract: "fee_contract".to_string(),
            },
        )
        .unwrap();
    }

    #[test]
    fn test_init_sanity() {
        let mut deps = mock_dependencies(&[]);
        let instantiate_msg = InstantiateMsg {
            name: "p2p-trading".to_string(),
            owner: Some("this_address".to_string()),
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();

        let res_init = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
        assert_eq!(0, res_init.messages.len());
    }

    fn create_trade_helper(deps: DepsMut, creator: &str) -> Response {
        let info = mock_info(creator, &[]);
        let env = mock_env();

        let res = execute(
            deps,
            env,
            info,
            ExecuteMsg::CreateTrade {
                whitelisted_users: Some(vec![]),
                comment: Some("Q".to_string()),
            },
        )
        .unwrap();
        return res;
    }

    fn create_private_trade_helper(deps: DepsMut, users: Vec<String>) -> Response {
        let info = mock_info("creator", &[]);
        let env = mock_env();

        let res = execute(
            deps,
            env,
            info,
            ExecuteMsg::CreateTrade {
                whitelisted_users: Some(users),
                comment: None,
            },
        )
        .unwrap();
        return res;
    }

    fn add_whitelisted_users(
        deps: DepsMut,
        trade_id: u64,
        users: Vec<String>,
    ) -> Result<Response, ContractError> {
        let info = mock_info("creator", &[]);
        let env = mock_env();

        let res = execute(
            deps,
            env,
            info,
            ExecuteMsg::AddWhitelistedUsers {
                trade_id: trade_id,
                whitelisted_users: users,
            },
        );
        return res;
    }

    fn remove_whitelisted_users(
        deps: DepsMut,
        trade_id: u64,
        users: Vec<String>,
    ) -> Result<Response, ContractError> {
        let info = mock_info("creator", &[]);
        let env = mock_env();

        let res = execute(
            deps,
            env,
            info,
            ExecuteMsg::RemoveWhitelistedUsers {
                trade_id,
                whitelisted_users: users,
            },
        );
        return res;
    }

    fn add_nfts_wanted_helper(
        deps: DepsMut,
        trader: &str,
        trade_id: u64,
        confirm: Vec<String>,
    ) -> Result<Response, ContractError> {
        let info = mock_info(trader, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::AddNFTsWanted {
                trade_id: Some(trade_id),
                nfts_wanted: confirm,
            },
        )
    }

    fn remove_nfts_wanted_helper(
        deps: DepsMut,
        trader: &str,
        trade_id: u64,
        confirm: Vec<String>,
    ) -> Result<Response, ContractError> {
        let info = mock_info(trader, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::RemoveNFTsWanted {
                trade_id,
                nfts_wanted: confirm,
            },
        )
    }

    fn add_funds_to_trade_helper(
        deps: DepsMut,
        trader: &str,
        trade_id: u64,
        coins_to_send: &Vec<Coin>,
    ) -> Result<Response, ContractError> {
        let info = mock_info(trader, coins_to_send);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::AddFundsToTrade {
                trade_id: Some(trade_id),
            },
        )
    }

    fn add_cw20_to_trade_helper(
        deps: DepsMut,
        token: &str,
        sender: &str,
        trade_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::AddCw20 {
                trade_id: Some(trade_id),
                counter_id: None,
                address: token.to_string(),
                amount: Uint128::from(100u64),
                to_last_trade: None,
                to_last_counter: None,
            },
        )
    }

    fn add_cw721_to_trade_helper(
        deps: DepsMut,
        token: &str,
        sender: &str,
        trade_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::AddCw721 {
                trade_id: Some(trade_id),
                counter_id: None,
                address: token.to_string(),
                token_id: "58".to_string(),
                to_last_trade: None,
                to_last_counter: None,
            },
        )
    }

    fn add_cw1155_to_trade_helper(
        deps: DepsMut,
        token: &str,
        sender: &str,
        value: u128,
        trade_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::AddCw1155 {
                trade_id: Some(trade_id),
                counter_id: None,
                address: token.to_string(),
                token_id: "58".to_string(),
                value: Uint128::from(value),
                to_last_trade: None,
                to_last_counter: None,
            },
        )
    }

    fn remove_from_trade_helper(
        deps: DepsMut,
        sender: &str,
        trade_id: u64,
        assets: Vec<(u16, AssetInfo)>,
        funds: Vec<(u16, Coin)>,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::RemoveFromTrade {
                trade_id,
                assets,
                funds,
            },
        )
    }

    fn confirm_trade_helper(
        deps: DepsMut,
        sender: &str,
        trade_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::ConfirmTrade {
                trade_id: Some(trade_id),
            },
        )
    }

    fn withdraw_helper(
        deps: DepsMut,
        trader: &str,
        sender: &str,
        trade_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::WithdrawPendingAssets {
                trader: trader.to_string(),
                trade_id,
            },
        )
    }

    fn withdraw_cancelled_trade_helper(
        deps: DepsMut,
        sender: &str,
        trade_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::WithdrawCancelledTrade { trade_id },
        )
    }

    fn withdraw_aborted_counter_helper(
        deps: DepsMut,
        sender: &str,
        trade_id: u64,
        counter_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::WithdrawAbortedCounter {
                trade_id,
                counter_id,
            },
        )
    }

    pub mod trade_tests {
        use super::*;
        use crate::query::{query_counter_trades, TradeResponse};
        use crate::trade::validate_addresses;
        use cosmwasm_std::{coin, Api, SubMsg};
        use p2p_trading_export::msg::QueryFilters;
        use p2p_trading_export::state::{CounterTradeInfo, AdditionnalTradeInfo, Comment};
        use std::collections::HashSet;
        use std::iter::FromIterator;

        #[test]
        fn create_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            let res = create_trade_helper(deps.as_mut(), "creator");

            assert_eq!(res.messages, vec![]);
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("trade", "created"),
                    Attribute::new("trade_id", "0"),
                ]
            );

            let res = create_trade_helper(deps.as_mut(), "creator");

            assert_eq!(res.messages, vec![]);
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("trade", "created"),
                    Attribute::new("trade_id", "1"),
                ]
            );

            let new_trade_info = load_trade(&deps.storage, 0).unwrap();

            assert_eq!(new_trade_info.state, TradeState::Created {});

            // Query all and check that trades exist, without filters specified
            let res = query_all_trades(deps.as_ref(), None, None, None).unwrap();

            assert_eq!(
                res.trades,
                vec![
                    {
                        TradeResponse {
                            trade_id: 1,
                            counter_id: None,
                            trade_info: TradeInfo {
                                owner: deps.api.addr_validate("creator").unwrap(),
                                additionnal_info: AdditionnalTradeInfo {
                                    owner_comment: Some(Comment {
                                        comment: "Q".to_string(),
                                        time: mock_env().block.time
                                    }),
                                    time: mock_env().block.time,
                                    ..Default::default()
                                },
                                ..Default::default()
                            },
                        }
                    },
                    {
                        TradeResponse {
                            trade_id: 0,
                            counter_id: None,
                            trade_info: TradeInfo {
                                owner: deps.api.addr_validate("creator").unwrap(),
                                additionnal_info: AdditionnalTradeInfo {
                                    owner_comment: Some(Comment {
                                        comment: "Q".to_string(),
                                        time: mock_env().block.time
                                    }),
                                    time: mock_env().block.time,
                                    ..Default::default()
                                },
                                ..Default::default()
                            },
                        }
                    }
                ]
            );
        }

        #[test]
        fn create_trade_and_nfts_wanted() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            let res = add_nfts_wanted_helper(
                deps.as_mut(),
                "creator",
                0,
                vec!["nft1".to_string(), "nft2".to_string()],
            )
            .unwrap();
            assert_eq!(
                res.attributes,
                vec![Attribute::new("added", "nfts_wanted"),]
            );

            let trade = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(
                trade.additionnal_info.nfts_wanted,
                HashSet::from_iter(vec![Addr::unchecked("nft1"), Addr::unchecked("nft2")])
            );

            add_nfts_wanted_helper(deps.as_mut(), "creator", 0, vec!["nft1".to_string()]).unwrap();
            remove_nfts_wanted_helper(deps.as_mut(), "creator", 0, vec!["nft1".to_string()])
                .unwrap();

            let trade = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(
                trade.additionnal_info.nfts_wanted,
                HashSet::from_iter(vec![Addr::unchecked("nft2")])
            );
        }

        #[test]
        fn create_multiple_trades_and_query() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            let res = create_trade_helper(deps.as_mut(), "creator");

            assert_eq!(res.messages, vec![]);
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("trade", "created"),
                    Attribute::new("trade_id", "0"),
                ]
            );

            let res = create_trade_helper(deps.as_mut(), "creator2");

            assert_eq!(res.messages, vec![]);
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("trade", "created"),
                    Attribute::new("trade_id", "1"),
                ]
            );

            let new_trade_info = load_trade(&deps.storage, 0).unwrap();

            assert_eq!(new_trade_info.state, TradeState::Created {});

            let new_trade_info = load_trade(&deps.storage, 1).unwrap();
            assert_eq!(new_trade_info.state, TradeState::Created {});

            create_trade_helper(deps.as_mut(), "creator2");
            confirm_trade_helper(deps.as_mut(), "creator2", 2).unwrap();

            // Query all created trades check that creators are different
            let res = query_all_trades(
                deps.as_ref(),
                None,
                None,
                Some(QueryFilters {
                    states: Some(vec![TradeState::Created.to_string()]),
                    ..Default::default()
                }),
            )
            .unwrap();

            assert_eq!(
                res.trades,
                vec![
                    {
                        TradeResponse {
                            trade_id: 1,
                            counter_id: None,
                            trade_info: TradeInfo {
                                owner: deps.api.addr_validate("creator2").unwrap(),
                                additionnal_info: AdditionnalTradeInfo {
                                    owner_comment: Some(Comment {
                                        comment: "Q".to_string(),
                                        time: mock_env().block.time
                                    }),
                                    time: mock_env().block.time,
                                    ..Default::default()
                                },
                                ..Default::default()
                            },
                        }
                    },
                    {
                        TradeResponse {
                            trade_id: 0,
                            counter_id: None,
                            trade_info: TradeInfo {
                                owner: deps.api.addr_validate("creator").unwrap(),
                                additionnal_info: AdditionnalTradeInfo {
                                    owner_comment: Some(Comment {
                                        comment: "Q".to_string(),
                                        time: mock_env().block.time
                                    }),
                                    time: mock_env().block.time,
                                    ..Default::default()
                                },
                                ..Default::default()
                            },
                        }
                    }
                ]
            );

            // Verify that pagination by trade_id works
            let res = query_all_trades(
                deps.as_ref(),
                Some(1),
                None,
                Some(QueryFilters {
                    states: Some(vec![TradeState::Created.to_string()]),
                    ..Default::default()
                }),
            )
            .unwrap();

            assert_eq!(
                res.trades,
                vec![{
                    TradeResponse {
                        trade_id: 0,
                        counter_id: None,
                        trade_info: TradeInfo {
                            owner: deps.api.addr_validate("creator").unwrap(),
                            additionnal_info: AdditionnalTradeInfo {
                                owner_comment: Some(Comment {
                                    comment: "Q".to_string(),
                                    time: mock_env().block.time
                                }),
                                time: mock_env().block.time,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    }
                }]
            );

            // Query that query returned only queries that are in created state and belong to creator2
            let res = query_all_trades(
                deps.as_ref(),
                None,
                None,
                Some(QueryFilters {
                    states: Some(vec![TradeState::Created.to_string()]),
                    owner: Some("creator2".to_string()),
                    ..Default::default()
                }),
            )
            .unwrap();

            assert_eq!(
                res.trades,
                vec![TradeResponse {
                    trade_id: 1,
                    counter_id: None,
                    trade_info: TradeInfo {
                        owner: deps.api.addr_validate("creator2").unwrap(),
                        additionnal_info: AdditionnalTradeInfo {
                            owner_comment: Some(Comment {
                                comment: "Q".to_string(),
                                time: mock_env().block.time
                            }),
                            time: mock_env().block.time,
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }]
            );

            // Check that if states are None that owner query still works
            let res = query_all_trades(
                deps.as_ref(),
                None,
                None,
                Some(QueryFilters {
                    owner: Some("creator2".to_string()),
                    ..Default::default()
                }),
            )
            .unwrap();

            assert_eq!(
                res.trades,
                vec![
                    TradeResponse {
                        trade_id: 2,
                        counter_id: None,
                        trade_info: TradeInfo {
                            owner: deps.api.addr_validate("creator2").unwrap(),
                            state: TradeState::Published,
                            additionnal_info: AdditionnalTradeInfo {
                                owner_comment: Some(Comment {
                                    comment: "Q".to_string(),
                                    time: mock_env().block.time
                                }),
                                time: mock_env().block.time,
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    },
                    TradeResponse {
                        trade_id: 1,
                        counter_id: None,
                        trade_info: TradeInfo {
                            owner: deps.api.addr_validate("creator2").unwrap(),
                            additionnal_info: AdditionnalTradeInfo {
                                owner_comment: Some(Comment {
                                    comment: "Q".to_string(),
                                    time: mock_env().block.time
                                }),
                                time: mock_env().block.time,
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    }
                ]
            );

            // Check that queries with published state do not return anything. Because none exists.
            let res = query_all_trades(
                deps.as_ref(),
                None,
                None,
                Some(QueryFilters {
                    states: Some(vec![TradeState::Accepted.to_string()]),
                    ..Default::default()
                }),
            )
            .unwrap();

            assert_eq!(res.trades, vec![]);

            // Check that queries with published state do not return anything when owner is specified. Because none exists.
            let res = query_all_trades(
                deps.as_ref(),
                None,
                None,
                Some(QueryFilters {
                    states: Some(vec![TradeState::Accepted.to_string()]),
                    owner: Some("creator2".to_string()),
                    ..Default::default()
                }),
            )
            .unwrap();
            assert_eq!(res.trades, vec![]);
        }

        #[test]
        fn create_trade_and_add_funds() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            let res =
                add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(2, "token")).unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added funds", "trade"),
                    Attribute::new("trade_id", "0"),
                ]
            );

            add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(2, "token")).unwrap();

            add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(2, "other_token"))
                .unwrap();

            let new_trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(
                new_trade_info.associated_funds,
                vec![
                    Coin {
                        amount: Uint128::from(4u64),
                        denom: "token".to_string()
                    },
                    Coin {
                        amount: Uint128::from(2u64),
                        denom: "other_token".to_string()
                    }
                ]
            );
        }

        #[test]
        fn create_trade_and_add_cw20_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");

            let res = add_cw20_to_trade_helper(deps.as_mut(), "token", "creator", 0).unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added token", "trade"),
                    Attribute::new("token", "token"),
                    Attribute::new("amount", "100"),
                ]
            );

            add_cw20_to_trade_helper(deps.as_mut(), "token", "creator", 0).unwrap();
            add_cw20_to_trade_helper(deps.as_mut(), "other_token", "creator", 0).unwrap();

            let new_trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(
                new_trade_info.associated_assets,
                vec![
                    AssetInfo::Cw20Coin(Cw20Coin {
                        amount: Uint128::from(200u64),
                        address: "token".to_string()
                    }),
                    AssetInfo::Cw20Coin(Cw20Coin {
                        amount: Uint128::from(100u64),
                        address: "other_token".to_string()
                    })
                ]
            );

            // Verify the token contain query
            let res = query_all_trades(
                deps.as_ref(),
                None,
                None,
                Some(QueryFilters {
                    contains_token: Some("other_token".to_string()),
                    ..Default::default()
                }),
            )
            .unwrap();

            let env = mock_env();
            assert_eq!(
                res.trades,
                vec![{
                    TradeResponse {
                        trade_id: 0,
                        counter_id: None,
                        trade_info: TradeInfo {
                            owner: deps.api.addr_validate("creator").unwrap(),
                            state: TradeState::Created,
                            associated_assets: vec![
                                AssetInfo::Cw20Coin(Cw20Coin {
                                    amount: Uint128::from(200u64),
                                    address: "token".to_string(),
                                }),
                                AssetInfo::Cw20Coin(Cw20Coin {
                                    amount: Uint128::from(100u64),
                                    address: "other_token".to_string(),
                                }),
                            ],
                            additionnal_info: AdditionnalTradeInfo {
                                owner_comment: Some(Comment {
                                    comment: "Q".to_string(),
                                    time: env.block.time,
                                }),
                                time: env.block.time,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    }
                }]
            );

            // Verify it works when querying another token
            let res = query_all_trades(
                deps.as_ref(),
                None,
                None,
                Some(QueryFilters {
                    contains_token: Some("bad_token".to_string()),
                    ..Default::default()
                }),
            )
            .unwrap();
            assert_eq!(res.trades, vec![]);

            // This triggers an error, the creator is not the same as the sender
            let err =
                add_cw20_to_trade_helper(deps.as_mut(), "token", "bad_person", 0).unwrap_err();

            assert_eq!(err, ContractError::TraderNotCreator {});
        }

        #[test]
        fn create_trade_and_add_cw721_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");

            let res = add_cw721_to_trade_helper(deps.as_mut(), "nft", "creator", 0).unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added token", "trade"),
                    Attribute::new("nft", "nft"),
                    Attribute::new("token_id", "58"),
                ]
            );

            let new_trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(
                new_trade_info.associated_assets,
                vec![AssetInfo::Cw721Coin(Cw721Coin {
                    token_id: "58".to_string(),
                    address: "nft".to_string()
                })]
            );

            // This triggers an error, the creator is not the same as the sender
            let err =
                add_cw721_to_trade_helper(deps.as_mut(), "token", "bad_person", 0).unwrap_err();

            assert_eq!(err, ContractError::TraderNotCreator {});
        }

        #[test]
        fn create_trade_and_add_cw1155_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");

            let res =
                add_cw1155_to_trade_helper(deps.as_mut(), "1155", "creator", 50u128, 0).unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added Cw1155", "trade"),
                    Attribute::new("token", "1155"),
                    Attribute::new("token_id", "58"),
                    Attribute::new("amount", "50"),
                ]
            );

            let new_trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(
                new_trade_info.associated_assets,
                vec![AssetInfo::Cw1155Coin(Cw1155Coin {
                    token_id: "58".to_string(),
                    address: "1155".to_string(),
                    value: Uint128::from(50u128)
                })]
            );

            // This triggers an error, the creator is not the same as the sender
            let err =
                add_cw721_to_trade_helper(deps.as_mut(), "token", "bad_person", 0).unwrap_err();

            assert_eq!(err, ContractError::TraderNotCreator {});
        }

        #[test]
        fn create_trade_automatic_trade_id() {
            let mut deps = mock_dependencies(&[]);
            let info = mock_info("creator", &[]);
            let env = mock_env();
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            create_trade_helper(deps.as_mut(), "creator");

            execute(
                deps.as_mut(),
                env.clone(),
                info,
                ExecuteMsg::AddCw20 {
                    trade_id: None,
                    counter_id: None,
                    address: "cw20".to_string(),
                    amount: Uint128::from(100u64),
                    to_last_trade: Some(true),
                    to_last_counter: None,
                },
            )
            .unwrap();

            let info = mock_info("creator", &coins(97u128, "uluna"));
            execute(
                deps.as_mut(),
                env,
                info,
                ExecuteMsg::AddFundsToTrade { trade_id: None },
            )
            .unwrap();

            let trade_info = TRADE_INFO.load(&deps.storage, 1u64.into()).unwrap();
            assert_eq!(
                trade_info.associated_assets,
                vec![AssetInfo::Cw20Coin(Cw20Coin {
                    address: "cw20".to_string(),
                    amount: Uint128::from(100u128)
                })]
            );
            assert_eq!(trade_info.associated_funds, coins(97u128, "uluna"));
        }

        #[test]
        fn create_trade_add_remove_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");

            add_cw721_to_trade_helper(deps.as_mut(), "nft", "creator", 0).unwrap();
            add_cw721_to_trade_helper(deps.as_mut(), "nft-2", "creator", 0).unwrap();
            add_cw20_to_trade_helper(deps.as_mut(), "token", "creator", 0).unwrap();
            add_cw1155_to_trade_helper(deps.as_mut(), "cw1155token", "creator", 100u128, 0)
                .unwrap();
            add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(100, "luna")).unwrap();

            let res = remove_from_trade_helper(
                deps.as_mut(),
                "creator",
                0,
                vec![
                    (
                        0,
                        AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft".to_string(),
                            token_id: "58".to_string(),
                        }),
                    ),
                    (
                        2,
                        AssetInfo::Cw20Coin(Cw20Coin {
                            address: "token".to_string(),
                            amount: Uint128::from(58u64),
                        }),
                    ),
                    (
                        3,
                        AssetInfo::Cw1155Coin(Cw1155Coin {
                            address: "cw1155token".to_string(),
                            token_id: "58".to_string(),
                            value: Uint128::from(58u128),
                        }),
                    ),
                ],
                vec![(0, coin(58, "luna"))],
            )
            .unwrap();

            assert_eq!(
                res.attributes,
                vec![Attribute::new("remove from", "trade"),]
            );

            assert_eq!(
                res.messages,
                vec![
                    SubMsg::new(
                        into_cosmos_msg(
                            Cw721ExecuteMsg::TransferNft {
                                recipient: "creator".to_string(),
                                token_id: "58".to_string()
                            },
                            "nft"
                        )
                        .unwrap()
                    ),
                    SubMsg::new(
                        into_cosmos_msg(
                            Cw20ExecuteMsg::Transfer {
                                recipient: "creator".to_string(),
                                amount: Uint128::from(58u64)
                            },
                            "token"
                        )
                        .unwrap()
                    ),
                    SubMsg::new(
                        into_cosmos_msg(
                            Cw1155ExecuteMsg::SendFrom {
                                from: mock_env().contract.address.to_string(),
                                to: "creator".to_string(),
                                token_id: "58".to_string(),
                                value: Uint128::from(58u128),
                                msg: None
                            },
                            "cw1155token"
                        )
                        .unwrap()
                    ),
                    SubMsg::new(BankMsg::Send {
                        to_address: "creator".to_string(),
                        amount: coins(58, "luna"),
                    })
                ]
            );

            let new_trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(
                new_trade_info.associated_assets,
                vec![
                    AssetInfo::Cw721Coin(Cw721Coin {
                        token_id: "58".to_string(),
                        address: "nft-2".to_string()
                    }),
                    AssetInfo::Cw20Coin(Cw20Coin {
                        amount: Uint128::from(42u64),
                        address: "token".to_string()
                    }),
                    AssetInfo::Cw1155Coin(Cw1155Coin {
                        value: Uint128::from(42u64),
                        address: "cw1155token".to_string(),
                        token_id: "58".to_string()
                    })
                ],
            );
            assert_eq!(new_trade_info.associated_funds, vec![coin(42, "luna")],);

            remove_from_trade_helper(
                deps.as_mut(),
                "creator",
                0,
                vec![
                    (
                        2,
                        AssetInfo::Cw1155Coin(Cw1155Coin {
                            address: "cw1155token".to_string(),
                            token_id: "58".to_string(),
                            value: Uint128::from(42u64),
                        }),
                    ),
                    (
                        1,
                        AssetInfo::Cw20Coin(Cw20Coin {
                            address: "token".to_string(),
                            amount: Uint128::from(42u64),
                        }),
                    ),
                ],
                vec![(0, coin(42, "luna"))],
            )
            .unwrap();

            let new_trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(
                new_trade_info.associated_assets,
                vec![AssetInfo::Cw721Coin(Cw721Coin {
                    token_id: "58".to_string(),
                    address: "nft-2".to_string()
                }),],
            );

            // This triggers an error, the creator is not the same as the sender
            let err = remove_from_trade_helper(
                deps.as_mut(),
                "bad_person",
                0,
                vec![(
                    0,
                    AssetInfo::Cw721Coin(Cw721Coin {
                        address: "nft-2".to_string(),
                        token_id: "58".to_string(),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(err, ContractError::TraderNotCreator {});

            // This triggers an error, no matching funds were found
            let err = remove_from_trade_helper(
                deps.as_mut(),
                "creator",
                0,
                vec![(
                    1,
                    AssetInfo::Cw721Coin(Cw721Coin {
                        address: "nft-2".to_string(),
                        token_id: "58".to_string(),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(
                err,
                ContractError::Std(StdError::generic_err(
                    "assets position does not exist in array"
                ))
            );

            // This triggers an error, no matching funds were found
            let err = remove_from_trade_helper(
                deps.as_mut(),
                "creator",
                0,
                vec![(
                    0,
                    AssetInfo::Cw721Coin(Cw721Coin {
                        address: "nft-1".to_string(),
                        token_id: "58".to_string(),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(
                err,
                ContractError::Std(StdError::generic_err("Wrong nft address at position 0"))
            );

            // This triggers an error, no matching funds were found
            let err = remove_from_trade_helper(
                deps.as_mut(),
                "creator",
                0,
                vec![(
                    0,
                    AssetInfo::Cw721Coin(Cw721Coin {
                        address: "nft-2".to_string(),
                        token_id: "42".to_string(),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(
                err,
                ContractError::Std(StdError::generic_err(
                    "Wrong nft id at position 0, wanted: 42, found: 58"
                ))
            );
        }

        #[test]
        fn create_trade_add_remove_tokens_errors() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");

            add_cw721_to_trade_helper(deps.as_mut(), "nft", "creator", 0).unwrap();
            add_cw721_to_trade_helper(deps.as_mut(), "nft-2", "creator", 0).unwrap();
            add_cw20_to_trade_helper(deps.as_mut(), "token", "creator", 0).unwrap();
            add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(100, "luna")).unwrap();

            let err = remove_from_trade_helper(
                deps.as_mut(),
                "creator",
                0,
                vec![(
                    2,
                    AssetInfo::Cw20Coin(Cw20Coin {
                        address: "token".to_string(),
                        amount: Uint128::from(101u64),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(
                err,
                ContractError::Std(StdError::generic_err(
                    "You can't withdraw that much token, wanted: 101, available: 100"
                ))
            );

            let err = remove_from_trade_helper(
                deps.as_mut(),
                "creator",
                0,
                vec![(
                    0,
                    AssetInfo::Cw20Coin(Cw20Coin {
                        address: "token".to_string(),
                        amount: Uint128::from(101u64),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(
                err,
                ContractError::Std(StdError::generic_err("Wrong token type at position 0"))
            );

            let err = remove_from_trade_helper(
                deps.as_mut(),
                "creator",
                0,
                vec![(
                    2,
                    AssetInfo::Cw20Coin(Cw20Coin {
                        address: "wrong-token".to_string(),
                        amount: Uint128::from(101u64),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(
                err,
                ContractError::Std(StdError::generic_err("Wrong token address at position 2"))
            );

            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            let err = remove_from_trade_helper(
                deps.as_mut(),
                "creator",
                0,
                vec![(
                    2,
                    AssetInfo::Cw20Coin(Cw20Coin {
                        address: "token".to_string(),
                        amount: Uint128::from(58u64),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(err, ContractError::TradeAlreadyPublished {});
        }

        #[test]
        fn confirm_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");

            //Wrong trade id
            let err = confirm_trade_helper(deps.as_mut(), "creator", 1).unwrap_err();
            assert_eq!(err, ContractError::NotFoundInTradeInfo {});

            //Wrong trader
            let err = confirm_trade_helper(deps.as_mut(), "bad_person", 0).unwrap_err();
            assert_eq!(err, ContractError::TraderNotCreator {});

            let res = confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("confirmed", "trade"),
                    Attribute::new("trade", "0"),
                ]
            );

            // Check with query that trade is confirmed, in published state
            let res = query_all_trades(
                deps.as_ref(),
                None,
                None,
                Some(QueryFilters {
                    states: Some(vec![TradeState::Published.to_string()]),
                    owner: Some("creator".to_string()),
                    ..Default::default()
                }),
            )
            .unwrap();

            assert_eq!(
                res.trades,
                vec![{
                    TradeResponse {
                        trade_id: 0,
                        counter_id: None,
                        trade_info: TradeInfo {
                            owner: deps.api.addr_validate("creator").unwrap(),
                            state: TradeState::Published,
                            additionnal_info: AdditionnalTradeInfo {
                                owner_comment: Some(Comment {
                                    comment: "Q".to_string(),
                                    time: mock_env().block.time
                                }),
                                time: mock_env().block.time,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    }
                }]
            );

            let new_trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(new_trade_info.state, TradeState::Published {});

            //Already confirmed
            let err = confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap_err();
            assert_eq!(
                err,
                ContractError::CantChangeTradeState {
                    from: TradeState::Published,
                    to: TradeState::Published
                }
            );
        }

        #[test]
        fn confirm_trade_and_try_add_assets() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");

            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            // This triggers an error, we can't send funds to confirmed trade
            let err = add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(2, "token"))
                .unwrap_err();
            assert_eq!(
                err,
                ContractError::WrongTradeState {
                    state: TradeState::Published
                }
            );

            // This triggers an error, we can't send tokens to confirmed trade
            let err = add_cw20_to_trade_helper(deps.as_mut(), "token", "creator", 0).unwrap_err();
            assert_eq!(
                err,
                ContractError::WrongTradeState {
                    state: TradeState::Published
                }
            );

            // This triggers an error, we can't send nfts to confirmed trade
            let err = add_cw721_to_trade_helper(deps.as_mut(), "token", "creator", 0).unwrap_err();
            assert_eq!(
                err,
                ContractError::WrongTradeState {
                    state: TradeState::Published
                }
            );
        }

        #[test]
        fn accept_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();

            let err = accept_trade_helper(deps.as_mut(), "creator", 0, 5).unwrap_err();
            assert_eq!(err, ContractError::NotFoundInCounterTradeInfo {});

            let err = accept_trade_helper(deps.as_mut(), "creator", 1, 0).unwrap_err();
            assert_eq!(err, ContractError::NotFoundInTradeInfo {});

            let err = accept_trade_helper(deps.as_mut(), "bad_person", 0, 0).unwrap_err();
            assert_eq!(err, ContractError::TraderNotCreator {});

            let err = accept_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap_err();
            assert_eq!(err, ContractError::CantAcceptNotPublishedCounter {});

            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();

            let res = accept_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("accepted", "trade"),
                    Attribute::new("trade", "0"),
                    Attribute::new("counter", "0"),
                ]
            );

            let trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(trade_info.state, TradeState::Accepted {});
            assert_eq!(
                trade_info.accepted_info.unwrap(),
                CounterTradeInfo {
                    trade_id: 0,
                    counter_id: 0
                }
            );

            let counter_trade_info = load_counter_trade(&deps.storage, 0, 0).unwrap();
            assert_eq!(counter_trade_info.state, TradeState::Accepted {});

            // Check with query that trade is confirmed, in ack state
            let res = query_all_trades(
                deps.as_ref(),
                None,
                None,
                Some(QueryFilters {
                    states: Some(vec![TradeState::Accepted.to_string()]),
                    owner: Some("creator".to_string()),
                    ..Default::default()
                }),
            )
            .unwrap();

            assert_eq!(
                res.trades,
                vec![{
                    TradeResponse {
                        trade_id: 0,
                        counter_id: None,
                        trade_info: TradeInfo {
                            owner: deps.api.addr_validate("creator").unwrap(),
                            state: TradeState::Accepted,
                            last_counter_id: Some(0),
                            accepted_info: Some(CounterTradeInfo {
                                trade_id: 0,
                                counter_id: 0,
                            }),
                            additionnal_info: AdditionnalTradeInfo {
                                owner_comment: Some(Comment {
                                    comment: "Q".to_string(),
                                    time: mock_env().block.time
                                }),
                                time: mock_env().block.time,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    }
                }]
            );

            // Check with query by trade id that one counter is returned
            let res = query_counter_trades(deps.as_ref(), 0, None, None, None).unwrap();

            assert_eq!(
                res.counter_trades,
                vec![{
                    TradeResponse {
                        counter_id: Some(0),
                        trade_id: 0,
                        trade_info: TradeInfo {
                            owner: deps.api.addr_validate("counterer").unwrap(),
                            state: TradeState::Accepted,
                            additionnal_info: AdditionnalTradeInfo {
                                owner_comment: Some(Comment {
                                    comment: "Q".to_string(),
                                    time: mock_env().block.time
                                }),
                                time: mock_env().block.time,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    }
                }]
            );

            // Check with queries that only one counter is returned by query and in accepted state
            let res = query_all_counter_trades(deps.as_ref(), None, None, None).unwrap();

            assert_eq!(
                res.counter_trades,
                vec![{
                    TradeResponse {
                        counter_id: Some(0),
                        trade_id: 0,
                        trade_info: TradeInfo {
                            owner: deps.api.addr_validate("counterer").unwrap(),
                            state: TradeState::Accepted,
                            additionnal_info: AdditionnalTradeInfo {
                                owner_comment: Some(Comment {
                                    comment: "Q".to_string(),
                                    time: mock_env().block.time
                                }),
                                time: mock_env().block.time,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    }
                }]
            );
        }

        #[test]
        fn accept_trade_with_multiple_counter() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();

            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();
            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 1).unwrap();

            let res = accept_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("accepted", "trade"),
                    Attribute::new("trade", "0"),
                    Attribute::new("counter", "0"),
                ]
            );

            let trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(trade_info.state, TradeState::Accepted {});

            let counter_trade_info = load_counter_trade(&deps.storage, 0, 0).unwrap();
            assert_eq!(counter_trade_info.state, TradeState::Accepted {});

            let counter_trade_info = load_counter_trade(&deps.storage, 0, 1).unwrap();
            assert_eq!(counter_trade_info.state, TradeState::Published {});

            // Check that both Accepted and Published counter queries exist
            let res = query_all_counter_trades(
                deps.as_ref(),
                None,
                None,
                Some(QueryFilters {
                    states: Some(vec![
                        TradeState::Accepted.to_string(),
                        TradeState::Published.to_string(),
                    ]),
                    ..Default::default()
                }),
            )
            .unwrap();

            assert_eq!(
                res.counter_trades,
                vec![
                    {
                        TradeResponse {
                            counter_id: Some(1),
                            trade_id: 0,
                            trade_info: TradeInfo {
                                owner: deps.api.addr_validate("counterer").unwrap(),
                                state: TradeState::Published,
                                additionnal_info: AdditionnalTradeInfo {
                                    owner_comment: Some(Comment {
                                        comment: "Q".to_string(),
                                        time: mock_env().block.time
                                    }),
                                    time: mock_env().block.time,
                                    ..Default::default()
                                },
                                ..Default::default()
                            },
                        }
                    },
                    {
                        TradeResponse {
                            counter_id: Some(0),
                            trade_id: 0,
                            trade_info: TradeInfo {
                                owner: deps.api.addr_validate("counterer").unwrap(),
                                state: TradeState::Accepted,

                                additionnal_info: AdditionnalTradeInfo {
                                    owner_comment: Some(Comment {
                                        comment: "Q".to_string(),
                                        time: mock_env().block.time
                                    }),
                                    time: mock_env().block.time,
                                    ..Default::default()
                                },
                                ..Default::default()
                            },
                        }
                    }
                ]
            );

            // Check that both Accepted and Published counter queries exist, paginate to skip last counter trade
            let res = query_all_counter_trades(
                deps.as_ref(),
                Some(CounterTradeInfo {
                    trade_id: 0,
                    counter_id: 1,
                }),
                None,
                Some(QueryFilters {
                    states: Some(vec![
                        TradeState::Accepted.to_string(),
                        TradeState::Published.to_string(),
                    ]),
                    ..Default::default()
                }),
            )
            .unwrap();

            assert_eq!(
                res.counter_trades,
                vec![{
                    TradeResponse {
                        counter_id: Some(0),
                        trade_id: 0,
                        trade_info: TradeInfo {
                            owner: deps.api.addr_validate("counterer").unwrap(),
                            state: TradeState::Accepted,
                            additionnal_info: AdditionnalTradeInfo {
                                owner_comment: Some(Comment {
                                    comment: "Q".to_string(),
                                    time: mock_env().block.time
                                }),
                                time: mock_env().block.time,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    }
                }]
            );
        }

        #[test]
        fn cancel_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();

            let err = cancel_trade_helper(deps.as_mut(), "creator", 1).unwrap_err();
            assert_eq!(err, ContractError::NotFoundInTradeInfo {});

            let err = cancel_trade_helper(deps.as_mut(), "bad_person", 0).unwrap_err();
            assert_eq!(err, ContractError::TraderNotCreator {});

            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();

            let res = cancel_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("cancelled", "trade"),
                    Attribute::new("trade", "0"),
                ]
            );

            // Query all counter trades make sure counter trade is published
            let res = query_all_counter_trades(deps.as_ref(), None, None, None).unwrap();

            assert_eq!(
                res.counter_trades,
                vec![{
                    TradeResponse {
                        counter_id: Some(0),
                        trade_id: 0,
                        trade_info: TradeInfo {
                            owner: deps.api.addr_validate("counterer").unwrap(),
                            state: TradeState::Published,
                            additionnal_info: AdditionnalTradeInfo {
                                owner_comment: Some(Comment {
                                    comment: "Q".to_string(),
                                    time: mock_env().block.time
                                }),
                                time: mock_env().block.time,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    }
                }]
            );
        }

        #[test]
        fn queries_with_multiple_trades_and_counter_trades() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 1).unwrap();

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 2).unwrap();

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 3).unwrap();

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 4).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer2", 0).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 1).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 2).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 3).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 4).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer2", 4).unwrap();

            // Query all before second one, should return the first one
            let res = query_all_counter_trades(
                deps.as_ref(),
                Some(CounterTradeInfo {
                    trade_id: 0,
                    counter_id: 1,
                }),
                None,
                Some(QueryFilters {
                    owner: Some("counterer2".to_string()),
                    ..Default::default()
                }),
            )
            .unwrap();

            assert_eq!(
                res.counter_trades,
                vec![TradeResponse {
                    trade_id: 0,
                    counter_id: Some(0),
                    trade_info: TradeInfo {
                        owner: deps.api.addr_validate("counterer2").unwrap(),
                        state: TradeState::Created,
                        additionnal_info: AdditionnalTradeInfo {
                            owner_comment: Some(Comment {
                                comment: "Q".to_string(),
                                time: mock_env().block.time
                            }),
                            time: mock_env().block.time,
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }]
            );

            // Query all before first one, should return empty array
            let res = query_all_counter_trades(
                deps.as_ref(),
                Some(CounterTradeInfo {
                    trade_id: 0,
                    counter_id: 0,
                }),
                None,
                None,
            )
            .unwrap();

            assert_eq!(res.counter_trades, vec![]);

            // Query for non existing user should return empty []
            let res = query_all_counter_trades(
                deps.as_ref(),
                None,
                None,
                Some(QueryFilters {
                    owner: Some("counterer5".to_string()),
                    ..Default::default()
                }),
            )
            .unwrap();

            assert_eq!(res.counter_trades, vec![]);

            // Query by trade_id should return counter queries for trade id 4
            let res = query_counter_trades(deps.as_ref(), 4, None, None, None).unwrap();

            assert_eq!(
                res.counter_trades,
                vec![
                    TradeResponse {
                        trade_id: 4,
                        counter_id: Some(1),
                        trade_info: TradeInfo {
                            owner: deps.api.addr_validate("counterer2").unwrap(),
                            state: TradeState::Created,
                            additionnal_info: AdditionnalTradeInfo {
                                owner_comment: Some(Comment {
                                    comment: "Q".to_string(),
                                    time: mock_env().block.time
                                }),
                                time: mock_env().block.time,
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    },
                    TradeResponse {
                        trade_id: 4,
                        counter_id: Some(0),
                        trade_info: TradeInfo {
                            owner: deps.api.addr_validate("counterer").unwrap(),
                            additionnal_info: AdditionnalTradeInfo {
                                owner_comment: Some(Comment {
                                    comment: "Q".to_string(),
                                    time: mock_env().block.time
                                }),
                                time: mock_env().block.time,
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    }
                ]
            );
        }

        #[test]
        fn withdraw_accepted_assets() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());
            set_fee_contract_helper(deps.as_mut());
            create_trade_helper(deps.as_mut(), "creator");
            add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(5, "lunas")).unwrap();
            add_cw20_to_trade_helper(deps.as_mut(), "token", "creator", 0).unwrap();
            add_cw721_to_trade_helper(deps.as_mut(), "nft", "creator", 0).unwrap();
            add_cw1155_to_trade_helper(deps.as_mut(), "cw1155", "creator", 100u128, 0).unwrap();
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "other_counterer", 0).unwrap();

            add_cw20_to_counter_trade_helper(
                deps.as_mut(),
                "other_counter-token",
                "other_counterer",
                0,
                0,
            )
            .unwrap();
            add_cw721_to_counter_trade_helper(
                deps.as_mut(),
                "other_counter-nft",
                "other_counterer",
                0,
                0,
            )
            .unwrap();

            add_funds_to_counter_trade_helper(
                deps.as_mut(),
                "other_counterer",
                0,
                0,
                &coins(9, "other_token"),
            )
            .unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            add_cw20_to_counter_trade_helper(deps.as_mut(), "counter-token", "counterer", 0, 1)
                .unwrap();
            add_cw721_to_counter_trade_helper(deps.as_mut(), "counter-nft", "counterer", 0, 1)
                .unwrap();
            add_funds_to_counter_trade_helper(deps.as_mut(), "counterer", 0, 1, &coins(2, "token"))
                .unwrap();
            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 1).unwrap();

            // Little test to start with (can't withdraw if the trade is not accepted)
            let err = withdraw_helper(deps.as_mut(), "anyone", "fee_contract", 0).unwrap_err();
            assert_eq!(err, ContractError::TradeNotAccepted {});

            accept_trade_helper(deps.as_mut(), "creator", 0, 1).unwrap();

            // Withdraw tests
            let err = withdraw_helper(deps.as_mut(), "bad_person", "fee_contract", 0).unwrap_err();
            assert_eq!(err, ContractError::NotWithdrawableByYou {});

            let err = withdraw_helper(deps.as_mut(), "creator", "bad_person", 0).unwrap_err();
            assert_eq!(err, ContractError::Unauthorized {});

            let res = withdraw_helper(deps.as_mut(), "creator", "fee_contract", 0).unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("withdraw funds", "counter"),
                    Attribute::new("trade", "0"),
                    Attribute::new("counter", "1"),
                ]
            );
            assert_eq!(
                res.messages,
                vec![
                    SubMsg::new(
                        into_cosmos_msg(
                            Cw20ExecuteMsg::Transfer {
                                recipient: "creator".to_string(),
                                amount: Uint128::from(100u64)
                            },
                            "counter-token"
                        )
                        .unwrap()
                    ),
                    SubMsg::new(
                        into_cosmos_msg(
                            Cw721ExecuteMsg::TransferNft {
                                recipient: "creator".to_string(),
                                token_id: "58".to_string()
                            },
                            "counter-nft"
                        )
                        .unwrap()
                    ),
                    SubMsg::new(BankMsg::Send {
                        to_address: "creator".to_string(),
                        amount: coins(2, "token"),
                    })
                ]
            );

            let err = withdraw_helper(deps.as_mut(), "creator", "fee_contract", 0).unwrap_err();
            assert_eq!(err, ContractError::TradeAlreadyWithdrawn {});

            let res = withdraw_helper(deps.as_mut(), "counterer", "fee_contract", 0).unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("withdraw funds", "trade"),
                    Attribute::new("trade", "0"),
                    Attribute::new("counter", "1"),
                ]
            );
            assert_eq!(
                res.messages,
                vec![
                    SubMsg::new(
                        into_cosmos_msg(
                            Cw20ExecuteMsg::Transfer {
                                recipient: "counterer".to_string(),
                                amount: Uint128::from(100u64)
                            },
                            "token"
                        )
                        .unwrap()
                    ),
                    SubMsg::new(
                        into_cosmos_msg(
                            Cw721ExecuteMsg::TransferNft {
                                recipient: "counterer".to_string(),
                                token_id: "58".to_string()
                            },
                            "nft"
                        )
                        .unwrap()
                    ),
                    SubMsg::new(
                        into_cosmos_msg(
                            Cw1155ExecuteMsg::SendFrom {
                                to: "counterer".to_string(),
                                from: mock_env().contract.address.to_string(),
                                token_id: "58".to_string(),
                                value: Uint128::from(100u128),
                                msg: None
                            },
                            "cw1155"
                        )
                        .unwrap()
                    ),
                    SubMsg::new(BankMsg::Send {
                        to_address: "counterer".to_string(),
                        amount: coins(5, "lunas"),
                    }),
                ]
            );

            let err = withdraw_helper(deps.as_mut(), "counterer", "fee_contract", 0).unwrap_err();
            assert_eq!(err, ContractError::TradeAlreadyWithdrawn {});

            let res =
                withdraw_aborted_counter_helper(deps.as_mut(), "other_counterer", 0, 0).unwrap();
            assert_eq!(
                res.messages,
                vec![
                    SubMsg::new(
                        into_cosmos_msg(
                            Cw20ExecuteMsg::Transfer {
                                recipient: "other_counterer".to_string(),
                                amount: Uint128::from(100u64)
                            },
                            "other_counter-token"
                        )
                        .unwrap()
                    ),
                    SubMsg::new(
                        into_cosmos_msg(
                            Cw721ExecuteMsg::TransferNft {
                                recipient: "other_counterer".to_string(),
                                token_id: "58".to_string()
                            },
                            "other_counter-nft"
                        )
                        .unwrap()
                    ),
                    SubMsg::new(BankMsg::Send {
                        to_address: "other_counterer".to_string(),
                        amount: coins(9, "other_token"),
                    }),
                ]
            );

            let err = withdraw_aborted_counter_helper(deps.as_mut(), "other_counterer", 0, 0)
                .unwrap_err();
            assert_eq!(err, ContractError::TradeAlreadyWithdrawn {});
        }

        #[test]
        fn withdraw_cancelled_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());
            create_trade_helper(deps.as_mut(), "creator");
            add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(5, "lunas")).unwrap();
            add_cw20_to_trade_helper(deps.as_mut(), "token", "creator", 0).unwrap();
            add_cw721_to_trade_helper(deps.as_mut(), "nft", "creator", 0).unwrap();
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();

            add_cw20_to_counter_trade_helper(
                deps.as_mut(),
                "other_counter-token",
                "counterer",
                0,
                0,
            )
            .unwrap();

            cancel_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            let res = withdraw_cancelled_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            assert_eq!(
                res.messages,
                vec![
                    SubMsg::new(
                        into_cosmos_msg(
                            Cw20ExecuteMsg::Transfer {
                                recipient: "creator".to_string(),
                                amount: Uint128::from(100u64)
                            },
                            "token"
                        )
                        .unwrap()
                    ),
                    SubMsg::new(
                        into_cosmos_msg(
                            Cw721ExecuteMsg::TransferNft {
                                recipient: "creator".to_string(),
                                token_id: "58".to_string()
                            },
                            "nft"
                        )
                        .unwrap()
                    ),
                    SubMsg::new(BankMsg::Send {
                        to_address: "creator".to_string(),
                        amount: coins(5, "lunas"),
                    }),
                ]
            );

            let err = withdraw_cancelled_trade_helper(deps.as_mut(), "creator", 0).unwrap_err();
            assert_eq!(err, ContractError::TradeAlreadyWithdrawn {});

            let res = withdraw_aborted_counter_helper(deps.as_mut(), "counterer", 0, 0).unwrap();
            assert_eq!(
                res.messages,
                vec![SubMsg::new(
                    into_cosmos_msg(
                        Cw20ExecuteMsg::Transfer {
                            recipient: "counterer".to_string(),
                            amount: Uint128::from(100u64)
                        },
                        "other_counter-token"
                    )
                    .unwrap()
                ),]
            );

            let err =
                withdraw_aborted_counter_helper(deps.as_mut(), "counterer", 0, 0).unwrap_err();
            assert_eq!(err, ContractError::TradeAlreadyWithdrawn {});
        }

        #[test]
        fn private() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());
            create_private_trade_helper(deps.as_mut(), vec!["whitelist".to_string()]);
            add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(5, "lunas")).unwrap();
            add_cw20_to_trade_helper(deps.as_mut(), "token", "creator", 0).unwrap();
            add_cw721_to_trade_helper(deps.as_mut(), "nft", "creator", 0).unwrap();
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            let err = suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap_err();
            assert_eq!(err, ContractError::AddressNotWhitelisted {});

            suggest_counter_trade_helper(deps.as_mut(), "whitelist", 0).unwrap();

            let err = remove_whitelisted_users(deps.as_mut(), 0, vec!["whitelist".to_string()])
                .unwrap_err();
            assert_eq!(
                err,
                ContractError::WrongTradeState {
                    state: TradeState::Countered
                }
            );

            let err =
                add_whitelisted_users(deps.as_mut(), 0, vec!["whitelist".to_string()]).unwrap_err();
            assert_eq!(
                err,
                ContractError::WrongTradeState {
                    state: TradeState::Countered
                }
            );

            create_private_trade_helper(deps.as_mut(), vec!["whitelist".to_string()]);

            remove_whitelisted_users(deps.as_mut(), 1, vec!["whitelist".to_string()]).unwrap();
            let info = TRADE_INFO.load(&deps.storage, 1_u64.into()).unwrap();
            let hash_set = HashSet::new();
            assert_eq!(info.whitelisted_users, hash_set);

            add_whitelisted_users(
                deps.as_mut(),
                1,
                vec!["whitelist-1".to_string(), "whitelist".to_string()],
            )
            .unwrap();
            add_whitelisted_users(
                deps.as_mut(),
                1,
                vec!["whitelist-2".to_string(), "whitelist".to_string()],
            )
            .unwrap();
            let info = TRADE_INFO.load(&deps.storage, 1_u64.into()).unwrap();

            let mut whitelisted_users = vec![];
            whitelisted_users.push("whitelist".to_string());
            whitelisted_users.push("whitelist-1".to_string());
            whitelisted_users.push("whitelist-2".to_string());
            let hash_set =
                HashSet::from_iter(validate_addresses(&deps.api, &whitelisted_users).unwrap());
            assert_eq!(info.whitelisted_users, hash_set);
        }
    }

    fn suggest_counter_trade_helper(
        deps: DepsMut,
        counterer: &str,
        trade_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(counterer, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::SuggestCounterTrade {
                trade_id: trade_id,
                comment: Some("Q".to_string()),
            },
        )
    }

    fn add_funds_to_counter_trade_helper(
        deps: DepsMut,
        counterer: &str,
        trade_id: u64,
        counter_id: u64,
        coins_to_send: &Vec<Coin>,
    ) -> Result<Response, ContractError> {
        let info = mock_info(counterer, coins_to_send);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::AddFundsToCounterTrade {
                trade_id,
                counter_id: Some(counter_id),
            },
        )
    }

    fn add_cw20_to_counter_trade_helper(
        deps: DepsMut,
        token: &str,
        sender: &str,
        trade_id: u64,
        counter_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::AddCw20 {
                trade_id: Some(trade_id),
                counter_id: Some(counter_id),
                address: token.to_string(),
                amount: Uint128::from(100u64),
                to_last_trade: None,
                to_last_counter: None,
            },
        )
    }

    fn add_cw721_to_counter_trade_helper(
        deps: DepsMut,
        token: &str,
        sender: &str,
        trade_id: u64,
        counter_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::AddCw721 {
                trade_id: Some(trade_id),
                counter_id: Some(counter_id),
                address: token.to_string(),
                token_id: "58".to_string(),
                to_last_trade: None,
                to_last_counter: None,
            },
        )
    }

    fn remove_from_counter_trade_helper(
        deps: DepsMut,
        sender: &str,
        trade_id: u64,
        counter_id: u64,
        assets: Vec<(u16, AssetInfo)>,
        funds: Vec<(u16, Coin)>,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::RemoveFromCounterTrade {
                trade_id,
                counter_id,
                assets,
                funds,
            },
        )
    }

    fn confirm_counter_trade_helper(
        deps: DepsMut,
        sender: &str,
        trade_id: u64,
        counter_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::ConfirmCounterTrade {
                trade_id: trade_id,
                counter_id: Some(counter_id),
            },
        )
    }

    fn review_counter_trade_helper(
        deps: DepsMut,
        sender: &str,
        trade_id: u64,
        counter_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::ReviewCounterTrade {
                trade_id: trade_id,
                counter_id: counter_id,
                comment: Some("Shit NFT my girl".to_string()),
            },
        )
    }

    fn accept_trade_helper(
        deps: DepsMut,
        sender: &str,
        trade_id: u64,
        counter_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::AcceptTrade {
                trade_id,
                counter_id,
            },
        )
    }

    fn cancel_trade_helper(
        deps: DepsMut,
        sender: &str,
        trade_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(sender, &[]);
        let env = mock_env();

        execute(deps, env, info, ExecuteMsg::CancelTrade { trade_id })
    }

    fn refuse_counter_trade_helper(
        deps: DepsMut,
        trader: &str,
        trade_id: u64,
        counter_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(trader, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::RefuseCounterTrade {
                trade_id: trade_id,
                counter_id: counter_id,
            },
        )
    }

    pub mod counter_trade_tests {
        use super::*;
        use crate::query::{AllTradesResponse, TradeResponse};
        use cosmwasm_std::{coin, from_binary, Api, SubMsg};
        use p2p_trading_export::msg::QueryFilters;
        use p2p_trading_export::state::{AdditionnalTradeInfo, Comment, CounterTradeInfo};

        #[test]
        fn create_counter_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");

            let err = suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap_err();

            assert_eq!(err, ContractError::NotCounterable {});

            let err = suggest_counter_trade_helper(deps.as_mut(), "counterer", 1).unwrap_err();

            assert_eq!(err, ContractError::NotFoundInTradeInfo {});

            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            let res = suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("counter", "created"),
                    Attribute::new("trade_id", "0"),
                    Attribute::new("counter_id", "0"),
                ]
            );
            // We need to make sure it is not couterable in case the counter is accepted
        }
        #[test]
        fn create_counter_trade_and_add_funds() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();

            let res = add_funds_to_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0u64,
                0u64,
                &coins(2, "token"),
            )
            .unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added funds", "counter"),
                    Attribute::new("trade_id", "0"),
                    Attribute::new("counter_id", "0"),
                ]
            );

            let counter_trade_info = load_counter_trade(&deps.storage, 0, 0).unwrap();
            assert_eq!(counter_trade_info.state, TradeState::Created);
            assert_eq!(counter_trade_info.associated_funds, coins(2, "token"));
        }

        #[test]
        fn create_counter_trade_and_add_cw20_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();

            let res = add_cw20_to_counter_trade_helper(deps.as_mut(), "token", "counterer", 0, 0)
                .unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added token", "counter"),
                    Attribute::new("token", "token"),
                    Attribute::new("amount", "100"),
                ]
            );

            let err = add_cw20_to_counter_trade_helper(deps.as_mut(), "token", "counterer", 0, 1)
                .unwrap_err();

            assert_eq!(err, ContractError::NotFoundInCounterTradeInfo {});

            // Verifying the state has been changed
            let trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(trade_info.state, TradeState::Countered);
            assert_eq!(trade_info.associated_assets, vec![]);

            let counter_trade_info = load_counter_trade(&deps.storage, 0, 0).unwrap();
            assert_eq!(counter_trade_info.state, TradeState::Created);
            assert_eq!(
                counter_trade_info.associated_assets,
                vec![AssetInfo::Cw20Coin(Cw20Coin {
                    address: "token".to_string(),
                    amount: Uint128::from(100u64)
                }),]
            );

            // This triggers an error, the creator is not the same as the sender
            let err = add_cw20_to_counter_trade_helper(deps.as_mut(), "token", "bad_person", 0, 0)
                .unwrap_err();

            assert_eq!(err, ContractError::CounterTraderNotCreator {});
        }

        #[test]
        fn create_trade_and_add_cw721_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();

            let res =
                add_cw721_to_counter_trade_helper(deps.as_mut(), "nft", "counterer", 0, 0).unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added token", "counter"),
                    Attribute::new("nft", "nft"),
                    Attribute::new("token_id", "58"),
                ]
            );

            // Verifying the state has been changed
            let trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(trade_info.state, TradeState::Countered);
            assert_eq!(trade_info.associated_assets, vec![]);

            let counter_trade_info = load_counter_trade(&deps.storage, 0, 0).unwrap();
            assert_eq!(counter_trade_info.state, TradeState::Created);
            assert_eq!(
                counter_trade_info.associated_assets,
                vec![AssetInfo::Cw721Coin(Cw721Coin {
                    address: "nft".to_string(),
                    token_id: "58".to_string()
                }),]
            );

            // This triggers an error, the counter-trade creator is not the same as the sender
            let err = add_cw721_to_counter_trade_helper(deps.as_mut(), "token", "bad_person", 0, 0)
                .unwrap_err();

            assert_eq!(err, ContractError::CounterTraderNotCreator {});
        }

        #[test]
        fn create_counter_trade_automatic_trade_id() {
            let mut deps = mock_dependencies(&[]);
            let info = mock_info("creator", &[]);
            let env = mock_env();
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 1).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "creator", 1).unwrap();
            
            suggest_counter_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            execute(
                deps.as_mut(),
                env.clone(),
                info,
                ExecuteMsg::AddCw20 {
                    trade_id: Some(0),
                    counter_id: None,
                    address: "cw20".to_string(),
                    amount: Uint128::from(100u64),
                    to_last_trade: None,
                    to_last_counter: Some(true),
                },
            )
            .unwrap();

            let info = mock_info("creator", &coins(97u128, "uluna"));
            execute(
                deps.as_mut(),
                env,
                info,
                ExecuteMsg::AddFundsToCounterTrade {
                    trade_id: 0,
                    counter_id: None,
                },
            )
            .unwrap();

            let info = mock_info("creator", &[]);
            let env = mock_env();

            execute(
                deps.as_mut(),
                env,
                info,
                ExecuteMsg::ConfirmCounterTrade {
                    trade_id: 0,
                    counter_id: None,
                },
            ).unwrap();

            let trade_info = COUNTER_TRADE_INFO
                .load(&deps.storage, (0u64.into(), 0u64.into()))
                .unwrap();
            assert_eq!(
                trade_info.associated_assets,
                vec![AssetInfo::Cw20Coin(Cw20Coin {
                    address: "cw20".to_string(),
                    amount: Uint128::from(100u128)
                })]
            );
            assert_eq!(trade_info.associated_funds, coins(97u128, "uluna"));
            assert_eq!(trade_info.state, TradeState::Published);
           
        }

        #[test]
        fn create_counter_trade_add_remove_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            add_cw721_to_counter_trade_helper(deps.as_mut(), "nft", "counterer", 0, 0).unwrap();
            add_cw721_to_counter_trade_helper(deps.as_mut(), "nft-2", "counterer", 0, 0).unwrap();
            add_cw20_to_counter_trade_helper(deps.as_mut(), "token", "counterer", 0, 0).unwrap();
            add_funds_to_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0,
                0,
                &coins(100, "luna"),
            )
            .unwrap();

            let res = remove_from_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0,
                0,
                vec![
                    (
                        0,
                        AssetInfo::Cw721Coin(Cw721Coin {
                            address: "nft".to_string(),
                            token_id: "58".to_string(),
                        }),
                    ),
                    (
                        2,
                        AssetInfo::Cw20Coin(Cw20Coin {
                            address: "token".to_string(),
                            amount: Uint128::from(58u64),
                        }),
                    ),
                ],
                vec![(0, coin(58, "luna"))],
            )
            .unwrap();

            assert_eq!(
                res.attributes,
                vec![Attribute::new("remove from", "counter"),]
            );
            assert_eq!(
                res.messages,
                vec![
                    SubMsg::new(
                        into_cosmos_msg(
                            Cw721ExecuteMsg::TransferNft {
                                recipient: "counterer".to_string(),
                                token_id: "58".to_string()
                            },
                            "nft"
                        )
                        .unwrap()
                    ),
                    SubMsg::new(
                        into_cosmos_msg(
                            Cw20ExecuteMsg::Transfer {
                                recipient: "counterer".to_string(),
                                amount: Uint128::from(58u64)
                            },
                            "token"
                        )
                        .unwrap()
                    ),
                    SubMsg::new(BankMsg::Send {
                        to_address: "counterer".to_string(),
                        amount: coins(58, "luna"),
                    })
                ]
            );

            let new_trade_info = load_counter_trade(&deps.storage, 0, 0).unwrap();
            assert_eq!(
                new_trade_info.associated_assets,
                vec![
                    AssetInfo::Cw721Coin(Cw721Coin {
                        token_id: "58".to_string(),
                        address: "nft-2".to_string()
                    }),
                    AssetInfo::Cw20Coin(Cw20Coin {
                        amount: Uint128::from(42u64),
                        address: "token".to_string()
                    })
                ],
            );
            assert_eq!(new_trade_info.associated_funds, vec![coin(42, "luna")],);

            remove_from_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0,
                0,
                vec![(
                    1,
                    AssetInfo::Cw20Coin(Cw20Coin {
                        address: "token".to_string(),
                        amount: Uint128::from(42u64),
                    }),
                )],
                vec![(0, coin(42, "luna"))],
            )
            .unwrap();

            let new_trade_info = load_counter_trade(&deps.storage, 0, 0).unwrap();
            assert_eq!(
                new_trade_info.associated_assets,
                vec![AssetInfo::Cw721Coin(Cw721Coin {
                    token_id: "58".to_string(),
                    address: "nft-2".to_string()
                }),],
            );

            // This triggers an error, the counterer is not the same as the sender
            let err = remove_from_counter_trade_helper(
                deps.as_mut(),
                "bad_person",
                0,
                0,
                vec![(
                    0,
                    AssetInfo::Cw721Coin(Cw721Coin {
                        address: "nft-2".to_string(),
                        token_id: "58".to_string(),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(err, ContractError::CounterTraderNotCreator {});

            // This triggers an error, no matching funds were found
            let err = remove_from_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0,
                0,
                vec![(
                    1,
                    AssetInfo::Cw721Coin(Cw721Coin {
                        address: "nft-2".to_string(),
                        token_id: "58".to_string(),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(
                err,
                ContractError::Std(StdError::generic_err(
                    "assets position does not exist in array"
                ))
            );

            // This triggers an error, no matching funds were found
            let err = remove_from_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0,
                0,
                vec![(
                    0,
                    AssetInfo::Cw721Coin(Cw721Coin {
                        address: "nft-1".to_string(),
                        token_id: "58".to_string(),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(
                err,
                ContractError::Std(StdError::generic_err("Wrong nft address at position 0"))
            );

            // This triggers an error, no matching funds were found
            let err = remove_from_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0,
                0,
                vec![(
                    0,
                    AssetInfo::Cw721Coin(Cw721Coin {
                        address: "nft-2".to_string(),
                        token_id: "42".to_string(),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(
                err,
                ContractError::Std(StdError::generic_err(
                    "Wrong nft id at position 0, wanted: 42, found: 58"
                ))
            );
        }

        #[test]
        fn create_trade_add_remove_tokens_errors() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            add_cw721_to_counter_trade_helper(deps.as_mut(), "nft", "counterer", 0, 0).unwrap();
            add_cw721_to_counter_trade_helper(deps.as_mut(), "nft-2", "counterer", 0, 0).unwrap();
            add_cw20_to_counter_trade_helper(deps.as_mut(), "token", "counterer", 0, 0).unwrap();
            add_funds_to_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0,
                0,
                &coins(100, "luna"),
            )
            .unwrap();

            let err = remove_from_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0,
                0,
                vec![(
                    2,
                    AssetInfo::Cw20Coin(Cw20Coin {
                        address: "token".to_string(),
                        amount: Uint128::from(101u64),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(
                err,
                ContractError::Std(StdError::generic_err(
                    "You can't withdraw that much token, wanted: 101, available: 100"
                ))
            );

            let err = remove_from_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0,
                0,
                vec![(
                    0,
                    AssetInfo::Cw20Coin(Cw20Coin {
                        address: "token".to_string(),
                        amount: Uint128::from(101u64),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(
                err,
                ContractError::Std(StdError::generic_err("Wrong token type at position 0"))
            );

            let err = remove_from_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0,
                0,
                vec![(
                    2,
                    AssetInfo::Cw20Coin(Cw20Coin {
                        address: "wrong-token".to_string(),
                        amount: Uint128::from(101u64),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(
                err,
                ContractError::Std(StdError::generic_err("Wrong token address at position 2"))
            );

            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();

            let err = remove_from_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0,
                0,
                vec![(
                    2,
                    AssetInfo::Cw20Coin(Cw20Coin {
                        address: "token".to_string(),
                        amount: Uint128::from(58u64),
                    }),
                )],
                vec![],
            )
            .unwrap_err();

            assert_eq!(err, ContractError::CounterTradeAlreadyPublished {});
        }

        #[test]
        fn confirm_counter_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();

            //Wrong trade id
            let err = confirm_counter_trade_helper(deps.as_mut(), "creator", 1, 0).unwrap_err();
            assert_eq!(err, ContractError::NotFoundInTradeInfo {});

            //Wrong counter id
            let err = confirm_counter_trade_helper(deps.as_mut(), "creator", 0, 1).unwrap_err();
            assert_eq!(err, ContractError::NotFoundInCounterTradeInfo {});

            //Wrong trader
            let err = confirm_counter_trade_helper(deps.as_mut(), "bad_person", 0, 0).unwrap_err();
            assert_eq!(err, ContractError::CounterTraderNotCreator {});

            // This time, it has to work fine
            let res = confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("confirmed", "counter"),
                    Attribute::new("trade", "0"),
                    Attribute::new("counter", "0"),
                ]
            );

            //Already confirmed
            let err = confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap_err();
            assert_eq!(
                err,
                ContractError::CantChangeCounterTradeState {
                    from: TradeState::Published,
                    to: TradeState::Published
                }
            );
        }

        #[test]
        fn review_counter_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();

            //Wrong trade id
            let err = review_counter_trade_helper(deps.as_mut(), "creator", 1, 0).unwrap_err();
            assert_eq!(err, ContractError::NotFoundInTradeInfo {});

            //Wrong counter id
            let err = review_counter_trade_helper(deps.as_mut(), "creator", 0, 1).unwrap_err();
            assert_eq!(err, ContractError::NotFoundInCounterTradeInfo {});

            //Wrong trader
            let err = review_counter_trade_helper(deps.as_mut(), "bad_person", 0, 0).unwrap_err();
            assert_eq!(err, ContractError::TraderNotCreator {});

            let err = review_counter_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap_err();
            assert_eq!(
                err,
                ContractError::CantChangeCounterTradeState {
                    from: TradeState::Created,
                    to: TradeState::Created
                }
            );

            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();

            // This time, it has to work fine
            let res = review_counter_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("review", "counter"),
                    Attribute::new("trade", "0"),
                    Attribute::new("counter", "0"),
                ]
            );

            // Because this was the only counter
            let new_trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(new_trade_info.state, TradeState::Countered {});
        }

        #[test]
        fn review_counter_trade_when_accepted() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();
            accept_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();

            let err = review_counter_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap_err();
            assert_eq!(err, ContractError::TradeAlreadyAccepted {});
        }

        #[test]
        fn review_counter_trade_when_cancelled() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();
            cancel_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            let err = review_counter_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap_err();
            assert_eq!(err, ContractError::TradeCancelled {});
        }

        #[test]
        fn review_counter_with_multiple() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();
            // We suggest and confirm one more counter
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 1).unwrap();

            // This time, it has to work fine
            let res = review_counter_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("review", "counter"),
                    Attribute::new("trade", "0"),
                    Attribute::new("counter", "0"),
                ]
            );

            let new_trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(new_trade_info.state, TradeState::Countered {});
        }

        #[test]
        fn refuse_counter_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            let res = refuse_counter_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("refuse", "counter"),
                    Attribute::new("trade", "0"),
                    Attribute::new("counter", "0"),
                ]
            );
        }

        #[test]
        fn refuse_counter_trade_with_multiple() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            // We suggest and confirm one more counter
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();
            // We suggest one more counter
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();

            let res = refuse_counter_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("refuse", "counter"),
                    Attribute::new("trade", "0"),
                    Attribute::new("counter", "0"),
                ]
            );
        }

        #[test]
        fn refuse_accepted_counter_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();

            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();
            accept_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();
            let err = refuse_counter_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap_err();
            assert_eq!(err, ContractError::TradeAlreadyAccepted {});
        }

        #[test]
        fn cancel_accepted_counter_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();

            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();
            accept_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();
            let err = cancel_trade_helper(deps.as_mut(), "creator", 0).unwrap_err();
            assert_eq!(
                err,
                ContractError::CantChangeTradeState {
                    from: TradeState::Accepted,
                    to: TradeState::Cancelled
                }
            );
        }

        #[test]
        fn confirm_counter_trade_after_accepted() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();
            accept_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();

            //Already confirmed
            let err = confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap_err();
            assert_eq!(
                err,
                ContractError::CantChangeTradeState {
                    from: TradeState::Accepted,
                    to: TradeState::Countered
                }
            );
        }

        #[test]
        fn query_trades_by_counterer() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            // When no counter_trades
            let env = mock_env();
            let res: AllTradesResponse = from_binary(
                &query(
                    deps.as_ref(),
                    env,
                    QueryMsg::GetAllTrades {
                        start_after: None,
                        limit: None,
                        filters: Some(QueryFilters {
                            counterer: Some("counterer".to_string()),
                            ..QueryFilters::default()
                        }),
                    },
                )
                .unwrap(),
            )
            .unwrap();

            assert_eq!(res.trades, vec![]);

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0).unwrap();
            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();
            accept_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 1).unwrap();
            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 2).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "bad_person", 1).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "bad_person", 1).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "bad_person", 1).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "bad_person", 1).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "bad_person", 1).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "bad_person", 1).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "bad_person", 1).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "bad_person", 1).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 2).unwrap();

            let env = mock_env();
            let res: AllTradesResponse = from_binary(
                &query(
                    deps.as_ref(),
                    env,
                    QueryMsg::GetAllTrades {
                        start_after: None,
                        limit: None,
                        filters: Some(QueryFilters {
                            counterer: Some("counterer".to_string()),
                            ..QueryFilters::default()
                        }),
                    },
                )
                .unwrap(),
            )
            .unwrap();

            let env = mock_env();
            assert_eq!(
                res.trades,
                vec![
                    {
                        TradeResponse {
                            trade_id: 2,
                            counter_id: None,
                            trade_info: TradeInfo {
                                owner: deps.api.addr_validate("creator").unwrap(),
                                last_counter_id: Some(0),
                                state: TradeState::Countered,
                                additionnal_info: AdditionnalTradeInfo {
                                    owner_comment: Some(Comment {
                                        comment: "Q".to_string(),
                                        time: env.block.time,
                                    }),
                                    time: env.block.time,
                                    ..Default::default()
                                },
                                ..Default::default()
                            },
                        }
                    },
                    {
                        TradeResponse {
                            trade_id: 0,
                            counter_id: None,
                            trade_info: TradeInfo {
                                owner: deps.api.addr_validate("creator").unwrap(),
                                last_counter_id: Some(3),
                                state: TradeState::Accepted,
                                accepted_info: Some(CounterTradeInfo {
                                    trade_id: 0,
                                    counter_id: 0,
                                }),
                                additionnal_info: AdditionnalTradeInfo {
                                    owner_comment: Some(Comment {
                                        comment: "Q".to_string(),
                                        time: env.block.time,
                                    }),
                                    time: env.block.time,
                                    ..Default::default()
                                },
                                ..Default::default()
                            },
                        }
                    }
                ]
            );
        }
    }
}
