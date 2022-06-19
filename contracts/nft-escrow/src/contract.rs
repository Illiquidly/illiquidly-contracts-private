#[cfg(not(feature = "library"))]
use anyhow::{anyhow, Result};
use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response, StdError, StdResult,
};
use cw_storage_plus::{Bound};

use escrow_export::msg::{
    ContractInfoResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, ReceiveMsg,
    TokenInfoResponse, TokensResponse,
};
use escrow_export::state::{ContractInfo, TokenInfo};

use crate::error::ContractError;
use crate::state::{is_owner, CONTRACT_INFO, DEPOSITED_NFTS, USER_OWNED_NFTS};

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
        nft_address: deps.api.addr_validate(&msg.nft_address)?,
        owner: msg
            .owner
            .map(|x| deps.api.addr_validate(&x))
            .unwrap_or(Ok(info.sender))?,
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    Ok(Response::default().add_attribute("one_sided_escrow_contract", "init"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> Result<Response> {
    match msg {
        ExecuteMsg::ReceiveNft {
            sender,
            token_id,
            msg,
        } => execute_receive_nft(deps, env, info, sender, token_id, msg),

        ExecuteMsg::SetOwner { owner } => set_owner(deps, env, info, owner),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // No state migrations performed, just returned a Response
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary> {
    match msg {
        QueryMsg::ContractInfo {} => to_binary(&contract_info(deps)?).map_err(|e| anyhow!(e)),
        QueryMsg::RegisteredTokens { start_after, limit } => {
            to_binary(&registered_tokens(deps, start_after, limit)?).map_err(|e| anyhow!(e))
        }
        QueryMsg::Depositor { token_id } => {
            to_binary(&depositor(deps, token_id)?).map_err(|e| anyhow!(e))
        }
        QueryMsg::UserTokens {
            user,
            start_after,
            limit,
        } => to_binary(&user_tokens(deps, user, start_after, limit)?).map_err(|e| anyhow!(e)),
    }
}

pub fn contract_info(deps: Deps) -> Result<ContractInfoResponse> {
    CONTRACT_INFO
        .load(deps.storage)
        .map(|x| ContractInfoResponse {
            name: x.name,
            nft_address: x.nft_address.to_string(),
            owner: x.owner.to_string(),
        })
        .map_err(|e| anyhow!(e))
}

const DEFAULT_LIMIT: u32 = 10;
const MAX_LIMIT: u32 = 30;

pub fn user_tokens(
    deps: Deps,
    owner: String,
    start_after: Option<u32>,
    limit: Option<u32>,
) -> StdResult<TokensResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.unwrap_or(0u32) as usize;

    let owner_addr = deps.api.addr_validate(&owner)?;
    let tokens: Vec<_> = USER_OWNED_NFTS.load(deps.storage, &owner_addr)?;

    Ok(TokensResponse {
        tokens: tokens[start..(start + limit)].to_vec(),
    })
}

pub fn registered_tokens(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<TokenInfoResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let start = start_after.map(Bound::exclusive);
    let tokens = DEPOSITED_NFTS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|x| {
            x.map(|(key, depositor)| {
                let token_id = std::str::from_utf8(&key)
                    .map(|x| x.to_string())
                    .map_err(|_| {
                        StdError::generic_err("Error while getting utf8 transcript of keys")
                    })
                    .unwrap();
                TokenInfo {
                    token_id,
                    depositor: depositor.to_string(),
                }
            })
        })
        .collect::<Result<Vec<TokenInfo>, StdError>>()?;

    Ok(TokenInfoResponse { tokens })
}

pub fn depositor(deps: Deps, token_id: String) -> StdResult<String> {
    let owner: Addr = DEPOSITED_NFTS.load(deps.storage, &token_id)?;
    Ok(owner.to_string())
}

pub fn set_owner(deps: DepsMut, _env: Env, info: MessageInfo, owner: String) -> Result<Response> {
    is_owner(deps.as_ref(), info.sender)?;

    let owner_addr = deps.api.addr_validate(&owner)?;
    CONTRACT_INFO.update::<_, StdError>(deps.storage, |mut x| {
        x.owner = owner_addr;
        Ok(x)
    })?;

    Ok(Response::new()
        .add_attribute("action", "parameter_update")
        .add_attribute("parameter", "owner")
        .add_attribute("value", owner))
}

pub fn execute_receive_nft(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    sender: String,
    token_id: String,
    msg: Binary,
) -> Result<Response> {
    match from_binary(&msg)? {
        ReceiveMsg::DepositNft {
            token_id: msg_token_id,
        } => {
            // We assert the message matches the sent token
            if token_id != msg_token_id {
                return Err(anyhow!(ContractError::IncorrectTokenId {}));
            }
            // We make sure the nft matches the contract nft
            let contract_info = CONTRACT_INFO.load(deps.storage)?;
            if contract_info.nft_address != info.sender {
                return Err(anyhow!(ContractError::IncorrectContract {}));
            }
            // We save the token to memory
            let sender_addr = deps.api.addr_validate(&sender)?;
            DEPOSITED_NFTS.save(deps.storage, &token_id, &sender_addr)?;
            USER_OWNED_NFTS.update::<_, anyhow::Error>(
                deps.storage,
                &sender_addr,
                |x| match x {
                    Some(mut tokens) => {
                        tokens.push(token_id.clone());
                        Ok(tokens)
                    }
                    None => Ok(vec![token_id.clone()]),
                },
            )?;
            Ok(Response::new()
                .add_attribute("action", "deposit_nft")
                .add_attribute("address", contract_info.nft_address)
                .add_attribute("token_id", token_id)
                .add_attribute("depositor", sender))
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::Api;

    fn init_helper(deps: DepsMut) -> Response {
        let instantiate_msg = InstantiateMsg {
            name: "escrow".to_string(),
            owner: None,
            nft_address: "nft".to_string(),
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();

        instantiate(deps, env, info, instantiate_msg).unwrap()
    }

    #[test]
    fn test_init_sanity() {
        let mut deps = mock_dependencies(&[]);
        let res = init_helper(deps.as_mut());
        assert_eq!(0, res.messages.len());
    }

    fn deposit_helper(
        deps: DepsMut,
        nft: &str,
        token_id: &str,
        token_id1: &str,
    ) -> Result<Response> {
        let env = mock_env();
        let info = mock_info(nft, &[]);
        execute(
            deps,
            env,
            info,
            ExecuteMsg::ReceiveNft {
                sender: "creator".to_string(),
                token_id: token_id.to_string(),
                msg: to_binary(&ReceiveMsg::DepositNft {
                    token_id: token_id1.to_string(),
                })
                .unwrap(),
            },
        )
    }

    #[test]
    fn test_deposit_nft() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let err = deposit_helper(deps.as_mut(), "other_nft", "id", "id").unwrap_err();
        assert_eq!(
            err.downcast::<ContractError>().unwrap(),
            ContractError::IncorrectContract {}
        );

        let err = deposit_helper(deps.as_mut(), "nft", "id", "id1").unwrap_err();
        assert_eq!(
            err.downcast::<ContractError>().unwrap(),
            ContractError::IncorrectTokenId {}
        );

        deposit_helper(deps.as_mut(), "nft", "id", "id").unwrap();

        // We verify both intermediary variables have been updated
        let addr = deps.api.addr_validate("creator").unwrap();
        let deposit = USER_OWNED_NFTS.load(&deps.storage, &addr).unwrap();
        assert_eq!(deposit, vec!["id".to_string()]);

        let deposit = DEPOSITED_NFTS.load(&deps.storage, "id").unwrap();
        assert_eq!(deposit, addr);
    }

    #[test]
    fn test_deposit_multiple_nft() {
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        deposit_helper(deps.as_mut(), "nft", "id", "id").unwrap();
        deposit_helper(deps.as_mut(), "nft", "id1", "id1").unwrap();
        deposit_helper(deps.as_mut(), "nft", "id2", "id2").unwrap();
        deposit_helper(deps.as_mut(), "nft", "id3", "id3").unwrap();
        deposit_helper(deps.as_mut(), "nft", "id4", "id4").unwrap();

        // We verify both intermediary variables have been updated
        let addr = deps.api.addr_validate("creator").unwrap();
        let deposit = USER_OWNED_NFTS.load(&deps.storage, &addr).unwrap();
        assert_eq!(
            deposit,
            vec![
                "id".to_string(),
                "id1".to_string(),
                "id2".to_string(),
                "id3".to_string(),
                "id4".to_string()
            ]
        );
        let deposit = DEPOSITED_NFTS.load(&deps.storage, "id").unwrap();
        assert_eq!(deposit, addr);
        let deposit = DEPOSITED_NFTS.load(&deps.storage, "id1").unwrap();
        assert_eq!(deposit, addr);
        let deposit = DEPOSITED_NFTS.load(&deps.storage, "id2").unwrap();
        assert_eq!(deposit, addr);
        let deposit = DEPOSITED_NFTS.load(&deps.storage, "id3").unwrap();
        assert_eq!(deposit, addr);
        let deposit = DEPOSITED_NFTS.load(&deps.storage, "id4").unwrap();
        assert_eq!(deposit, addr);
    }

    #[test]
    fn test_query_contract_info() {
        let env = mock_env();
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        let res = query(deps.as_ref(), env, QueryMsg::ContractInfo {}).unwrap();
        assert_eq!(
            from_binary::<ContractInfoResponse>(&res).unwrap(),
            ContractInfoResponse {
                name: "escrow".to_string(),
                nft_address: "nft".to_string(),
                owner: "creator".to_string()
            }
        )
    }

    #[test]
    fn test_query_deposited_nft() {
        let env = mock_env();
        let mut deps = mock_dependencies(&[]);
        init_helper(deps.as_mut());

        deposit_helper(deps.as_mut(), "nft", "id", "id").unwrap();
        deposit_helper(deps.as_mut(), "nft", "id1", "id1").unwrap();
        deposit_helper(deps.as_mut(), "nft", "id2", "id2").unwrap();

        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::RegisteredTokens {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
        assert_eq!(
            from_binary::<TokenInfoResponse>(&res).unwrap(),
            TokenInfoResponse {
                tokens: vec![
                    TokenInfo {
                        depositor: "creator".to_string(),
                        token_id: "id".to_string()
                    },
                    TokenInfo {
                        depositor: "creator".to_string(),
                        token_id: "id1".to_string()
                    },
                    TokenInfo {
                        depositor: "creator".to_string(),
                        token_id: "id2".to_string()
                    }
                ]
            }
        );

        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::RegisteredTokens {
                start_after: Some("id".to_string()),
                limit: None,
            },
        )
        .unwrap();
        assert_eq!(
            from_binary::<TokenInfoResponse>(&res).unwrap(),
            TokenInfoResponse {
                tokens: vec![
                    TokenInfo {
                        depositor: "creator".to_string(),
                        token_id: "id1".to_string()
                    },
                    TokenInfo {
                        depositor: "creator".to_string(),
                        token_id: "id2".to_string()
                    }
                ]
            }
        );

        let res = query(
            deps.as_ref(),
            env,
            QueryMsg::RegisteredTokens {
                start_after: Some("id".to_string()),
                limit: Some(1u32),
            },
        )
        .unwrap();
        assert_eq!(
            from_binary::<TokenInfoResponse>(&res).unwrap(),
            TokenInfoResponse {
                tokens: vec![TokenInfo {
                    depositor: "creator".to_string(),
                    token_id: "id1".to_string()
                }]
            }
        );
    }
}
