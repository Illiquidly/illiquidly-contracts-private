#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128, BankMsg
};

use cw20_base::allowances::query_allowance;
use cw20_base::contract::{
    query_balance, query_download_logo, query_marketing_info, query_minter, query_token_info,
};

use crate::error::ContractError;
use cw20_base::enumerable::{query_all_accounts, query_all_allowances};

use cw721::Cw721ExecuteMsg;
use cw20::Cw20ExecuteMsg;

use crate::state::{CONTRACT_INFO, TRADE_INFO, COUNTER_TRADE_INFO};
use p2p_trading_export::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ReceiveMsg, into_cosmos_msg};
use p2p_trading_export::state::{ContractInfo, TradeState, AssetInfo, TradeInfo};

use crate::counter_trade::{
    add_funds_to_counter_trade, add_nft_to_counter_trade, add_token_to_counter_trade,
    confirm_counter_trade, suggest_counter_trade,
};
use crate::trade::{
    accept_trade, add_funds_to_trade, add_nft_to_trade, add_token_to_trade, confirm_trade,
    create_trade, refuse_counter_trade,
};

use crate::messages::review_counter_trade;

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
        //Register Receive messages
        ExecuteMsg::Receive {
            sender,
            amount,
            msg,
        } => receive(deps, env, info, sender, amount, msg),
        ExecuteMsg::ReceiveNft {
            sender,
            token_id,
            msg,
        } => receive_nft(deps, env, info, sender, token_id, msg),

        // Trade Creation Messages
        ExecuteMsg::CreateTrade {} => create_trade(deps, env, info),
        ExecuteMsg::AddFundsToTrade { trade_id, confirm } => {
            add_funds_to_trade(deps, env, info, trade_id, confirm)
        }
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

        ExecuteMsg::ConfirmCounterTrade {
            trade_id,
            counter_id,
        } => confirm_counter_trade(deps, env, info, trade_id, counter_id),

        // After Create Messages
        ExecuteMsg::AcceptTrade {
            trade_id,
            counter_id,
        } => accept_trade(deps, env, info, trade_id, counter_id),

        ExecuteMsg::RefuseCounterTrade {
            trade_id,
            counter_id,
        } => refuse_counter_trade(deps, env, info, trade_id, counter_id),

        ExecuteMsg::ReviewCounterTrade {
            trade_id,
            counter_id,
            comment,
        } => review_counter_trade(deps, env, info, trade_id, counter_id, comment),

        ExecuteMsg::WithdrawPendingFunds { trade_id } => {
            withdraw_accepted_funds(deps, env, info, trade_id)
        }
        // Generic (will have to remove at the end of development)
        /*_ => Err(ContractError::Std(StdError::generic_err(
            "Ow whaou, please wait just a bit, it's not implemented yet !",
        ))),
        */
    }
}

pub fn receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    from: String,
    sent_amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    let msg: ReceiveMsg = from_binary(&msg)?;
    match msg {
        ReceiveMsg::AddToTrade { trade_id } => {
            add_token_to_trade(deps, env, info, from, trade_id, sent_amount)
        }

        ReceiveMsg::AddToCounterTrade {
            trade_id,
            counter_id,
        } => add_token_to_counter_trade(deps, env, info, from, trade_id, counter_id, sent_amount),
    }
}

pub fn receive_nft(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    from: String,
    token_id: String,
    msg: Binary,
) -> Result<Response, ContractError> {
    let msg: ReceiveMsg = from_binary(&msg)?;
    match msg {
        ReceiveMsg::AddToTrade { trade_id } => {
            add_nft_to_trade(deps, env, info, from, trade_id, token_id)
        }

        ReceiveMsg::AddToCounterTrade {
            trade_id,
            counter_id,
        } => add_nft_to_counter_trade(deps, env, info, from, trade_id, counter_id, token_id),
    }
}

