#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
};

use crate::error::ContractError;

use crate::state::{
    is_counter_trader, is_trader, load_counter_trade, load_trade, CONTRACT_INFO,
    COUNTER_TRADE_INFO, TRADE_INFO,
};
use p2p_trading_export::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use p2p_trading_export::state::{ContractInfo, TradeInfo, TradeState};

use crate::counter_trade::{
    add_funds_to_counter_trade, add_nft_to_counter_trade, add_token_to_counter_trade,
    cancel_counter_trade, confirm_counter_trade, suggest_counter_trade,
    withdraw_counter_trade_assets_while_creating,
};
use crate::trade::{
    accept_trade, add_funds_to_trade, add_nft_to_trade, add_token_to_trade, add_whitelisted_users,
    cancel_trade, confirm_trade, create_trade, create_withdraw_messages, refuse_counter_trade,
    remove_whitelisted_users, withdraw_trade_assets_while_creating,
};

use crate::messages::review_counter_trade;
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
        owner: msg.owner.unwrap_or_else(|| info.sender.to_string()),
        last_trade_id: None,
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    Ok(Response::default().add_attribute("multisender", "init"))
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
        ExecuteMsg::CreateTrade { whitelisted_users } => {
            create_trade(deps, env, info, whitelisted_users)
        }
        ExecuteMsg::AddFundsToTrade { trade_id, confirm } => {
            add_funds_to_trade(deps, env, info, trade_id, confirm)
        }
        ExecuteMsg::AddCw20 {
            trade_id,
            counter_id,
            address,
            amount,
        } => {
            if let Some(counter) = counter_id {
                add_token_to_counter_trade(
                    deps,
                    env,
                    info.sender.into(),
                    trade_id,
                    counter,
                    address,
                    amount,
                )
            } else {
                add_token_to_trade(deps, env, info.sender.into(), trade_id, address, amount)
            }
        }

        ExecuteMsg::AddCw721 {
            trade_id,
            counter_id,
            address,
            token_id,
        } => {
            if let Some(counter) = counter_id {
                add_nft_to_counter_trade(
                    deps,
                    env,
                    info.sender.into(),
                    trade_id,
                    counter,
                    address,
                    token_id,
                )
            } else {
                add_nft_to_trade(deps, env, info.sender.into(), trade_id, address, token_id)
            }
        }
        ExecuteMsg::RemoveFromTrade {
            trade_id,
            assets,
            funds,
        } => withdraw_trade_assets_while_creating(deps, env, info, trade_id, assets, funds),

        ExecuteMsg::AddWhitelistedUsers {
            trade_id,
            whitelisted_users,
        } => add_whitelisted_users(deps, env, info, trade_id, whitelisted_users),

        ExecuteMsg::RemoveWhitelistedUsers {
            trade_id,
            whitelisted_users,
        } => remove_whitelisted_users(deps, env, info, trade_id, whitelisted_users),

        ExecuteMsg::ConfirmTrade { trade_id } => confirm_trade(deps, env, info, trade_id),

        //Counter Trade Creation Messages
        ExecuteMsg::SuggestCounterTrade { trade_id, confirm } => {
            suggest_counter_trade(deps, env, info, trade_id, confirm)
        }

        ExecuteMsg::AddFundsToCounterTrade {
            trade_id,
            counter_id,
            confirm,
        } => add_funds_to_counter_trade(deps, env, info, trade_id, counter_id, confirm),

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

        ExecuteMsg::WithdrawPendingAssets { trade_id } => {
            withdraw_accepted_funds(deps, env, info, trade_id)
        }

        ExecuteMsg::WithdrawCancelledTrade { trade_id } => {
            withdraw_cancelled_trade(deps, env, info, trade_id)
        }

        ExecuteMsg::WithdrawAbortedCounter {
            trade_id,
            counter_id,
        } => withdraw_aborted_counter(deps, env, info, trade_id, counter_id), /*
                                                                              // Generic (will have to remove at the end of development)
                                                                                _ => Err(ContractError::Std(StdError::generic_err(
                                                                                    "Ow whaou, please wait just a bit, it's not implemented yet !",
                                                                                ))),
                                                                              */
    }
}

