#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
};

use cw20_base::allowances::query_allowance;
use cw20_base::contract::{
    query_balance, query_download_logo, query_marketing_info, query_minter, query_token_info,
};

use cw20::Cw20ExecuteMsg;
use cw20_base::enumerable::{query_all_accounts, query_all_allowances};
use cw20_base::ContractError;
use cw721::Cw721ExecuteMsg;

use crate::msg::{into_cosmos_msg, ExecuteMsg, InstantiateMsg, QueryMsg, TokenToSend};
use crate::state::{ContractInfo, TOKEN_INFO};

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
    let data = ContractInfo { name: msg.name };
    TOKEN_INFO.save(deps.storage, &data)?;
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
        ExecuteMsg::Send { to_send, receivers } => send(deps, env, info, to_send, receivers),
    }
}

pub fn send(
    _deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    to_send: Vec<TokenToSend>,
    receivers: Vec<String>,
) -> Result<Response, ContractError> {
    if to_send.len() != receivers.len() {
        return Err(ContractError::Std(StdError::generic_err(
            "You need to have either as much receivers as tokens to send or just the one",
        )));
    }

    // We iterate over the tokens and create sendmsgs for each one
    // TODO : integrate a callback possibility, this is just a transfer function here
    let mut res = Response::new();
    for it in to_send.iter().zip(receivers.iter()) {
        let (token, receiver) = it;
        match token {
            TokenToSend::Cw20Coin(token) => {
                let message = Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.clone().into(),
                    recipient: receiver.clone(),
                    amount: token.amount,
                };
                res = res.add_message(into_cosmos_msg(message, token.address.clone())?);
            }
            TokenToSend::Cw721Coin(token) => {
                let message = Cw721ExecuteMsg::TransferNft {
                    recipient: receiver.clone(),
                    token_id: token.token_id.clone(),
                };
                res = res.add_message(into_cosmos_msg(message, token.address.clone())?);
            }
        }
    }
    Ok(res)
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