pub fn create_withdraw_response(
    info: MessageInfo,
    trade_info: TradeInfo,
    counter_info: TradeInfo,
) -> Result<Response, ContractError>{

    let mut res = Response::new();
    //We add tokens and nfts
    for fund in counter_info.associated_assets{
        match fund {
            AssetInfo::Cw20Coin(token) => {
                let message = Cw20ExecuteMsg::Transfer{
                    recipient: info.sender.to_string(),
                    amount: token.amount,
                };
                res = res.add_message(into_cosmos_msg(message, token.address.clone())?);
            },
            AssetInfo::Cw721Coin(nft) =>{
                let message = Cw721ExecuteMsg::TransferNft{
                    recipient: info.sender.to_string(),
                    token_id: nft.token_id,
                };
                res = res.add_message(into_cosmos_msg(message, nft.address.clone())?);
            },
        }
    }
    // We add funds (coins)
    if !trade_info.associated_funds.is_empty(){
        res = res.add_message(BankMsg::Send {
            to_address: String::from("recipient"),
            amount: trade_info.associated_funds,
        });
    };
    Ok(res)
}

pub fn withdraw_accepted_funds(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trade_id: u64,
) -> Result<Response, ContractError> {

    //We load the trade and verify it has been accepted
    let mut trade_info = TRADE_INFO.load(deps.storage, &trade_id.to_be_bytes())?;
    if trade_info.state != TradeState::Accepted {
        return Err(ContractError::TradeNotAccepted {});
    }

    let counter_id = trade_info.accepted_info.clone().ok_or(ContractError::ContractBug{})?.counter_id;
    let mut counter_info = COUNTER_TRADE_INFO.load(deps.storage, (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()))?;
    let trade_type: &str;

    let res;

    // We need to indentify who the transaction sender is (trader or counter-trader)
    if trade_info.owner == info.sender{
        // In case the trader wants to withdraw the exchanged funds
        res = create_withdraw_response(info,counter_info.clone(),trade_info.clone())?;

        trade_type = "trade";
        trade_info.state = TradeState::Withdrawn;
        TRADE_INFO.save(deps.storage, &trade_id.to_be_bytes(), &trade_info)?;
    }else if counter_info.owner == info.sender{
        // In case the counter_trader wants to withdraw the exchanged funds
        res = create_withdraw_response(info,trade_info.clone(),counter_info.clone())?;

        trade_type = "counter";
        counter_info.state = TradeState::Withdrawn;
        COUNTER_TRADE_INFO.save(deps.storage, (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()), &counter_info)?;
    }else{
        return Err(ContractError::ContractBug{});
    }

    Ok(res
        .add_attribute("withdraw funds",trade_type)
        .add_attribute("trade",trade_id.to_string())
        .add_attribute("counter",counter_id.to_string())
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::TokenInfo {} => to_binary(&query_token_info(deps)?),
        QueryMsg::Minter {} => to_binary(&query_minter(deps)?),
        QueryMsg::Allowance { owner, spender } => {
            to_binary(&query_allowance(deps, owner, spender)?)
        }
        QueryMsg::AllAllowances {
            owner,
            start_after,
            limit,
        } => to_binary(&query_all_allowances(deps, owner, start_after, limit)?),
        QueryMsg::AllAccounts { start_after, limit } => {
            to_binary(&query_all_accounts(deps, start_after, limit)?)
        }
        QueryMsg::MarketingInfo {} => to_binary(&query_marketing_info(deps)?),
        QueryMsg::DownloadLogo {} => to_binary(&query_download_logo(deps)?),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, Attribute};

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

    fn create_trade_helper(deps: DepsMut) -> Response {
        let info = mock_info("creator", &[]);
        let env = mock_env();

        let res = execute(deps, env, info, ExecuteMsg::CreateTrade {}).unwrap();
        return res;
    }

    fn add_funds_to_trade_helper(deps: DepsMut, trade_id: u64, confirm: Option<bool>) -> Response {
        let info = mock_info("creator", &coins(2, "token"));
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
        .unwrap()
    }

    fn add_cw20_to_trade_helper(
        deps: DepsMut,
        token: &str,
        sender: String,
    ) -> Result<Response, ContractError> {
        let info = mock_info(token, &[]);
        let env = mock_env();

        let msg = to_binary(&ReceiveMsg::AddToTrade { trade_id: 0 }).unwrap();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::Receive {
                sender: sender,
                amount: Uint128::from(100u64),
                msg: msg,
            },
        )
    }

    fn add_cw721_to_trade_helper(
        deps: DepsMut,
        token: &str,
        sender: String,
    ) -> Result<Response, ContractError> {
        let info = mock_info(token, &[]);
        let env = mock_env();

        let msg = to_binary(&ReceiveMsg::AddToTrade { trade_id: 0 }).unwrap();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::ReceiveNft {
                sender: sender,
                token_id: "58".to_string(),
                msg: msg,
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

    pub mod trade_tests {
        use super::*;

        #[test]
        fn create_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            let res = create_trade_helper(deps.as_mut());

            assert_eq!(res.messages, vec![]);
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("trade", "created"),
                    Attribute::new("trade_id", "0"),
                ]
            );

            // TODO, verify the query

            let res = create_trade_helper(deps.as_mut());

            assert_eq!(res.messages, vec![]);
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("trade", "created"),
                    Attribute::new("trade_id", "1"),
                ]
            );
        }

        #[test]
        fn create_trade_and_add_funds() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
            let res = add_funds_to_trade_helper(deps.as_mut(), 0u64, Some(false));
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added funds", "trade"),
                    Attribute::new("trade_id", "0"),
                ]
            );

            // TODO, verify the query
        }

        #[test]
        fn create_trade_and_add_cw20_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());

            let res =
                add_cw20_to_trade_helper(deps.as_mut(), "token", "creator".to_string()).unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added token", "trade"),
                    Attribute::new("token", "token"),
                    Attribute::new("amount", "100"),
                ]
            );

            // TODO, verify the query

            // This triggers an error, the creator is not the same as the sender
            let err = add_cw20_to_trade_helper(deps.as_mut(), "token", "bad_person".to_string())
                .unwrap_err();

            assert_eq!(err, ContractError::TraderNotCreator {});
        }

        #[test]
        fn create_trade_and_add_cw721_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());

            let res =
                add_cw721_to_trade_helper(deps.as_mut(), "token", "creator".to_string()).unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added token", "trade"),
                    Attribute::new("nft", "token"),
                    Attribute::new("token_id", "58"),
                ]
            );

            // TODO, verify the query

            // This triggers an error, the creator is not the same as the sender
            let err = add_cw721_to_trade_helper(deps.as_mut(), "token", "bad_person".to_string())
                .unwrap_err();

            assert_eq!(err, ContractError::TraderNotCreator {});
        }

        #[test]
        fn confirm_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());

            let res = confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("confirmed", "trade"),
                    Attribute::new("trade", "0"),
                ]
            );

            //Wrong trade id
            let err = confirm_trade_helper(deps.as_mut(), "creator", 1).unwrap_err();
            assert_eq!(err, ContractError::NotFoundInTradeInfo {});

            //Wrong trader
            let err = confirm_trade_helper(deps.as_mut(), "bad_person", 0).unwrap_err();
            assert_eq!(err, ContractError::TraderNotCreator {});
        }

        #[test]
        fn accept_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            suggest_counter_trade_helper(deps.as_mut(), 0, Some(false)).unwrap();

            let err = accept_trade_helper(deps.as_mut(), "creator", 0, 5).unwrap_err();
            assert_eq!(err, ContractError::NotFoundInCounterTradeInfo {});

            let err = accept_trade_helper(deps.as_mut(), "creator", 1, 0).unwrap_err();
            assert_eq!(err, ContractError::NotFoundInTradeInfo {});

            let err = accept_trade_helper(deps.as_mut(), "bad_person", 0, 0).unwrap_err();
            assert_eq!(err, ContractError::TraderNotCreator {});

            let err = accept_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap_err();
            assert_eq!(err, ContractError::CantAcceptNotPublishedCounter {});

            confirm_counter_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();

            let res = accept_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("accepted", "trade"),
                    Attribute::new("trade", "0"),
                    Attribute::new("counter", "0"),
                ]
            );
        }
    }

    fn suggest_counter_trade_helper(
        deps: DepsMut,
        trade_id: u64,
        confirm: Option<bool>,
    ) -> Result<Response, ContractError> {
        let info = mock_info("creator", &[]);
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
        trade_id: u64,
        counter_id: u64,
        confirm: Option<bool>,
    ) -> Response {
        let info = mock_info("creator", &coins(2, "token"));
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
        .unwrap()
    }

    fn add_cw20_to_counter_trade_helper(
        deps: DepsMut,
        token: &str,
        sender: String,
    ) -> Result<Response, ContractError> {
        let info = mock_info(token, &[]);
        let env = mock_env();

        let msg = to_binary(&ReceiveMsg::AddToCounterTrade {
            trade_id: 0,
            counter_id: 0,
        })
        .unwrap();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::Receive {
                sender: sender,
                amount: Uint128::from(100u64),
                msg: msg,
            },
        )
    }

    fn add_cw721_to_counter_trade_helper(
        deps: DepsMut,
        token: &str,
        sender: String,
    ) -> Result<Response, ContractError> {
        let info = mock_info(token, &[]);
        let env = mock_env();

        let msg = to_binary(&ReceiveMsg::AddToCounterTrade {
            trade_id: 0,
            counter_id: 0,
        })
        .unwrap();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::ReceiveNft {
                sender: sender,
                token_id: "58".to_string(),
                msg: msg,
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
    pub mod counter_trade_tests {
        use super::*;

        #[test]
        fn create_counter_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());

            let err = suggest_counter_trade_helper(deps.as_mut(), 0, Some(false)).unwrap_err();

            assert_eq!(err, ContractError::NotCounterable {});

            let err = suggest_counter_trade_helper(deps.as_mut(), 1, Some(false)).unwrap_err();

            assert_eq!(err, ContractError::NotFoundInTradeInfo {});

            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            let res = suggest_counter_trade_helper(deps.as_mut(), 0, Some(false)).unwrap();

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

            create_trade_helper(deps.as_mut());
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), 0, None).unwrap();

            let res = add_funds_to_counter_trade_helper(deps.as_mut(), 0u64, 0u64, Some(false));

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added funds", "counter"),
                    Attribute::new("trade_id", "0"),
                    Attribute::new("counter_id", "0"),
                ]
            );

            // TODO, verify the query
        }

        #[test]
        fn create_trade_and_add_cw20_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), 0, None).unwrap();

            let res =
                add_cw20_to_counter_trade_helper(deps.as_mut(), "token", "creator".to_string())
                    .unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added token", "counter"),
                    Attribute::new("token", "token"),
                    Attribute::new("amount", "100"),
                ]
            );

            // TODO, verify the query

            // This triggers an error, the creator is not the same as the sender
            let err = add_cw20_to_trade_helper(deps.as_mut(), "token", "bad_person".to_string())
                .unwrap_err();

            assert_eq!(err, ContractError::TraderNotCreator {});
        }

        #[test]
        fn create_trade_and_add_cw721_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), 0, None).unwrap();

            let res =
                add_cw721_to_counter_trade_helper(deps.as_mut(), "token", "creator".to_string())
                    .unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added token", "counter"),
                    Attribute::new("nft", "token"),
                    Attribute::new("token_id", "58"),
                ]
            );

            // TODO, verify the query

            // This triggers an error, the creator is not the same as the sender
            let err =
                add_cw721_to_counter_trade_helper(deps.as_mut(), "token", "bad_person".to_string())
                    .unwrap_err();

            assert_eq!(err, ContractError::CounterTraderNotCreator {});
        }

        #[test]
        fn confirm_counter_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), 0, None).unwrap();

            let res = confirm_counter_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("confirmed", "counter"),
                    Attribute::new("trade", "0"),
                    Attribute::new("counter", "0"),
                ]
            );

            //Wrong trade id
            let err = confirm_counter_trade_helper(deps.as_mut(), "creator", 1, 0).unwrap_err();
            assert_eq!(err, ContractError::NotFoundInCounterTradeInfo {});

            //Wrong counter id
            let err = confirm_counter_trade_helper(deps.as_mut(), "creator", 0, 1).unwrap_err();
            assert_eq!(err, ContractError::NotFoundInCounterTradeInfo {});

            //Wrong trader
            let err = confirm_counter_trade_helper(deps.as_mut(), "bad_person", 0, 0).unwrap_err();
            assert_eq!(err, ContractError::CounterTraderNotCreator {});
        }
    }
}
