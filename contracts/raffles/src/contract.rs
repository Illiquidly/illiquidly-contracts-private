use anyhow::{anyhow, Result};
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, Event, MessageInfo, Reply, Response, StdError,
    SubMsgResult, Uint128,
};
#[cfg(not(feature = "library"))]
use std::convert::TryInto;

use cw2::set_contract_version;

use crate::error::ContractError;

use crate::state::{get_raffle_state, is_owner, load_raffle, CONTRACT_INFO, RAFFLE_INFO};
use raffles_export::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use raffles_export::state::{
    ContractInfo, MINIMUM_RAFFLE_DURATION, MINIMUM_RAFFLE_TIMEOUT,
    MINIMUM_RAND_FEE, Randomness
};

use crate::execute::{
    execute_buy_ticket, execute_claim, execute_create_raffle, execute_receive,
    execute_receive_1155, execute_receive_nft, execute_update_randomness,
};
use crate::query::{query_all_raffles, query_contract_info, query_ticket_number, RaffleResponse};

const CONTRACT_NAME: &str = "illiquidlabs.io:raffles";
const CONTRACT_VERSION: &str = "0.1.0";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // Verify the contract name

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    msg.validate()?;
    // store token info
    let data = ContractInfo {
        name: msg.name,
        owner: deps
            .api
            .addr_validate(&msg.owner.unwrap_or_else(|| info.sender.to_string()))?,
        fee_addr: deps
            .api
            .addr_validate(&msg.fee_addr.unwrap_or_else(|| info.sender.to_string()))?,
        last_raffle_id: None,
        minimum_raffle_duration: msg
            .minimum_raffle_duration
            .unwrap_or(MINIMUM_RAFFLE_DURATION)
            .max(MINIMUM_RAFFLE_DURATION),
        minimum_raffle_timeout: msg
            .minimum_raffle_timeout
            .unwrap_or(MINIMUM_RAFFLE_TIMEOUT)
            .max(MINIMUM_RAFFLE_TIMEOUT),
        raffle_fee: msg.raffle_fee.unwrap_or(Uint128::zero()),
        rand_fee: msg
            .rand_fee
            .unwrap_or_else(|| Uint128::from(MINIMUM_RAND_FEE)),
        lock: false,
        drand_url: msg
            .drand_url
            .unwrap_or_else(|| "https://api.drand.sh/".to_string()),
        random_pubkey: msg.random_pubkey,
        verify_signature_contract: deps.api.addr_validate(&msg.verify_signature_contract)?,
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    Ok(Response::default()
        .add_attribute("action", "init")
        .add_attribute("contract", "raffle")
        .add_attribute("owner", data.owner))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> Result<Response> {
    match msg {
        ExecuteMsg::CreateRaffle {
            owner,
            asset,
            raffle_ticket_price,
            raffle_options
        } => execute_create_raffle(
            deps,
            env,
            info,
            owner,
            asset,
            raffle_ticket_price,
            raffle_options
        ),
        ExecuteMsg::BuyTicket {
            raffle_id,
            sent_assets,
        } => execute_buy_ticket(deps, env, info, raffle_id, sent_assets),
        ExecuteMsg::Receive {
            sender,
            amount,
            msg,
        } => execute_receive(deps, env, info, sender, amount, msg),
        ExecuteMsg::ReceiveNft {
            sender,
            token_id,
            msg,
        } => execute_receive_nft(deps, env, info, sender, token_id, msg),
        ExecuteMsg::Cw1155ReceiveMsg {
            operator,
            from,
            token_id,
            amount,
            msg,
        } => execute_receive_1155(
            deps,
            env,
            info,
            from.unwrap_or(operator),
            token_id,
            amount,
            msg,
        ),
        ExecuteMsg::ClaimNft { raffle_id } => execute_claim(deps, env, info, raffle_id),
        ExecuteMsg::UpdateRandomness {
            raffle_id,
            randomness,
        } => execute_update_randomness(deps, env, info, raffle_id, randomness),

        // Admin messages
        ExecuteMsg::ToggleLock { lock } => execute_toggle_lock(deps, env, info, lock),
        ExecuteMsg::Renounce {} => execute_renounce(deps, env, info),
        ExecuteMsg::ChangeParameter { parameter, value } => {
            execute_change_parameter(deps, env, info, parameter, value)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // No state migrations performed, just returned a Response
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary> {
    match msg {
        QueryMsg::ContractInfo {} => to_binary(&query_contract_info(deps)?).map_err(|x| anyhow!(x)),
        QueryMsg::RaffleInfo { raffle_id } => {
            let raffle_info = load_raffle(deps.storage, raffle_id)?;
            to_binary(&RaffleResponse {
                raffle_id,
                raffle_state: get_raffle_state(env, raffle_info.clone()),
                raffle_info: Some(raffle_info),
            })
            .map_err(|x| anyhow!(x))
        }

        QueryMsg::GetAllRaffles {
            start_after,
            limit,
            filters,
        } => to_binary(&query_all_raffles(deps, env, start_after, limit, filters)?)
            .map_err(|x| anyhow!(x)),

        QueryMsg::TicketNumber { owner, raffle_id } => {
            to_binary(&query_ticket_number(deps, env, raffle_id, owner)?).map_err(|x| anyhow!(x))
        }
    }
}

/// Replace the current contract owner with the provided owner address
/// * `owner` must be a valid Terra address
/// The owner has limited power on this contract :
/// 1. Change the contract owner
/// 2. Change the fee contract
pub fn execute_renounce(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    contract_info.owner = env.contract.address;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "modify_parameter")
        .add_attribute("parameter", "owner")
        .add_attribute("value", contract_info.owner))
}

pub fn execute_toggle_lock(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    lock: bool,
) -> Result<Response> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    contract_info.lock = lock;
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "modify_parameter")
        .add_attribute("parameter", "contract_lock")
        .add_attribute("value", lock.to_string()))
}