pub fn check_and_create_withdraw_messages(
    info: MessageInfo,
    trade_info: &TradeInfo,
) -> Result<Response, ContractError> {
    if trade_info.assets_withdrawn {
        return Err(ContractError::TradeAlreadyWithdrawn {});
    }
    create_withdraw_messages(
        info,
        &trade_info.associated_assets,
        &trade_info.associated_funds,
    )
}

pub fn withdraw_accepted_funds(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
) -> Result<Response, ContractError> {
    //We load the trade and verify it has been accepted
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

    let trade_type: &str;

    let res;

    // We need to indentify who the transaction sender is (trader or counter-trader)
    if trade_info.owner == info.sender {
        // In case the trader wants to withdraw the exchanged funds
        res = check_and_create_withdraw_messages(info, &counter_info)?;

        trade_type = "counter";
        counter_info.assets_withdrawn = true;
        COUNTER_TRADE_INFO.save(
            deps.storage,
            (trade_id.into(), counter_id.into()),
            &counter_info,
        )?;
    } else if counter_info.owner == info.sender {
        // In case the counter_trader wants to withdraw the exchanged funds
        res = check_and_create_withdraw_messages(info, &trade_info)?;

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
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
) -> Result<Response, ContractError> {
    //We load the trade and verify it has been accepted
    let mut trade_info = is_trader(deps.storage, &info.sender, trade_id)?;
    if trade_info.state != TradeState::Cancelled {
        return Err(ContractError::TradeNotCancelled {});
    }
    let res = check_and_create_withdraw_messages(info, &trade_info)?;
    trade_info.assets_withdrawn = true;
    TRADE_INFO.save(deps.storage, trade_id.into(), &trade_info)?;

    Ok(res
        .add_attribute("withdraw funds", "trade")
        .add_attribute("trade", trade_id.to_string()))
}

pub fn withdraw_aborted_counter(
    deps: DepsMut,
    _env: Env,
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
    let res = check_and_create_withdraw_messages(info, &counter_info)?;
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
            states,
            start_after,
            limit,
            owner,
        } => to_binary(&query_all_counter_trades(
            deps,
            start_after,
            limit,
            states,
            owner,
        )?),
        QueryMsg::GetCounterTrades { trade_id } => {
            to_binary(&query_counter_trades(deps, trade_id)?)
        }
        QueryMsg::GetAllTrades {
            states,
            start_after,
            limit,
            owner,
        } => to_binary(&query_all_trades(deps, start_after, limit, states, owner)?),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::state::load_trade;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, Attribute, BankMsg, Coin, Uint128};
    use cw20::Cw20ExecuteMsg;
    use cw721::Cw721ExecuteMsg;
    use p2p_trading_export::msg::into_cosmos_msg;
    use p2p_trading_export::state::{AssetInfo, Cw20Coin, Cw721Coin};

    fn init_helper(deps: DepsMut) {
        let instantiate_msg = InstantiateMsg {
            name: "p2p-trading".to_string(),
            owner: None,
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();

        instantiate(deps, env, info, instantiate_msg).unwrap();
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
                trade_id,
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

    fn add_funds_to_trade_helper(
        deps: DepsMut,
        trader: &str,
        trade_id: u64,
        coins_to_send: &Vec<Coin>,
        confirm: Option<bool>,
    ) -> Result<Response, ContractError> {
        let info = mock_info(trader, coins_to_send);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::AddFundsToTrade {
                trade_id: trade_id,
                confirm: confirm,
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
                trade_id,
                counter_id: None,
                address: token.to_string(),
                amount: Uint128::from(100u64),
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
                trade_id,
                counter_id: None,
                address: token.to_string(),
                token_id: "58".to_string(),
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
            ExecuteMsg::ConfirmTrade { trade_id: trade_id },
        )
    }

    fn withdraw_helper(
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
            ExecuteMsg::WithdrawPendingAssets { trade_id },
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
        use cosmwasm_std::{coin, SubMsg};
        use p2p_trading_export::state::CounterTradeInfo;
        use std::collections::HashSet;

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
            let res = query_all_trades(deps.as_ref(), None, None, None, None).unwrap();

            assert_eq!(
                res.trades,
                vec![
                    {
                        TradeResponse {
                            trade_id: 1,
                            counter_id: None,
                            owner: "creator".to_string(),
                            associated_assets: vec![],
                            associated_funds: vec![],
                            state: TradeState::Created.to_string(),
                            last_counter_id: None,
                            comment: None,
                            accepted_info: None,
                        }
                    },
                    {
                        TradeResponse {
                            trade_id: 0,
                            counter_id: None,
                            owner: "creator".to_string(),
                            associated_assets: vec![],
                            associated_funds: vec![],
                            state: TradeState::Created.to_string(),
                            last_counter_id: None,
                            comment: None,
                            accepted_info: None,
                        }
                    }
                ]
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
            confirm_trade_helper(deps.as_mut(), "creator2",2).unwrap();

            // Query all created trades check that creators are different
            let res = query_all_trades(
                deps.as_ref(),
                None,
                None,
                Some(vec![TradeState::Created.to_string()]),
                None,
            )
            .unwrap();

            assert_eq!(
                res.trades,
                vec![
                    {
                        TradeResponse {
                            trade_id: 1,
                            counter_id: None,
                            owner: "creator2".to_string(),
                            associated_assets: vec![],
                            associated_funds: vec![],
                            state: TradeState::Created.to_string(),
                            last_counter_id: None,
                            comment: None,
                            accepted_info: None,
                        }
                    },
                    {
                        TradeResponse {
                            trade_id: 0,
                            counter_id: None,
                            owner: "creator".to_string(),
                            associated_assets: vec![],
                            associated_funds: vec![],
                            state: TradeState::Created.to_string(),
                            last_counter_id: None,
                            comment: None,
                            accepted_info: None,
                        }
                    }
                ]
            );

            // Verify that pagination by trade_id works
            let res = query_all_trades(
                deps.as_ref(),
                Some(1),
                None,
                Some(vec![TradeState::Created.to_string()]),
                None,
            )
            .unwrap();

            assert_eq!(
                res.trades,
                vec![{
                    TradeResponse {
                        trade_id: 0,
                        counter_id: None,
                        owner: "creator".to_string(),
                        associated_assets: vec![],
                        associated_funds: vec![],
                        state: TradeState::Created.to_string(),
                        last_counter_id: None,
                        comment: None,
                        accepted_info: None,
                    }
                }]
            );

            // Query that query returned only queries that are in created state and belong to creator2
            let res = query_all_trades(
                deps.as_ref(),
                None,
                None,
                Some(vec![TradeState::Created.to_string()]),
                Some("creator2".to_string()),
            )
            .unwrap();

            assert_eq!(
                res.trades,
                vec![
                    TradeResponse {
                        trade_id: 1,
                        counter_id: None,
                        owner: "creator2".to_string(),
                        associated_assets: vec![],
                        associated_funds: vec![],
                        state: TradeState::Created.to_string(),
                        last_counter_id: None,
                        comment: None,
                        accepted_info: None,
                    }
                ]
            );

            // Check that if states are None that owner query still works
            let res = query_all_trades(
                deps.as_ref(),
                None,
                None,
                None,
                Some("creator2".to_string()),
            )
            .unwrap();

            assert_eq!(
                res.trades,
                vec![
                    TradeResponse {
                        trade_id: 2,
                        counter_id: None,
                        owner: "creator2".to_string(),
                        associated_assets: vec![],
                        associated_funds: vec![],
                        state: TradeState::Published.to_string(),
                        last_counter_id: None,
                        comment: None,
                        accepted_info: None,
                    },
                    TradeResponse {
                        trade_id: 1,
                        counter_id: None,
                        owner: "creator2".to_string(),
                        associated_assets: vec![],
                        associated_funds: vec![],
                        state: TradeState::Created.to_string(),
                        last_counter_id: None,
                        comment: None,
                        accepted_info: None,
                    }
                ]
            );

            // Check that queries with published state do not return anything. Because none exists.
            let res = query_all_trades(
                deps.as_ref(),
                None,
                None,
                Some(vec![TradeState::Accepted.to_string()]),
                None,
            )
            .unwrap();

            assert_eq!(res.trades, vec![]);

            // Check that queries with published state do not return anything when owner is specified. Because none exists.
            let res = query_all_trades(
                deps.as_ref(),
                None,
                None,
                Some(vec![TradeState::Accepted.to_string()]),
                Some("creator2".to_string()),
            )
            .unwrap();
            assert_eq!(res.trades, vec![]);
        }

        #[test]
        fn create_trade_and_add_funds() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            let res = add_funds_to_trade_helper(
                deps.as_mut(),
                "creator",
                0,
                &coins(2, "token"),
                Some(false),
            )
            .unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added funds", "trade"),
                    Attribute::new("trade_id", "0"),
                ]
            );

            add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(2, "token"), Some(false))
                .unwrap();

            add_funds_to_trade_helper(
                deps.as_mut(),
                "creator",
                0,
                &coins(2, "other_token"),
                Some(false),
            )
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
        fn create_trade_and_add_funds_and_confirm() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            let res = add_funds_to_trade_helper(
                deps.as_mut(),
                "creator",
                0,
                &coins(2, "token"),
                Some(true),
            )
            .unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added funds", "trade"),
                    Attribute::new("trade_id", "0"),
                ]
            );
            let new_trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(new_trade_info.state, TradeState::Published {});
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
                vec![AssetInfo::Cw20Coin(Cw20Coin {
                    amount: Uint128::from(200u64),
                    address: "token".to_string()
                }),AssetInfo::Cw20Coin(Cw20Coin {
                    amount: Uint128::from(100u64),
                    address: "other_token".to_string()
                })]
            );

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
        fn create_trade_add_remove_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");

            add_cw721_to_trade_helper(deps.as_mut(), "nft", "creator", 0).unwrap();
            add_cw721_to_trade_helper(deps.as_mut(), "nft-2", "creator", 0).unwrap();
            add_cw20_to_trade_helper(deps.as_mut(), "token", "creator", 0).unwrap();
            add_funds_to_trade_helper(
                deps.as_mut(),
                "creator",
                0,
                &coins(100, "luna"),
                Some(false),
            )
            .unwrap();

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
                    })
                ],
            );
            assert_eq!(new_trade_info.associated_funds, vec![coin(42, "luna")],);

            remove_from_trade_helper(
                deps.as_mut(),
                "creator",
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
            add_funds_to_trade_helper(
                deps.as_mut(),
                "creator",
                0,
                &coins(100, "luna"),
                Some(false),
            )
            .unwrap();

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
                Some(vec![TradeState::Published.to_string()]),
                Some("creator".to_string()),
            )
            .unwrap();

            assert_eq!(
                res.trades,
                vec![{
                    TradeResponse {
                        trade_id: 0,
                        counter_id: None,
                        owner: "creator".to_string(),
                        associated_assets: vec![],
                        associated_funds: vec![],
                        state: TradeState::Published.to_string(),
                        last_counter_id: None,
                        comment: None,
                        accepted_info: None,
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
            let err =
                add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(2, "token"), None)
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

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(false)).unwrap();

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
                Some(vec![TradeState::Accepted.to_string()]),
                Some("creator".to_string()),
            )
            .unwrap();

            assert_eq!(
                res.trades,
                vec![{
                    TradeResponse {
                        trade_id: 0,
                        counter_id: None,
                        owner: "creator".to_string(),
                        associated_assets: vec![],
                        associated_funds: vec![],
                        state: TradeState::Accepted.to_string(),
                        last_counter_id: Some(0),
                        comment: None,
                        accepted_info: Some(CounterTradeInfo {
                            trade_id: 0,
                            counter_id: 0,
                        }),
                    }
                }]
            );

            // Check with query by trade id that one counter is returned
            let res = query_counter_trades(deps.as_ref(), 0).unwrap();

            assert_eq!(
                res.counter_trades,
                vec![{
                    TradeResponse {
                        counter_id: Some(0),
                        trade_id: 0,
                        owner: "counterer".to_string(),
                        associated_assets: vec![],
                        associated_funds: vec![],
                        state: TradeState::Accepted.to_string(),
                        last_counter_id: None,
                        comment: None,
                        accepted_info: None,
                    }
                }]
            );

            // Check with queries that only one counter is returned by query and in accepted state
            let res = query_all_counter_trades(deps.as_ref(), None, None, None, None).unwrap();

            assert_eq!(
                res.counter_trades,
                vec![{
                    TradeResponse {
                        counter_id: Some(0),
                        trade_id: 0,
                        owner: "counterer".to_string(),
                        associated_assets: vec![],
                        associated_funds: vec![],
                        state: TradeState::Accepted.to_string(),
                        last_counter_id: None,
                        comment: None,
                        accepted_info: None,
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

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(false)).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(false)).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(false)).unwrap();

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
                Some(vec![
                    TradeState::Accepted.to_string(),
                    TradeState::Published.to_string(),
                ]),
                None,
            )
            .unwrap();

            assert_eq!(
                res.counter_trades,
                vec![
                    {
                        TradeResponse {
                            counter_id: Some(1),
                            trade_id: 0,
                            owner: "counterer".to_string(),
                            associated_assets: vec![],
                            associated_funds: vec![],
                            state: TradeState::Published.to_string(),
                            last_counter_id: None,
                            comment: None,
                            accepted_info: None,
                        }
                    },
                    {
                        TradeResponse {
                            counter_id: Some(0),
                            trade_id: 0,
                            owner: "counterer".to_string(),
                            associated_assets: vec![],
                            associated_funds: vec![],
                            state: TradeState::Accepted.to_string(),
                            last_counter_id: None,
                            comment: None,
                            accepted_info: None,
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
                Some(vec![
                    TradeState::Accepted.to_string(),
                    TradeState::Published.to_string(),
                ]),
                None,
            )
            .unwrap();

            assert_eq!(
                res.counter_trades,
                vec![{
                    TradeResponse {
                        counter_id: Some(0),
                        trade_id: 0,
                        owner: "counterer".to_string(),
                        associated_assets: vec![],
                        associated_funds: vec![],
                        state: TradeState::Accepted.to_string(),
                        last_counter_id: None,
                        comment: None,
                        accepted_info: None,
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

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(false)).unwrap();

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
            let res = query_all_counter_trades(deps.as_ref(), None, None, None, None).unwrap();

            assert_eq!(
                res.counter_trades,
                vec![{
                    TradeResponse {
                        counter_id: Some(0),
                        trade_id: 0,
                        owner: "counterer".to_string(),
                        associated_assets: vec![],
                        associated_funds: vec![],
                        state: TradeState::Published.to_string(),
                        last_counter_id: None,
                        comment: None,
                        accepted_info: None,
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

            suggest_counter_trade_helper(deps.as_mut(), "counterer2", 0, Some(false)).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 1, Some(false)).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 2, Some(false)).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 3, Some(false)).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 4, Some(false)).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer2", 4, Some(false)).unwrap();

            // Query all before second one, should return the first one
            let res = query_all_counter_trades(
                deps.as_ref(),
                Some(CounterTradeInfo {
                    trade_id: 0,
                    counter_id: 1,
                }),
                None,
                None,
                Some("counterer2".to_string()),
            )
            .unwrap();

            assert_eq!(
                res.counter_trades,
                vec![TradeResponse {
                    trade_id: 0,
                    counter_id: Some(0),
                    owner: "counterer2".to_string(),
                    associated_assets: vec![],
                    associated_funds: vec![],
                    state: TradeState::Created.to_string(),
                    last_counter_id: None,
                    comment: None,
                    accepted_info: None
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
                None
            )
            .unwrap();

            assert_eq!(res.counter_trades, vec![]);

            // Query for non existing user should return empty []
            let res = query_all_counter_trades(
                deps.as_ref(),
                None,
                None,
                None,
                Some("counterer5".to_string()),
            )
            .unwrap();

            assert_eq!(res.counter_trades, vec![]);

            // Query by trade_id should return counter queries for trade id 4
            let res = query_counter_trades(deps.as_ref(), 4).unwrap();

            assert_eq!(
                res.counter_trades,
                vec![
                    TradeResponse {
                        trade_id: 4,
                        counter_id: Some(1),
                        owner: "counterer2".to_string(),
                        associated_assets: vec![],
                        associated_funds: vec![],
                        state: TradeState::Created.to_string(),
                        last_counter_id: None,
                        comment: None,
                        accepted_info: None
                    },
                    TradeResponse {
                        trade_id: 4,
                        counter_id: Some(0),
                        owner: "counterer".to_string(),
                        associated_assets: vec![],
                        associated_funds: vec![],
                        state: TradeState::Created.to_string(),
                        last_counter_id: None,
                        comment: None,
                        accepted_info: None
                    }
                ]
            );
        }

        #[test]
        fn withdraw_accepted_assets() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());
            create_trade_helper(deps.as_mut(), "creator");
            add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(5, "lunas"), None)
                .unwrap();
            add_cw20_to_trade_helper(deps.as_mut(), "token", "creator", 0).unwrap();
            add_cw721_to_trade_helper(deps.as_mut(), "nft", "creator", 0).unwrap();
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "other_counterer", 0, Some(false)).unwrap();

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
                Some(false),
            )
            .unwrap();

            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(false)).unwrap();
            add_cw20_to_counter_trade_helper(deps.as_mut(), "counter-token", "counterer", 0, 1)
                .unwrap();
            add_cw721_to_counter_trade_helper(deps.as_mut(), "counter-nft", "counterer", 0, 1)
                .unwrap();
            add_funds_to_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0,
                1,
                &coins(2, "token"),
                Some(true),
            )
            .unwrap();

            // Little test to start with (can't withdraw if the trade is not accepted)
            let err = withdraw_helper(deps.as_mut(), "anyone", 0).unwrap_err();
            assert_eq!(err, ContractError::TradeNotAccepted {});

            accept_trade_helper(deps.as_mut(), "creator", 0, 1).unwrap();

            // Withdraw tests
            let err = withdraw_helper(deps.as_mut(), "bad_person", 0).unwrap_err();
            assert_eq!(err, ContractError::NotWithdrawableByYou {});

            let res = withdraw_helper(deps.as_mut(), "creator", 0).unwrap();
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

            let err = withdraw_helper(deps.as_mut(), "creator", 0).unwrap_err();
            assert_eq!(err, ContractError::TradeAlreadyWithdrawn {});

            let res = withdraw_helper(deps.as_mut(), "counterer", 0).unwrap();
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
                    SubMsg::new(BankMsg::Send {
                        to_address: "counterer".to_string(),
                        amount: coins(5, "lunas"),
                    }),
                ]
            );

            let err = withdraw_helper(deps.as_mut(), "counterer", 0).unwrap_err();
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
            add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(5, "lunas"), None)
                .unwrap();
            add_cw20_to_trade_helper(deps.as_mut(), "token", "creator", 0).unwrap();
            add_cw721_to_trade_helper(deps.as_mut(), "nft", "creator", 0).unwrap();
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(false)).unwrap();

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
            add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(5, "lunas"), None)
                .unwrap();
            add_cw20_to_trade_helper(deps.as_mut(), "token", "creator", 0).unwrap();
            add_cw721_to_trade_helper(deps.as_mut(), "nft", "creator", 0).unwrap();
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            let err = suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(false))
                .unwrap_err();
            assert_eq!(err, ContractError::AddressNotWhitelisted {});

            suggest_counter_trade_helper(deps.as_mut(), "whitelist", 0, Some(false)).unwrap();

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
            let mut hash_set = HashSet::new();
            hash_set.insert("whitelist".to_string());
            hash_set.insert("whitelist-1".to_string());
            hash_set.insert("whitelist-2".to_string());
            assert_eq!(info.whitelisted_users, hash_set);
        }
    }

    fn suggest_counter_trade_helper(
        deps: DepsMut,
        counterer: &str,
        trade_id: u64,
        confirm: Option<bool>,
    ) -> Result<Response, ContractError> {
        let info = mock_info(counterer, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::SuggestCounterTrade {
                trade_id: trade_id,
                confirm: confirm,
            },
        )
    }

    fn add_funds_to_counter_trade_helper(
        deps: DepsMut,
        counterer: &str,
        trade_id: u64,
        counter_id: u64,
        coins_to_send: &Vec<Coin>,
        confirm: Option<bool>,
    ) -> Result<Response, ContractError> {
        let info = mock_info(counterer, coins_to_send);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::AddFundsToCounterTrade {
                trade_id,
                counter_id,
                confirm,
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
                trade_id,
                counter_id: Some(counter_id),
                address: token.to_string(),
                amount: Uint128::from(100u64),
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
                trade_id,
                counter_id: Some(counter_id),
                address: token.to_string(),
                token_id: "58".to_string(),
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
                counter_id: counter_id,
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
        use cosmwasm_std::{coin, SubMsg};

        #[test]
        fn create_counter_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");

            let err = suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(false))
                .unwrap_err();

            assert_eq!(err, ContractError::NotCounterable {});

            let err = suggest_counter_trade_helper(deps.as_mut(), "counterer", 1, Some(false))
                .unwrap_err();

            assert_eq!(err, ContractError::NotFoundInTradeInfo {});

            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            let res =
                suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(false)).unwrap();

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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();

            let res = add_funds_to_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0u64,
                0u64,
                &coins(2, "token"),
                Some(false),
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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();

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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();

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
        fn create_counter_trade_add_remove_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut(), "creator");
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();
            add_cw721_to_counter_trade_helper(deps.as_mut(), "nft", "counterer", 0, 0).unwrap();
            add_cw721_to_counter_trade_helper(deps.as_mut(), "nft-2", "counterer", 0, 0).unwrap();
            add_cw20_to_counter_trade_helper(deps.as_mut(), "token", "counterer", 0, 0).unwrap();
            add_funds_to_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0,
                0,
                &coins(100, "luna"),
                Some(false),
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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();
            add_cw721_to_counter_trade_helper(deps.as_mut(), "nft", "counterer", 0, 0).unwrap();
            add_cw721_to_counter_trade_helper(deps.as_mut(), "nft-2", "counterer", 0, 0).unwrap();
            add_cw20_to_counter_trade_helper(deps.as_mut(), "token", "counterer", 0, 0).unwrap();
            add_funds_to_counter_trade_helper(
                deps.as_mut(),
                "counterer",
                0,
                0,
                &coins(100, "luna"),
                Some(false),
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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();

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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();

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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();
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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();
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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(true)).unwrap();
            // We suggest and confirm one more counter
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(true)).unwrap();

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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();
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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();
            // We suggest and confirm one more counter
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(true)).unwrap();
            // We suggest one more counter
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(false)).unwrap();

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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();

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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();

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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();
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
    }
}
