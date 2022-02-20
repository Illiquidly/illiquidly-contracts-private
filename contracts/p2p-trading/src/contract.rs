#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128, BankMsg, StdError
};


use crate::error::ContractError;

use cw721::Cw721ExecuteMsg;
use cw20::Cw20ExecuteMsg;

use crate::state::{CONTRACT_INFO, TRADE_INFO, COUNTER_TRADE_INFO, load_trade, load_counter_trade};
use p2p_trading_export::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ReceiveMsg, into_cosmos_msg};
use p2p_trading_export::state::{ContractInfo, TradeState, AssetInfo, TradeInfo};

use crate::counter_trade::{
    add_funds_to_counter_trade, add_nft_to_counter_trade, add_token_to_counter_trade,
    confirm_counter_trade, suggest_counter_trade,
};
use crate::trade::{
    accept_trade, cancel_trade, add_funds_to_trade, add_nft_to_trade, add_token_to_trade, confirm_trade,
    create_trade, refuse_counter_trade,
};

use crate::messages::review_counter_trade;
use crate::query::{query_contract_info};

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

        // After Create Messages
        ExecuteMsg::CancelTrade {
            trade_id,
        } => cancel_trade(deps, env, info, trade_id),

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
) -> Result<Response, ContractError>{

    if trade_info.state == TradeState::Withdrawn{
        return Err(ContractError::TradeAlreadyWithdrawn {});
    }

    let mut res = Response::new();
    //We add tokens and nfts
    for fund in trade_info.associated_assets{
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
            to_address: info.sender.to_string(),
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
    if trade_info.state != TradeState::Accepted && trade_info.state != TradeState::Withdrawn{
        return Err(ContractError::TradeNotAccepted {});
    }

    let counter_id = trade_info.accepted_info.clone().ok_or(ContractError::ContractBug{})?.counter_id;
    let mut counter_info = COUNTER_TRADE_INFO.load(deps.storage, (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()))?;
    let trade_type: &str;

    let res;

    // We need to indentify who the transaction sender is (trader or counter-trader)
    if trade_info.owner == info.sender{
        // In case the trader wants to withdraw the exchanged funds
        res = create_withdraw_response(info,counter_info.clone())?;

        trade_type = "trade";
        counter_info.state = TradeState::Withdrawn;
        COUNTER_TRADE_INFO.save(deps.storage, (&trade_id.to_be_bytes(), &counter_id.to_be_bytes()), &counter_info)?;
    
    }else if counter_info.owner == info.sender{
        // In case the counter_trader wants to withdraw the exchanged funds
        res = create_withdraw_response(info,trade_info.clone())?;

        trade_type = "counter";
        trade_info.state = TradeState::Withdrawn;
        TRADE_INFO.save(deps.storage, &trade_id.to_be_bytes(), &trade_info)?;
    }else{
        return Err(ContractError::NotWithdrawableByYou{});
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
        QueryMsg::ContractInfo { } => to_binary(&query_contract_info(deps)?),
        QueryMsg::TradeInfo{ trade_id } => to_binary(&load_trade(deps.storage, trade_id)
            .map_err(|e|StdError::generic_err(e.to_string()))?),
        QueryMsg::CounterTradeInfo{ trade_id, counter_id } => to_binary(&load_counter_trade(deps.storage, trade_id, counter_id)
            .map_err(|e|StdError::generic_err(e.to_string()))?),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, Attribute, Coin};
    use crate::state::load_trade;
    use p2p_trading_export::state::{AssetInfo, Cw721Coin, Cw20Coin};

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

    fn add_funds_to_trade_helper(deps: DepsMut, trader: &str, trade_id: u64, coins_to_send: &Vec<Coin>, confirm: Option<bool>) -> Result<Response, ContractError> {
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
        sender: String,
        trade_id: u64
    ) -> Result<Response, ContractError> {
        let info = mock_info(token, &[]);
        let env = mock_env();

        let msg = to_binary(&ReceiveMsg::AddToTrade { trade_id }).unwrap();

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
        trade_id: u64,
    ) -> Result<Response, ContractError> {
        let info = mock_info(token, &[]);
        let env = mock_env();

        let msg = to_binary(&ReceiveMsg::AddToTrade { trade_id }).unwrap();

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

    pub mod trade_tests {
        use super::*;
        use p2p_trading_export::state::AcceptedTradeInfo;
        use cosmwasm_std::SubMsg;

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

            let new_trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(new_trade_info.state,TradeState::Created{});
        }

        #[test]
        fn create_trade_and_add_funds() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
            let res = add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(2, "token"), Some(false)).unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added funds", "trade"),
                    Attribute::new("trade_id", "0"),
                ]
            );
        }

        #[test]
        fn create_trade_and_add_funds_and_confirm() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
            let res = add_funds_to_trade_helper(deps.as_mut(), "creator", 0, &coins(2, "token"), Some(true)).unwrap();
            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added funds", "trade"),
                    Attribute::new("trade_id", "0"),
                ]
            );
            let new_trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(new_trade_info.state,TradeState::Published{});
        }

        #[test]
        fn create_trade_and_add_cw20_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());

            let res =
                add_cw20_to_trade_helper(deps.as_mut(), "token", "creator".to_string(), 0).unwrap();

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
            let err = add_cw20_to_trade_helper(deps.as_mut(), "token", "bad_person".to_string(), 0)
                .unwrap_err();

            assert_eq!(err, ContractError::TraderNotCreator {});
        }

        #[test]
        fn create_trade_and_add_cw721_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());

            let res =
                add_cw721_to_trade_helper(deps.as_mut(), "token", "creator".to_string(), 0).unwrap();

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
            let err = add_cw721_to_trade_helper(deps.as_mut(), "token", "bad_person".to_string(), 0)
                .unwrap_err();

            assert_eq!(err, ContractError::TraderNotCreator {});
        }

        #[test]
        fn confirm_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());

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

            let new_trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(new_trade_info.state,TradeState::Published{});

            //Already confirmed 
            let err = confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap_err();
            assert_eq!(err, ContractError::CantChangeTradeState {from: TradeState::Published, to: TradeState::Published});
        }

        #[test]
        fn confirm_trade_and_try_add_assets() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());

            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            
            // This triggers an error, we can't send funds to confirmed trade
            let err = add_funds_to_trade_helper(deps.as_mut(), "creator", 0,&coins(2, "token"), None)
                .unwrap_err();
            assert_eq!(err, ContractError::WrongTradeState{state: TradeState::Published});

            // This triggers an error, we can't send tokens to confirmed trade
            let err = add_cw20_to_trade_helper(deps.as_mut(), "token", "creator".to_string(), 0)
                .unwrap_err();
            assert_eq!(err, ContractError::WrongTradeState{state: TradeState::Published});

            // This triggers an error, we can't send nfts to confirmed trade
            let err = add_cw721_to_trade_helper(deps.as_mut(), "token", "creator".to_string(), 0)
                .unwrap_err();
            assert_eq!(err, ContractError::WrongTradeState{state: TradeState::Published});

        }

        #[test]
        fn accept_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
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
            assert_eq!(trade_info.state,TradeState::Accepted{});
            assert_eq!(trade_info.accepted_info.unwrap(),AcceptedTradeInfo{
                trade_id:0,
                counter_id:0
            });

            let counter_trade_info = load_counter_trade(&deps.storage, 0, 0).unwrap();
            assert_eq!(counter_trade_info.state,TradeState::Accepted{});
        }

        #[test]
        fn accept_trade_with_multiple_counter() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

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
            assert_eq!(trade_info.state,TradeState::Accepted{});

            let counter_trade_info = load_counter_trade(&deps.storage, 0, 0).unwrap();
            assert_eq!(counter_trade_info.state,TradeState::Accepted{});

            let counter_trade_info = load_counter_trade(&deps.storage, 0, 1).unwrap();
            assert_eq!(counter_trade_info.state,TradeState::Refused{});
        }

        #[test]
        fn cancel_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
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
        }

        #[test]
        fn withdraw_accepted_assets(){
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());
            create_trade_helper(deps.as_mut());
            add_funds_to_trade_helper(deps.as_mut(),"creator",0,&coins(5,"lunas"),None).unwrap();
            add_cw20_to_trade_helper(deps.as_mut(), "token", "creator".to_string(), 0).unwrap();
            add_cw721_to_trade_helper(deps.as_mut(), "nft", "creator".to_string(), 0).unwrap();
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(true)).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(false)).unwrap();

            add_funds_to_counter_trade_helper(deps.as_mut(), "counterer", 0, 1 ,Some(true), &coins(2, "token")).unwrap();
            add_cw20_to_counter_trade_helper(deps.as_mut(), "counter-token", "counterer".to_string(), 0, 1).unwrap();
            add_cw721_to_counter_trade_helper(deps.as_mut(), "counter-nft", "counterer".to_string(), 0, 1).unwrap();

            // Little test to start with (can't withdraw if the tade is not accepted)
            let err = withdraw_helper(deps.as_mut(),"anyone",0).unwrap_err();
            assert_eq!(err,ContractError::TradeNotAccepted{});

            accept_trade_helper(deps.as_mut(), "creator", 0, 1).unwrap();

            // Withdraw tests
            let err = withdraw_helper(deps.as_mut(),"bad_person",0).unwrap_err();
            assert_eq!(err,ContractError::NotWithdrawableByYou{});

            let res = withdraw_helper(deps.as_mut(),"creator",0).unwrap();
            assert_eq!(res.attributes,
                vec![
                    Attribute::new("withdraw funds", "trade"),
                    Attribute::new("trade", "0"),
                    Attribute::new("counter", "1"),
                ]
            );
            assert_eq!(res.messages,
                vec![
                    SubMsg::new(into_cosmos_msg(
                        Cw20ExecuteMsg::Transfer{
                            recipient: "creator".to_string(),
                            amount: Uint128::from(100u64)
                        },
                        "counter-token"
                    ).unwrap()),
                    SubMsg::new(into_cosmos_msg(
                        Cw721ExecuteMsg::TransferNft{
                            recipient: "creator".to_string(),
                            token_id: "58".to_string()
                        },
                        "counter-nft"
                    ).unwrap()),
                    SubMsg::new(BankMsg::Send {
                        to_address: "creator".to_string(),
                        amount: coins(2,"token"),
                    })
                ]
            );

            let err = withdraw_helper(deps.as_mut(),"creator",0).unwrap_err();
            assert_eq!(err,ContractError::TradeAlreadyWithdrawn{});

            let res = withdraw_helper(deps.as_mut(),"counterer",0).unwrap();
            assert_eq!(res.attributes,
                vec![
                    Attribute::new("withdraw funds", "counter"),
                    Attribute::new("trade", "0"),
                    Attribute::new("counter", "1"),
                ]
            );
            assert_eq!(res.messages,
                vec![
                    SubMsg::new(into_cosmos_msg(
                        Cw20ExecuteMsg::Transfer{
                            recipient: "counterer".to_string(),
                            amount: Uint128::from(100u64)
                        },
                        "token"
                    ).unwrap()),
                    SubMsg::new(into_cosmos_msg(
                        Cw721ExecuteMsg::TransferNft{
                            recipient: "counterer".to_string(),
                            token_id: "58".to_string()
                        },
                        "nft"
                    ).unwrap()),
                    SubMsg::new(BankMsg::Send {
                        to_address: "counterer".to_string(),
                        amount: coins(5,"lunas"),
                    }),
                ]
            );

            let err = withdraw_helper(deps.as_mut(),"counterer",0).unwrap_err();
            assert_eq!(err,ContractError::TradeAlreadyWithdrawn{});
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
        confirm: Option<bool>,
        coins_to_send: &Vec<Coin>
    ) -> Result<Response,ContractError> {
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
        sender: String,
        trade_id: u64,
        counter_id: u64
    ) -> Result<Response, ContractError> {
        let info = mock_info(token, &[]);
        let env = mock_env();

        let msg = to_binary(&ReceiveMsg::AddToCounterTrade {
            trade_id,
            counter_id,
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
        trade_id:u64,
        counter_id:u64
    ) -> Result<Response, ContractError> {
        let info = mock_info(token, &[]);
        let env = mock_env();

        let msg = to_binary(&ReceiveMsg::AddToCounterTrade {
            trade_id,
            counter_id,
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
                comment:Some("Shit NFT my girl".to_string()),
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

        execute(
            deps,
            env,
            info,
            ExecuteMsg::CancelTrade {
                trade_id,
            },
        )
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

        #[test]
        fn create_counter_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());

            let err = suggest_counter_trade_helper(deps.as_mut(),"counterer", 0, Some(false)).unwrap_err();

            assert_eq!(err, ContractError::NotCounterable {});

            let err = suggest_counter_trade_helper(deps.as_mut(),"counterer", 1, Some(false)).unwrap_err();

            assert_eq!(err, ContractError::NotFoundInTradeInfo {});

            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();

            let res = suggest_counter_trade_helper(deps.as_mut(),"counterer", 0, Some(false)).unwrap();

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
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();

            let res = add_funds_to_counter_trade_helper(deps.as_mut(), "counterer", 0u64, 0u64, Some(false),&coins(2, "token")).unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added funds", "counter"),
                    Attribute::new("trade_id", "0"),
                    Attribute::new("counter_id", "0"),
                ]
            );

            let counter_trade_info = load_counter_trade(&deps.storage, 0,0).unwrap();
            assert_eq!(counter_trade_info.state, TradeState::Created);
            assert_eq!(counter_trade_info.associated_funds,coins(2, "token"));
        }

        #[test]
        fn create_counter_trade_and_add_cw20_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();

            let res =
                add_cw20_to_counter_trade_helper(deps.as_mut(), "token", "counterer".to_string(), 0, 0)
                    .unwrap();

            assert_eq!(
                res.attributes,
                vec![
                    Attribute::new("added token", "counter"),
                    Attribute::new("token", "token"),
                    Attribute::new("amount", "100"),
                ]
            );

            let err = add_cw20_to_counter_trade_helper(deps.as_mut(), "token", "counterer".to_string(), 0, 1)
                    .unwrap_err();

            assert_eq!(err, ContractError::NotFoundInCounterTradeInfo{});

            // Verifying the state has been changed
            let trade_info = load_trade(&deps.storage, 0).unwrap();
            assert_eq!(trade_info.state, TradeState::Acknowledged);
            assert_eq!(trade_info.associated_assets,vec![]);


            let counter_trade_info = load_counter_trade(&deps.storage, 0, 0).unwrap();
            assert_eq!(counter_trade_info.state, TradeState::Created);
            assert_eq!(counter_trade_info.associated_assets,vec![
                AssetInfo::Cw20Coin(Cw20Coin{
                    address:"token".to_string(),
                    amount: Uint128::from(100u64)
                }),
            ]);

            // This triggers an error, the creator is not the same as the sender
            let err = add_cw20_to_counter_trade_helper(deps.as_mut(), "token", "bad_person".to_string(), 0, 0)
                .unwrap_err();

            assert_eq!(err, ContractError::CounterTraderNotCreator {});
        }

        #[test]
        fn create_trade_and_add_cw721_tokens() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();

            let res =
                add_cw721_to_counter_trade_helper(deps.as_mut(), "nft", "counterer".to_string(), 0, 0)
                    .unwrap();

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
            assert_eq!(trade_info.state, TradeState::Acknowledged);
            assert_eq!(trade_info.associated_assets,vec![]);


            let counter_trade_info = load_counter_trade(&deps.storage, 0, 0).unwrap();
            assert_eq!(counter_trade_info.state, TradeState::Created);
            assert_eq!(counter_trade_info.associated_assets,vec![
                AssetInfo::Cw721Coin(Cw721Coin{
                    address:"nft".to_string(),
                    token_id: "58".to_string()
                }),
            ]);

            // This triggers an error, the counter-trade creator is not the same as the sender
            let err =
                add_cw721_to_counter_trade_helper(deps.as_mut(), "token", "bad_person".to_string(), 0, 0)
                    .unwrap_err();

            assert_eq!(err, ContractError::CounterTraderNotCreator {});
        }

        #[test]
        fn confirm_counter_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
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
            assert_eq!(err, ContractError::CantChangeCounterTradeState {from: TradeState::Published, to: TradeState::Published});
        } 

        #[test]
        fn review_counter_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
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
            assert_eq!(err, ContractError::CantChangeCounterTradeState {from: TradeState::Created, to: TradeState::Created});

            confirm_counter_trade_helper(deps.as_mut(),"counterer",0,0).unwrap();

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
            assert_eq!(new_trade_info.state,TradeState::Acknowledged{});
        }

        #[test]
        fn review_counter_trade_when_accepted() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
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

            create_trade_helper(deps.as_mut());
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

            create_trade_helper(deps.as_mut());
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
            assert_eq!(new_trade_info.state,TradeState::Countered{});

        }

        #[test]
        fn refuse_counter_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();
            let res = refuse_counter_trade_helper(deps.as_mut(),"creator", 0, 0).unwrap();
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

            create_trade_helper(deps.as_mut());
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();
            // We suggest and confirm one more counter
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(true)).unwrap();
            // We suggest one more counter
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, Some(false)).unwrap();


            let res = refuse_counter_trade_helper(deps.as_mut(),"creator", 0, 0).unwrap();
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

            create_trade_helper(deps.as_mut());
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();

            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();
            accept_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();
            let err = refuse_counter_trade_helper(deps.as_mut(),"creator", 0, 0).unwrap_err();
            assert_eq!(err,ContractError::TradeAlreadyAccepted{});
        }

        #[test]
        fn cancel_accepted_counter_trade() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();

            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();
            accept_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();
            let err = cancel_trade_helper(deps.as_mut(),"creator", 0).unwrap_err();
            assert_eq!(err,ContractError::CantChangeTradeState {from: TradeState::Accepted, to: TradeState::Cancelled});
        }

        #[test]
        fn confirm_counter_trade_after_accepted() {
            let mut deps = mock_dependencies(&[]);
            init_helper(deps.as_mut());

            create_trade_helper(deps.as_mut());
            confirm_trade_helper(deps.as_mut(), "creator", 0).unwrap();
            suggest_counter_trade_helper(deps.as_mut(), "counterer", 0, None).unwrap();
            confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap();
            accept_trade_helper(deps.as_mut(), "creator", 0, 0).unwrap();

            //Already confirmed 
            let err = confirm_counter_trade_helper(deps.as_mut(), "counterer", 0, 0).unwrap_err();
            assert_eq!(err, ContractError::CantChangeTradeState {from: TradeState::Accepted, to: TradeState::Countered});
        } 
    }
}