pub fn execute_change_parameter(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    parameter: String,
    value: String,
) -> Result<Response> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    match parameter.as_str() {
        "fee_addr" => {
            let addr = deps.api.addr_validate(&value)?;
            contract_info.fee_addr = addr;
        }
        "minimum_raffle_duration" => {
            let time = value.parse::<u64>()?;
            contract_info.minimum_raffle_duration = time;
        }
        "minimum_raffle_timeout" => {
            let time = value.parse::<u64>()?;
            contract_info.minimum_raffle_timeout = time;
        }
        "raffle_fee" => {
            let fee = Uint128::from(value.parse::<u128>()?);
            contract_info.raffle_fee = fee;
        }
        "rand_fee" => {
            let fee = Uint128::from(value.parse::<u128>()?);
            contract_info.rand_fee = fee;
        }
        "drand_url" => {
            contract_info.drand_url = value.clone();
        }
        "verify_signature_contract" => {
            let addr = deps.api.addr_validate(&value)?;
            contract_info.verify_signature_contract = addr;
        }
        "random_pubkey" => {
            contract_info.random_pubkey = Binary::from_base64(&value).unwrap();
        }
        _ => return Err(anyhow!(ContractError::ParameterNotFound {})),
    }

    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "modify_parameter")
        .add_attribute("parameter", parameter)
        .add_attribute("value", value))
}

// Messages triggered after random generation
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        0 => Ok(verify(deps, env, msg.result)?),
        _ => Err(ContractError::Unauthorized {}),
    }
}

