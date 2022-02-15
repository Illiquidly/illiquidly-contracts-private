#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary,  Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128, 
};

use cw20_base::allowances::query_allowance;
use cw20_base::contract::{
    query_balance, query_download_logo, query_marketing_info, query_minter, query_token_info,
};

use crate::error::ContractError;
use cw20_base::enumerable::{query_all_accounts, query_all_allowances};

use crate::state::{CONTRACT_INFO};
use p2p_trading_export::msg::{
    ExecuteMsg, InstantiateMsg, QueryMsg, ReceiveMsg,
};
use p2p_trading_export::state::{ContractInfo};

use crate::trade::{create_trade, add_funds_to_trade, add_token_to_trade, add_nft_to_trade, confirm_trade};
use crate::counter_trade::{suggest_counter_trade, add_funds_to_counter_trade, add_token_to_counter_trade, add_nft_to_counter_trade, confirm_counter_trade};

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
        last_trade_id: 0,
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
        ExecuteMsg::SuggestCounterTrade { trade_id, confirm } => suggest_counter_trade(deps,env, info, trade_id, confirm),
        
        ExecuteMsg::AddFundsToCounterTrade { trade_id, counter_id, confirm } => {
            add_funds_to_counter_trade(deps, env, info, trade_id, counter_id, confirm)
        },
        ExecuteMsg::ConfirmCounterTrade { trade_id, counter_id } => confirm_counter_trade(deps, env, info, trade_id, counter_id),
        
        // Generic (will have to remove at the end of development)
        _ => {
            return Err(ContractError::Std(StdError::generic_err(
                "Ow whaou, please wait just a bit, it's not implemented yet !",
            )));
        },
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

/*
// We gather the sent tokens in a specific structure
let mut to_save: Vec<FundsInfo> = to_send.iter().map(|x| {
    match x {
        TokenToSend::Cw20Coin(c) => FundsInfo::Cw20Coin(c.clone()),
        TokenToSend::Cw721Coin(c) => FundsInfo::Cw721Coin(c.clone())
    }
}).collect();
*/

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