pub fn verify(deps: DepsMut, _env: Env, msg: SubMsgResult) -> Result<Response, StdError> {
    match msg {
        SubMsgResult::Ok(subcall) => {
            let event: Event = subcall
                .events
                .into_iter()
                .find(|e| e.ty == "wasm")
                .ok_or_else(|| StdError::generic_err("no wasm result"))?;

            let round = event
                .attributes
                .clone()
                .into_iter()
                .find(|attr| attr.key == "round")
                .map_or(Err(StdError::generic_err("np round response")), |round| {
                    round
                        .value
                        .parse::<u64>()
                        .map_err(|_| StdError::generic_err("round value is shit"))
                })?;

            let randomness: String = event
                .attributes
                .clone()
                .into_iter()
                .find(|attr| attr.key == "randomness")
                .map(|rand| rand.value)
                .ok_or_else(|| StdError::generic_err("randomnesss value error"))?;

            let raffle_id: u64 = event
                .attributes
                .clone()
                .into_iter()
                .find(|attr| attr.key == "raffle_id")
                .map(|raffle_id| raffle_id.value.parse::<u64>())
                .transpose()
                .map_err(|_| StdError::generic_err("raffle_id parse error"))?
                .ok_or_else(|| StdError::generic_err("raffle_id parse error 1"))?;

            let owner = deps.api.addr_validate(
                &event
                    .attributes
                    .into_iter()
                    .find(|attr| attr.key == "owner")
                    .map(|raffle_id| raffle_id.value)
                    .ok_or_else(|| StdError::generic_err("owner parse err"))?,
            )?;

            let mut raffle_info = RAFFLE_INFO.load(deps.storage, raffle_id)?;
            println!("{:?}", Binary::from_base64(&randomness)?);
            raffle_info.randomness = Some(Randomness{
                randomness: Binary::from_base64(&randomness)?
                .as_slice()
                .try_into()
                .map_err(|_| StdError::generic_err("randomness parse err"))?,
                randomness_round: round,
                randomness_owner: owner.clone()
            });
            
            RAFFLE_INFO.save(deps.storage, raffle_id, &raffle_info)?;

            Ok(Response::new()
                .add_attribute("action", "update_randomness")
                .add_attribute("raffle_id", raffle_id.to_string())
                .add_attribute("sender", owner))
        }
        SubMsgResult::Err(_) => Err(StdError::generic_err("err")),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use cosmwasm_std::{
        coin, coins, from_binary,
        testing::{mock_dependencies, mock_env, mock_info, MOCK_CONTRACT_ADDR},
        Api, BankMsg, Coin, SubMsg, SubMsgResponse, Attribute
    };
    use raffles_export::msg::{into_cosmos_msg, DrandRandomness, QueryFilters, VerifierExecuteMsg};
    use raffles_export::state::{AssetInfo, Cw721Coin, RaffleInfo, RaffleState, RaffleOptions, RaffleOptionsMsg};
    extern crate rustc_serialize as serialize;
    use crate::query::AllRafflesResponse;
    use serialize::base64::{self, ToBase64};
    use serialize::hex::FromHex;
    
    use cw20::Cw20ExecuteMsg;
    use cw721::Cw721ExecuteMsg;
    use cw1155::Cw1155ExecuteMsg;

    const HEX_PUBKEY: &str = "868f005eb8e6e4ca0a47c8a77ceaa5309a47978a7c71bc5cce96366b5d7a569937c529eeda66c7293784a9402801af31";

    fn init_helper(deps: DepsMut) {
        let instantiate_msg = InstantiateMsg {
            name: "nft-raffle".to_string(),
            owner: None,
            random_pubkey: Binary::from_base64(
                &HEX_PUBKEY.from_hex().unwrap().to_base64(base64::STANDARD),
            )
            .unwrap(),
            drand_url: None,
            verify_signature_contract: "verifier".to_string(),
            fee_addr: None,
            minimum_raffle_timeout: None,
            minimum_raffle_duration: None,
            raffle_fee: Some(Uint128::from(2u128)),
            rand_fee: None,
            max_participant_number: None,
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();

        instantiate(deps, env, info, instantiate_msg).unwrap();
    }

    fn create_raffle(deps: DepsMut) -> Result<Response> {
        let info = mock_info("creator", &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::CreateRaffle {
                owner: None,
                asset: AssetInfo::Cw721Coin(Cw721Coin {
                    address: "nft".to_string(),
                    token_id: "token_id".to_string(),
                }),
                raffle_options: RaffleOptionsMsg::default(),
                raffle_ticket_price: AssetInfo::coin(10000u128, "uluna")
            },
        )
    }

    fn create_raffle_by_receiving(deps: DepsMut, nft: &str, token_id: &str) -> Result<Response> {
        let info = mock_info(nft, &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::ReceiveNft {
                sender: "creator".to_string(),
                token_id: token_id.to_string(),
                msg: to_binary(&ExecuteMsg::CreateRaffle {
                    owner: Some("creator".to_string()),
                    asset: AssetInfo::Cw721Coin(Cw721Coin {
                        address: nft.to_string(),
                        token_id: token_id.to_string(),
                    }),
                    raffle_options: RaffleOptionsMsg::default(),
                    raffle_ticket_price: AssetInfo::coin(10000u128, "uluna")
                }).unwrap()
            },
        )
    }

    fn create_raffle_comment(deps: DepsMut, comment: &str) -> Result<Response> {
        let info = mock_info("creator", &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::CreateRaffle {
                owner: None,
                asset: AssetInfo::Cw721Coin(Cw721Coin {
                    address: "nft".to_string(),
                    token_id: "token_id".to_string(),
                }),
                raffle_options: RaffleOptionsMsg{
                    comment: Some(comment.to_string()),
                    ..Default::default()
                },
                
                raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
            },
        )
    }

    fn create_raffle_cw20(deps: DepsMut) -> Result<Response> {
        let info = mock_info("creator", &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::CreateRaffle {
                owner: None,
                asset: AssetInfo::cw721("nft", "token_id"),
                raffle_options: RaffleOptionsMsg::default(),
                raffle_ticket_price: AssetInfo::cw20(10000u128, "address"),
            },
        )
    }

    fn create_raffle_cw1155(deps: DepsMut) -> Result<Response> {
        let info = mock_info("creator", &[]);
        let env = mock_env();

        execute(
            deps,
            env,
            info,
            ExecuteMsg::CreateRaffle {
                owner: None,
                asset: AssetInfo::cw1155("nft", "token_id", 675u128),
                raffle_options: RaffleOptionsMsg::default(),
                raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
            },
        )
    }

    fn buy_ticket_coin(
        deps: DepsMut,
        raffle_id: u64,
        buyer: &str,
        c: Coin,
        delta: u64,
    ) -> Result<Response> {
        let info = mock_info(buyer, &[c.clone()]);
        let mut env = mock_env();
        env.block.time = env.block.time.plus_seconds(delta);
        execute(
            deps,
            env,
            info,
            ExecuteMsg::BuyTicket {
                raffle_id,
                sent_assets: AssetInfo::Coin(c),
            },
        )
    }

    fn buy_ticket_cw20(
        deps: DepsMut,
        raffle_id: u64,
        buyer: &str,
        amount: u128,
        address: &str,
        delta: u64,
    ) -> Result<Response> {
        let info = mock_info(buyer, &[]);
        let mut env = mock_env();
        env.block.time = env.block.time.plus_seconds(delta);
        execute(
            deps,
            env,
            info,
            ExecuteMsg::BuyTicket {
                raffle_id,
                sent_assets: AssetInfo::cw20(amount, address),
            },
        )
    }

    fn claim_nft(deps: DepsMut, raffle_id: u64, time_delta: u64) -> Result<Response> {
        let info = mock_info("creator", &[]);
        let mut env = mock_env();
        env.block.time = env.block.time.plus_seconds(time_delta);
        execute(deps, env, info, ExecuteMsg::ClaimNft { raffle_id })
    }

    #[test]
    fn test_init_sanity() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
    }

    #[test]
    fn test_create_raffle() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        let response = create_raffle(deps.as_mut()).unwrap();

        assert_eq!(
            response.messages,
            vec![SubMsg::new(
                into_cosmos_msg(
                    Cw721ExecuteMsg::TransferNft {
                        recipient: MOCK_CONTRACT_ADDR.to_string(),
                        token_id: "token_id".to_string(),
                    },
                    "nft"
                )
                .unwrap()
            )]
        );
    }

    #[test]
    fn test_create_raffle_receive() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        let response = create_raffle_by_receiving(deps.as_mut(),"nft","token_id").unwrap();

        assert_eq!(
            response.attributes,
            vec![
                Attribute::new("action","create_raffle"),
                Attribute::new("raffle_id",0.to_string()),
                Attribute::new("owner","creator")
            ]
        );
    }

    #[test]
    fn test_claim_raffle() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        create_raffle(deps.as_mut()).unwrap();

        // Update the randomness internally
        let mut raffle_info = RAFFLE_INFO.load(&deps.storage, 0).unwrap();

        let mut randomness: [u8; 32] = [0; 32];
        hex::decode_to_slice(
            "89580f6a639add6c90dcf3d222e35415f89d9ee2cd6ef6fc4f23134cdffa5d1e",
            randomness.as_mut_slice(),
        )
        .unwrap();
        raffle_info.randomness = Some(Randomness{
            randomness,
            randomness_round:  2098475u64,
            randomness_owner: deps.api.addr_validate("rand_provider").unwrap()
        });
        RAFFLE_INFO
            .save(deps.as_mut().storage, 0, &raffle_info)
            .unwrap();

        claim_nft(deps.as_mut(), 0, 1000u64).unwrap();
    }

    #[test]
    fn test_ticket_and_claim_raffle() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        create_raffle(deps.as_mut()).unwrap();

        //Buy some tickets
        buy_ticket_coin(deps.as_mut(), 0, "first", coin(10, "uluna"), 0u64).unwrap_err();
        buy_ticket_coin(deps.as_mut(), 0, "first", coin(1000000, "uluna"), 0u64).unwrap_err();
        buy_ticket_coin(deps.as_mut(), 0, "first", coin(10000, "uluna"), 0u64).unwrap();
        buy_ticket_coin(deps.as_mut(), 0, "first", coin(10000, "uluna"), 0u64).unwrap();
        buy_ticket_coin(deps.as_mut(), 0, "second", coin(10000, "uluna"), 0u64).unwrap();
        buy_ticket_coin(deps.as_mut(), 0, "third", coin(10000, "uluna"), 0u64).unwrap();
        buy_ticket_coin(deps.as_mut(), 0, "fourth", coin(10000, "uluna"), 0u64).unwrap();

        // Update the randomness internally
        let mut raffle_info = RAFFLE_INFO.load(&deps.storage, 0).unwrap();

        let mut randomness: [u8; 32] = [0; 32];
        hex::decode_to_slice(
            "89580f6a639add6c90dcf3d222e35415f89d9ee2cd6ef6fc4f23134cdffa5d1e",
            randomness.as_mut_slice(),
        )
        .unwrap();
        raffle_info.randomness = Some(Randomness{
            randomness,
            randomness_round:  2098475u64,
            randomness_owner: deps.api.addr_validate("rand_provider").unwrap()
        });
        RAFFLE_INFO
            .save(deps.as_mut().storage, 0, &raffle_info)
            .unwrap();

        let response = claim_nft(deps.as_mut(), 0, 1000u64).unwrap();

        assert_eq!(
            response.messages,
            vec![
                SubMsg::new(
                    into_cosmos_msg(
                        Cw721ExecuteMsg::TransferNft {
                            recipient: "first".to_string(),
                            token_id: "token_id".to_string()
                        },
                        "nft".to_string()
                    )
                    .unwrap()
                ),
                SubMsg::new(BankMsg::Send {
                    to_address: "rand_provider".to_string(),
                    amount: coins(5, "uluna")
                }),
                SubMsg::new(BankMsg::Send {
                    to_address: "creator".to_string(),
                    amount: coins(10, "uluna")
                }),
                SubMsg::new(BankMsg::Send {
                    to_address: "creator".to_string(),
                    amount: coins(49985u128, "uluna")
                }),
            ]
        );

        // You can't buy tickets when the raffle is over
        buy_ticket_coin(deps.as_mut(), 0, "first", coin(10000, "uluna"), 100u64).unwrap_err();
        buy_ticket_coin(deps.as_mut(), 0, "first", coin(10000, "uluna"), 1000u64).unwrap_err();
    }

    #[test]
    fn test_ticket_and_claim_raffle_cw20() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        create_raffle_cw20(deps.as_mut()).unwrap();

        //Buy some tickets

        buy_ticket_cw20(deps.as_mut(), 0, "first", 100u128, "address", 0u64).unwrap_err();
        buy_ticket_cw20(deps.as_mut(), 0, "first", 1000000000u128, "address", 0u64).unwrap_err();

        let response =
            buy_ticket_cw20(deps.as_mut(), 0, "first", 10000u128, "address", 0u64).unwrap();
        assert_eq!(
            response.messages,
            vec![SubMsg::new(
                into_cosmos_msg(
                    Cw20ExecuteMsg::Transfer {
                        recipient: MOCK_CONTRACT_ADDR.to_string(),
                        amount: Uint128::from(10000u128),
                    },
                    "address".to_string()
                )
                .unwrap()
            )]
        );

        buy_ticket_cw20(deps.as_mut(), 0, "first", 10000u128, "address", 0u64).unwrap();
        buy_ticket_cw20(deps.as_mut(), 0, "second", 10000u128, "address", 0u64).unwrap();
        buy_ticket_cw20(deps.as_mut(), 0, "third", 10000u128, "address", 0u64).unwrap();
        buy_ticket_cw20(deps.as_mut(), 0, "fourth", 10000u128, "address", 0u64).unwrap();

        // Update the randomness internally
        let mut raffle_info = RAFFLE_INFO.load(&deps.storage, 0).unwrap();

        let mut randomness: [u8; 32] = [0; 32];
        hex::decode_to_slice(
            "89580f6a639add6c90dcf3d222e35415f89d9ee2cd6ef6fc4f23134cdffa5d1e",
            randomness.as_mut_slice(),
        )
        .unwrap();
        raffle_info.randomness = Some(Randomness{
            randomness,
            randomness_round:  2098475u64,
            randomness_owner: deps.api.addr_validate("rand_provider").unwrap()
        });
        RAFFLE_INFO
            .save(deps.as_mut().storage, 0, &raffle_info)
            .unwrap();

        let response = claim_nft(deps.as_mut(), 0, 1000u64).unwrap();

        assert_eq!(
            response.messages,
            vec![
                SubMsg::new(
                    into_cosmos_msg(
                        Cw721ExecuteMsg::TransferNft {
                            recipient: "first".to_string(),
                            token_id: "token_id".to_string()
                        },
                        "nft".to_string()
                    )
                    .unwrap()
                ),
                SubMsg::new(
                    into_cosmos_msg(
                        Cw20ExecuteMsg::Transfer {
                            recipient: "rand_provider".to_string(),
                            amount: Uint128::from(5u128)
                        },
                        "address".to_string()
                    )
                    .unwrap()
                ),
                SubMsg::new(
                    into_cosmos_msg(
                        Cw20ExecuteMsg::Transfer {
                            recipient: "creator".to_string(),
                            amount: Uint128::from(10u128)
                        },
                        "address".to_string()
                    )
                    .unwrap()
                ),
                SubMsg::new(
                    into_cosmos_msg(
                        Cw20ExecuteMsg::Transfer {
                            recipient: "creator".to_string(),
                            amount: Uint128::from(49985u128)
                        },
                        "address".to_string()
                    )
                    .unwrap()
                ),
            ]
        );

        // You can't buy tickets when the raffle is over
        buy_ticket_coin(deps.as_mut(), 0, "first", coin(10000, "uluna"), 100u64).unwrap_err();
        buy_ticket_coin(deps.as_mut(), 0, "first", coin(10000, "uluna"), 1000u64).unwrap_err();
    }
    #[test]
    fn test_ticket_and_claim_raffle_cw1155() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        let response = create_raffle_cw1155(deps.as_mut()).unwrap();

        assert_eq!(
            response.messages,
            vec![SubMsg::new(
                into_cosmos_msg(
                    Cw1155ExecuteMsg::SendFrom {
                        from: "creator".to_string(),
                        to: MOCK_CONTRACT_ADDR.to_string(),
                        token_id: "token_id".to_string(),
                        value: Uint128::from(675u128),
                        msg: None,
                    },
                    "nft"
                )
                .unwrap()
            )]
        );

        //Buy some tickets
        buy_ticket_coin(deps.as_mut(), 0, "first", coin(10, "uluna"), 0u64).unwrap_err();
        buy_ticket_coin(deps.as_mut(), 0, "first", coin(1000000, "uluna"), 0u64).unwrap_err();
        buy_ticket_coin(deps.as_mut(), 0, "first", coin(10000, "uluna"), 0u64).unwrap();
        buy_ticket_coin(deps.as_mut(), 0, "first", coin(10000, "uluna"), 0u64).unwrap();
        buy_ticket_coin(deps.as_mut(), 0, "second", coin(10000, "uluna"), 0u64).unwrap();
        buy_ticket_coin(deps.as_mut(), 0, "third", coin(10000, "uluna"), 0u64).unwrap();
        buy_ticket_coin(deps.as_mut(), 0, "fourth", coin(10000, "uluna"), 0u64).unwrap();

        // Update the randomness internally
        let mut raffle_info = RAFFLE_INFO.load(&deps.storage, 0).unwrap();

        let mut randomness: [u8; 32] = [0; 32];
        hex::decode_to_slice(
            "89580f6a639add6c90dcf3d222e35415f89d9ee2cd6ef6fc4f23134cdffa5d1e",
            randomness.as_mut_slice(),
        )
        .unwrap();
        raffle_info.randomness = Some(Randomness{
            randomness,
            randomness_round:  2098475u64,
            randomness_owner: deps.api.addr_validate("rand_provider").unwrap()
        });
        RAFFLE_INFO
            .save(deps.as_mut().storage, 0, &raffle_info)
            .unwrap();

        let response = claim_nft(deps.as_mut(), 0, 1000u64).unwrap();

        assert_eq!(
            response.messages,
            vec![
                SubMsg::new(
                    into_cosmos_msg(
                        Cw1155ExecuteMsg::SendFrom {
                            from: MOCK_CONTRACT_ADDR.to_string(),
                            to: "first".to_string(),
                            token_id: "token_id".to_string(),
                            value: Uint128::from(675u128),
                            msg: None,
                        },
                        "nft".to_string()
                    )
                    .unwrap()
                ),
                SubMsg::new(BankMsg::Send {
                    to_address: "rand_provider".to_string(),
                    amount: coins(5, "uluna")
                }),
                SubMsg::new(BankMsg::Send {
                    to_address: "creator".to_string(),
                    amount: coins(10, "uluna")
                }),
                SubMsg::new(BankMsg::Send {
                    to_address: "creator".to_string(),
                    amount: coins(49985u128, "uluna")
                }),
            ]
        );

        // You can't buy tickets when the raffle is over
        buy_ticket_coin(deps.as_mut(), 0, "first", coin(10000, "uluna"), 100u64).unwrap_err();
        buy_ticket_coin(deps.as_mut(), 0, "first", coin(10000, "uluna"), 1000u64).unwrap_err();
    }

    #[test]
    fn test_randomness_provider() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        create_raffle_cw1155(deps.as_mut()).unwrap();
        let mut env = mock_env();
        env.block.time = env.block.time.plus_seconds(2u64);
        let info = mock_info("anyone", &[]);
        let mut randomness = DrandRandomness {
            round: 90,
            signature: Binary::from_base64("quid").unwrap(),
            previous_signature: Binary::from_base64("quid").unwrap(),
        };
        let response = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::UpdateRandomness {
                raffle_id: 0,
                randomness: randomness.clone(),
            },
        )
        .unwrap();
        let msg = VerifierExecuteMsg::Verify {
            randomness: randomness.clone(),
            pubkey: Binary::from_base64(
                &HEX_PUBKEY.from_hex().unwrap().to_base64(base64::STANDARD),
            )
            .unwrap(),
            raffle_id: 0,
            owner: "anyone".to_string(),
        };

        assert_eq!(
            response.messages,
            vec![SubMsg::reply_on_success(
                into_cosmos_msg(msg, "verifier".to_string()).unwrap(),
                0
            )]
        );
        let random = "iVgPamOa3WyQ3PPSIuNUFfidnuLNbvb8TyMTTN/6XR4=";

        verify(
            deps.as_mut(),
            env.clone(),
            SubMsgResult::Ok(SubMsgResponse {
                events: vec![Event::new("wasm")
                    .add_attribute("round", 90u128.to_string())
                    .add_attribute("owner", "anyone")
                    .add_attribute("randomness", random)
                    .add_attribute("raffle_id", 0u128.to_string())],
                data: None,
            }),
        )
        .unwrap();

        randomness.round = 76;
        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::UpdateRandomness {
                raffle_id: 0,
                randomness: randomness.clone(),
            },
        )
        .unwrap_err();
        randomness.round = 90;
        execute(
            deps.as_mut(),
            env,
            info,
            ExecuteMsg::UpdateRandomness {
                raffle_id: 0,
                randomness,
            },
        )
        .unwrap_err();
    }

    // Admin functions
    #[test]
    fn test_renounce() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        let info = mock_info("bad_person", &[]);
        let env = mock_env();
        execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Renounce {}).unwrap_err();

        let info = mock_info("creator", &[]);
        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::Renounce {},
        )
        .unwrap();

        execute(deps.as_mut(), env, info, ExecuteMsg::Renounce {}).unwrap_err();
    }

    #[test]
    fn test_lock() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        assert!(!CONTRACT_INFO.load(&deps.storage).unwrap().lock);

        let info = mock_info("bad_person", &[]);
        let env = mock_env();
        execute(
            deps.as_mut(),
            env.clone(),
            info,
            ExecuteMsg::ToggleLock { lock: false },
        )
        .unwrap_err();

        let info = mock_info("creator", &[]);
        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::ToggleLock { lock: true },
        )
        .unwrap();
        assert!(CONTRACT_INFO.load(&deps.storage).unwrap().lock);

        execute(
            deps.as_mut(),
            env,
            info,
            ExecuteMsg::ToggleLock { lock: false },
        )
        .unwrap();
        assert!(!CONTRACT_INFO.load(&deps.storage).unwrap().lock);
    }

    #[test]
    fn test_change_parameter() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());

        let info = mock_info("bad_person", &[]);
        let env = mock_env();
        execute(
            deps.as_mut(),
            env.clone(),
            info,
            ExecuteMsg::ChangeParameter {
                parameter: "any".to_string(),
                value: "any".to_string(),
            },
        )
        .unwrap_err();

        let info = mock_info("creator", &[]);
        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::ChangeParameter {
                parameter: "any".to_string(),
                value: "any".to_string(),
            },
        )
        .unwrap_err();

        execute(
            deps.as_mut(),
            env,
            info,
            ExecuteMsg::ChangeParameter {
                parameter: "fee_addr".to_string(),
                value: "any".to_string(),
            },
        )
        .unwrap();

        assert_eq!(
            CONTRACT_INFO
                .load(&deps.storage)
                .unwrap()
                .fee_addr
                .to_string(),
            "any"
        );
    }

    // Query tests

    #[test]
    fn test_query_contract_info() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());
        let env = mock_env();
        let response = query(deps.as_ref(), env, QueryMsg::ContractInfo {}).unwrap();
        assert_eq!(
            from_binary::<ContractInfo>(&response).unwrap(),
            ContractInfo {
                name: "nft-raffle".to_string(),
                owner: deps.api.addr_validate("creator").unwrap(),
                fee_addr: deps.api.addr_validate("creator").unwrap(),
                last_raffle_id: None,
                minimum_raffle_duration: 1u64,
                minimum_raffle_timeout: 120u64,
                raffle_fee: Uint128::from(2u128), // in 10_000
                rand_fee: Uint128::from(1u64),    // in 10_000
                lock: false,
                drand_url: "https://api.drand.sh/".to_string(),
                verify_signature_contract: deps.api.addr_validate("verifier").unwrap(),
                random_pubkey: Binary::from_base64(
                    &HEX_PUBKEY.from_hex().unwrap().to_base64(base64::STANDARD)
                )
                .unwrap()
            }
        );
    }

    #[test]
    fn test_query_raffle_info() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());

        create_raffle(deps.as_mut()).unwrap();
        create_raffle_comment(deps.as_mut(), "random things my dude").unwrap();

        let env = mock_env();
        let response = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::RaffleInfo { raffle_id: 1 },
        )
        .unwrap();

        assert_eq!(
            from_binary::<RaffleResponse>(&response).unwrap(),
            RaffleResponse {
                raffle_id: 1,
                raffle_state: RaffleState::Started,
                raffle_info: Some(RaffleInfo {
                    owner: deps.api.addr_validate("creator").unwrap(),
                    asset: AssetInfo::cw721("nft", "token_id"),
                    raffle_options: RaffleOptions{
                        raffle_start_timestamp: env.block.time,
                        raffle_duration: 1u64,
                        raffle_timeout: 120u64,
                        comment: Some("random things my dude".to_string()),
                        max_participant_number: None,
                        max_ticket_per_address: None, 
                    },
                    raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
                    accumulated_ticket_fee: AssetInfo::coin(0u128, "uluna"),
                    number_of_tickets: 0u32,
                    randomness: None,
                    winner: None
                })
            }
        );
    }

    #[test]
    fn test_query_all_raffle_info() {
        let mut deps = mock_dependencies();
        init_helper(deps.as_mut());

        create_raffle(deps.as_mut()).unwrap();
        create_raffle_comment(deps.as_mut(), "random things my dude").unwrap();

        let env = mock_env();
        // Testing the general function
        let response = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::GetAllRaffles {
                start_after: None,
                limit: None,
                filters: None,
            },
        )
        .unwrap();

        assert_eq!(
            from_binary::<AllRafflesResponse>(&response)
                .unwrap()
                .raffles,
            vec![
                RaffleResponse {
                    raffle_id: 1,
                    raffle_state: RaffleState::Started,
                    raffle_info: Some(RaffleInfo {
                        owner: deps.api.addr_validate("creator").unwrap(),
                        asset: AssetInfo::cw721("nft", "token_id"),
                        raffle_options: RaffleOptions{
                            raffle_start_timestamp: env.block.time,
                            raffle_duration: 1u64,
                            raffle_timeout: 120u64,
                            comment: Some("random things my dude".to_string()),
                            max_participant_number: None,
                            max_ticket_per_address: None
                        },
                        raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
                        accumulated_ticket_fee: AssetInfo::coin(0u128, "uluna"),
                        number_of_tickets: 0u32,
                        randomness: None,
                        winner: None
                    })
                },
                RaffleResponse {
                    raffle_id: 0,
                    raffle_state: RaffleState::Started,
                    raffle_info: Some(RaffleInfo {
                        owner: deps.api.addr_validate("creator").unwrap(),
                        asset: AssetInfo::cw721("nft", "token_id"),
                        raffle_options: RaffleOptions{
                            raffle_start_timestamp: env.block.time,
                            raffle_duration: 1u64,
                            raffle_timeout: 120u64,
                            comment: None,
                            max_participant_number: None,
                            max_ticket_per_address: None
                        },
                        raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
                        accumulated_ticket_fee: AssetInfo::coin(0u128, "uluna"),
                        number_of_tickets: 0u32,
                        randomness: None,
                        winner: None
                    })
                }
            ]
        );

        // Testing the limit parameter
        let response = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::GetAllRaffles {
                start_after: None,
                limit: Some(1u32),
                filters: None,
            },
        )
        .unwrap();

        assert_eq!(
            from_binary::<AllRafflesResponse>(&response)
                .unwrap()
                .raffles,
            vec![RaffleResponse {
                raffle_id: 1,
                raffle_state: RaffleState::Started,
                raffle_info: Some(RaffleInfo {
                    owner: deps.api.addr_validate("creator").unwrap(),
                    asset: AssetInfo::cw721("nft", "token_id"),
                    raffle_options: RaffleOptions{
                        raffle_start_timestamp: env.block.time,
                        raffle_duration: 1u64,
                        raffle_timeout: 120u64,
                        comment: Some("random things my dude".to_string()),
                        max_participant_number: None,
                        max_ticket_per_address: None
                    },
                    raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
                    accumulated_ticket_fee: AssetInfo::coin(0u128, "uluna"),
                    number_of_tickets: 0u32,
                    randomness: None,
                    winner: None
                })
            }]
        );

        // Testing the start_after parameter
        let response = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::GetAllRaffles {
                start_after: Some(1u64),
                limit: None,
                filters: None,
            },
        )
        .unwrap();

        assert_eq!(
            from_binary::<AllRafflesResponse>(&response)
                .unwrap()
                .raffles,
            vec![RaffleResponse {
                raffle_id: 0,
                raffle_state: RaffleState::Started,
                raffle_info: Some(RaffleInfo {
                    owner: deps.api.addr_validate("creator").unwrap(),
                    asset: AssetInfo::cw721("nft", "token_id"),
                    raffle_options: RaffleOptions{
                        raffle_start_timestamp: env.block.time,
                        raffle_duration: 1u64,
                        raffle_timeout: 120u64,
                        comment: None,
                        max_participant_number: None,
                        max_ticket_per_address: None
                    },
                    raffle_ticket_price: AssetInfo::coin(10000u128, "uluna"),
                    accumulated_ticket_fee: AssetInfo::coin(0u128, "uluna"),
                    number_of_tickets: 0u32,
                    randomness: None,
                    winner: None
                })
            }]
        );

        // Testing the filter parameter
        buy_ticket_coin(deps.as_mut(), 1, "actor", coin(10000, "uluna"), 0u64).unwrap();
        let response = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::GetAllRaffles {
                start_after: None,
                limit: None,
                filters: Some(QueryFilters {
                    states: None,
                    owner: None,
                    ticket_depositor: Some("actor".to_string()),
                    contains_token: None,
                }),
            },
        )
        .unwrap();
        let raffles = from_binary::<AllRafflesResponse>(&response)
            .unwrap()
            .raffles;
        assert_eq!(raffles.len(), 1);
        assert_eq!(raffles[0].raffle_id, 1);

        buy_ticket_coin(deps.as_mut(), 0, "actor", coin(10000, "uluna"), 0u64).unwrap();
        buy_ticket_coin(deps.as_mut(), 0, "actor1", coin(10000, "uluna"), 0u64).unwrap();
        let response = query(
            deps.as_ref(),
            env,
            QueryMsg::GetAllRaffles {
                start_after: None,
                limit: None,
                filters: Some(QueryFilters {
                    states: None,
                    owner: None,
                    ticket_depositor: Some("actor".to_string()),
                    contains_token: None,
                }),
            },
        )
        .unwrap();
        let raffles = from_binary::<AllRafflesResponse>(&response)
            .unwrap()
            .raffles;
        assert_eq!(raffles.len(), 2);
    }
}
