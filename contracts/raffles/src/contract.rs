use anyhow::{anyhow, Result};
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Coin, Deps, DepsMut, Env, Event, MessageInfo, Reply,
    Response, StdError, StdResult, SubMsgResult, Timestamp, Uint128,
};
#[cfg(not(feature = "library"))]
use std::convert::TryInto;

use cw2::set_contract_version;

use crate::error::ContractError;

use crate::state::{
    assert_randomness_origin_and_order, can_buy_ticket, get_asset_amount,
    get_raffle_owner_finished_messages, get_raffle_state, get_raffle_winner,
    get_raffle_winner_message, is_owner, load_raffle, CONTRACT_INFO, RAFFLE_INFO,
};
use raffles_export::msg::{
    into_cosmos_msg, DrandRandomness, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
};
use raffles_export::state::{
    AssetInfo, ContractInfo, Cw1155Coin, Cw20Coin, Cw721Coin, RaffleInfo, RaffleState,
    MAXIMUM_PARTICIPANT_NUMBER, MINIMUM_RAFFLE_DURATION, MINIMUM_RAFFLE_TIMEOUT, MINIMUM_RAND_FEE,
};

use cw1155::Cw1155ExecuteMsg;
use cw20::Cw20ExecuteMsg;
use cw721::Cw721ExecuteMsg;

use crate::query::{query_all_raffles, query_contract_info};

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
            asset,
            raffle_start_timestamp,
            raffle_duration,
            raffle_timeout,
            comment,
            raffle_ticket_price,
            max_participant_number,
        } => execute_create_raffle(
            deps,
            env,
            info,
            asset,
            raffle_start_timestamp,
            raffle_duration,
            raffle_timeout,
            comment,
            raffle_ticket_price,
            max_participant_number,
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
            to_binary(&load_raffle(deps.storage, raffle_id)?).map_err(|x| anyhow!(x))
        }

        QueryMsg::GetAllRaffles {
            start_after,
            limit,
            filters,
        } => to_binary(&query_all_raffles(deps, env, start_after, limit, filters)?)
            .map_err(|x| anyhow!(x)),
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

/// Replace the current fee_contract with the provided fee_contract address
/// * `fee_addr` must be a valid Terra address
pub fn set_new_fee_addr(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    fee_addr: String,
) -> Result<Response, ContractError> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    let fee_addr = deps.api.addr_validate(&fee_addr)?;
    contract_info.fee_addr = fee_addr.clone();
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "modify_parameter")
        .add_attribute("parameter", "fee_addr")
        .add_attribute("value", fee_addr))
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

/// Create a new raffle by depositing assets
#[allow(clippy::too_many_arguments)]
pub fn execute_create_raffle(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset: AssetInfo,
    raffle_start_timestamp: Option<u64>,
    raffle_duration: Option<u64>,
    raffle_timeout: Option<u64>,
    comment: Option<String>,
    raffle_ticket_price: AssetInfo,
    max_participant_number: Option<u64>,
) -> Result<Response> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    if contract_info.lock {
        return Err(anyhow!(ContractError::ContractIsLocked {}));
    }

    // First we physcially transfer the AssetInfo
    let transfer_message = match &asset {
        AssetInfo::Cw721Coin(token) => {
            let message = Cw721ExecuteMsg::TransferNft {
                recipient: env.contract.address.clone().into(),
                token_id: token.token_id.clone(),
            };

            into_cosmos_msg(message, token.address.clone())?
        }
        AssetInfo::Cw1155Coin(token) => {
            let message = Cw1155ExecuteMsg::SendFrom {
                from: info.sender.to_string(),
                to: env.contract.address.clone().into(),
                token_id: token.token_id.clone(),
                value: token.value,
                msg: None,
            };

            into_cosmos_msg(message, token.address.clone())?
        }
        _ => return Err(anyhow!(ContractError::WrongAssetType {})),
    };
    // Then we create the internal raffle structure
    let raffle_id = _create_raffle(
        deps,
        env,
        info.sender.clone(),
        asset,
        raffle_start_timestamp,
        raffle_duration,
        raffle_timeout,
        comment,
        raffle_ticket_price,
        max_participant_number,
    )?;

    Ok(Response::new()
        .add_message(transfer_message)
        .add_attribute("action", "create_raffle")
        .add_attribute("raffle_id", raffle_id.to_string())
        .add_attribute("owner", info.sender))
}

/// Create a new raffle by depositing assets
pub fn execute_receive_nft(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: String,
    token_id: String,
    msg: Binary,
) -> Result<Response> {
    let sender = deps.api.addr_validate(&sender)?;
    match from_binary(&msg)? {
        ExecuteMsg::CreateRaffle {
            asset,
            raffle_start_timestamp,
            raffle_duration,
            raffle_timeout,
            comment,
            raffle_ticket_price,
            max_participant_number,
        } => {
            // First we make sure the received NFT is the one specified in the message

            match asset.clone() {
                AssetInfo::Cw721Coin(Cw721Coin {
                    address: address_received,
                    token_id: token_id_received,
                }) => {
                    if deps.api.addr_validate(&address_received)? == info.sender
                        && token_id_received == token_id
                    {
                        // The asset is a match, we can create the raffle object and return
                        let raffle_id = _create_raffle(
                            deps,
                            env,
                            sender,
                            asset,
                            raffle_start_timestamp,
                            raffle_duration,
                            raffle_timeout,
                            comment,
                            raffle_ticket_price,
                            max_participant_number,
                        )?;

                        Ok(Response::new()
                            .add_attribute("action", "create_raffle")
                            .add_attribute("raffle_id", raffle_id.to_string())
                            .add_attribute("owner", info.sender))
                    } else {
                        Err(anyhow!(ContractError::AssetMismatch {}))
                    }
                }
                _ => Err(anyhow!(ContractError::AssetMismatch {})),
            }
        }
        _ => Err(anyhow!(ContractError::Unauthorized {})),
    }
}

/// Create a new raffle by depositing assets
pub fn execute_receive_1155(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    from: String,
    token_id: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response> {
    let sender = deps.api.addr_validate(&from)?;
    match from_binary(&msg)? {
        ExecuteMsg::CreateRaffle {
            asset,
            raffle_start_timestamp,
            raffle_duration,
            raffle_timeout,
            comment,
            raffle_ticket_price,
            max_participant_number,
        } => {
            // First we make sure the received NFT is the one specified in the message
            match asset.clone() {
                AssetInfo::Cw1155Coin(Cw1155Coin {
                    address: address_received,
                    token_id: token_id_received,
                    value: value_received,
                }) => {
                    if deps.api.addr_validate(&address_received)? == info.sender
                        && token_id_received == token_id
                        && value_received == amount
                    {
                        // The asset is a match, we can create the raffle object and return
                        let raffle_id = _create_raffle(
                            deps,
                            env,
                            sender,
                            asset,
                            raffle_start_timestamp,
                            raffle_duration,
                            raffle_timeout,
                            comment,
                            raffle_ticket_price,
                            max_participant_number,
                        )?;

                        Ok(Response::new()
                            .add_attribute("action", "create_raffle")
                            .add_attribute("raffle_id", raffle_id.to_string())
                            .add_attribute("owner", info.sender))
                    } else {
                        Err(anyhow!(ContractError::AssetMismatch {}))
                    }
                }
                _ => Err(anyhow!(ContractError::AssetMismatch {})),
            }
        }
        _ => Err(anyhow!(ContractError::Unauthorized {})),
    }
}

/// Create a new raffle and assign it a unique id
#[allow(clippy::too_many_arguments)]
pub fn _create_raffle(
    deps: DepsMut,
    env: Env,
    owner: Addr,
    asset: AssetInfo,
    raffle_start_timestamp: Option<u64>,
    raffle_duration: Option<u64>,
    raffle_timeout: Option<u64>,
    comment: Option<String>,
    raffle_ticket_price: AssetInfo,
    max_participant_number: Option<u64>,
) -> Result<u64> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    // We start by creating a new trade_id (simply incremented from the last id)
    let raffle_id: u64 = CONTRACT_INFO
        .update(deps.storage, |mut c| -> StdResult<_> {
            c.last_raffle_id = c.last_raffle_id.map_or(Some(0), |id| Some(id + 1));
            Ok(c)
        })?
        .last_raffle_id
        .unwrap(); // This is safe because of the function architecture just there

    RAFFLE_INFO.update(deps.storage, raffle_id, |trade| match trade {
        // If the trade id already exists, the contract is faulty
        // Or an external error happened, or whatever...
        // In that case, we emit an error
        // The priority is : We do not want to overwrite existing data
        Some(_) => Err(ContractError::ExistsInRaffleInfo {}),
        None => Ok(RaffleInfo {
            owner,
            asset,
            raffle_start_timestamp: raffle_start_timestamp
                .map(Timestamp::from_seconds)
                .unwrap_or(env.block.time),
            raffle_duration: raffle_duration
                .unwrap_or(contract_info.minimum_raffle_duration)
                .max(contract_info.minimum_raffle_duration),
            raffle_timeout: raffle_timeout
                .unwrap_or(contract_info.minimum_raffle_timeout)
                .max(contract_info.minimum_raffle_timeout),
            comment,
            raffle_ticket_price: raffle_ticket_price.clone(),
            accumulated_ticket_fee: match raffle_ticket_price {
                AssetInfo::Cw20Coin(coin) => AssetInfo::Cw20Coin(Cw20Coin {
                    address: coin.address,
                    amount: Uint128::zero(),
                }),
                AssetInfo::Coin(coin) => AssetInfo::Coin(Coin {
                    denom: coin.denom,
                    amount: Uint128::zero(),
                }),
                _ => return Err(ContractError::WrongFundsType {}),
            },
            tickets: vec![],
            randomness_owner: None,
            randomness: <[u8; 32]>::default(),
            randomness_round: 0,
            max_participant_number: max_participant_number
                .unwrap_or(MAXIMUM_PARTICIPANT_NUMBER)
                .min(MAXIMUM_PARTICIPANT_NUMBER),
            winner: None,
        }),
    })?;
    Ok(raffle_id)
}

pub fn execute_buy_ticket(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    raffle_id: u64,
    assets: AssetInfo,
) -> Result<Response> {
    // First we physcially transfer the AssetInfo
    let transfer_messages = match &assets {
        AssetInfo::Cw20Coin(token) => {
            let message = Cw20ExecuteMsg::Transfer {
                recipient: env.contract.address.clone().into(),
                amount: token.amount,
            };

            vec![into_cosmos_msg(message, token.address.clone())?]
        }
        // or verify the sent coins match the message coins
        AssetInfo::Coin(coin) => {
            if info.funds.len() != 1 {
                return Err(anyhow!(ContractError::AssetMismatch {}));
            }
            if info.funds[0].denom != coin.denom || info.funds[0].amount != coin.amount {
                return Err(anyhow!(ContractError::AssetMismatch {}));
            }
            vec![]
        }
        _ => return Err(anyhow!(ContractError::WrongAssetType {})),
    };
    // Then we verify the funds sent match the raffle conditions and we save the ticket that was bought
    _buy_ticket(deps, env, info.sender.clone(), raffle_id, assets)?;

    Ok(Response::new()
        .add_messages(transfer_messages)
        .add_attribute("action", "buy_ticket")
        .add_attribute("raffle_id", raffle_id.to_string())
        .add_attribute("owner", info.sender))
}
/// Create a new raffle by depositing assets
pub fn execute_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response> {
    let sender = deps.api.addr_validate(&sender)?;
    match from_binary(&msg)? {
        ExecuteMsg::BuyTicket {
            raffle_id,
            sent_assets,
        } => {
            // First we make sure the received Assets is the one specified in the message
            match sent_assets.clone() {
                AssetInfo::Cw20Coin(Cw20Coin {
                    address: address_received,
                    amount: amount_received,
                }) => {
                    if deps.api.addr_validate(&address_received)? == info.sender
                        && amount_received == amount
                    {
                        // The asset is a match, we can create the raffle object and return
                        _buy_ticket(deps, env, sender, raffle_id, sent_assets)?;

                        Ok(Response::new()
                            .add_attribute("action", "create_raffle")
                            .add_attribute("raffle_id", raffle_id.to_string())
                            .add_attribute("owner", info.sender))
                    } else {
                        Err(anyhow!(ContractError::AssetMismatch {}))
                    }
                }
                _ => Err(anyhow!(ContractError::AssetMismatch {})),
            }
        }
        _ => Err(anyhow!(ContractError::Unauthorized {})),
    }
}

pub fn _buy_ticket(
    deps: DepsMut,
    env: Env,
    owner: Addr,
    raffle_id: u64,
    assets: AssetInfo,
) -> Result<()> {
    let mut raffle_info = RAFFLE_INFO.load(deps.storage, raffle_id)?;

    // We first check the raffle is in the right state
    can_buy_ticket(env, raffle_info.clone())?;

    // We then check the sent assets match the raffle assets
    if raffle_info.raffle_ticket_price != assets {
        return Err(anyhow!(ContractError::PaiementNotSufficient {
            assets_wanted: raffle_info.raffle_ticket_price,
            assets_received: assets
        }));
    }

    // Then we save the sender to the bought tickets
    if raffle_info.tickets.len() >= raffle_info.max_participant_number as usize {
        return Err(anyhow!(ContractError::TooMuchTickets {}));
    }
    raffle_info.tickets.push(owner);
    let ticket_amount = get_asset_amount(raffle_info.raffle_ticket_price.clone())?;
    let accumulated_amount = get_asset_amount(raffle_info.accumulated_ticket_fee)?;
    raffle_info.accumulated_ticket_fee = match raffle_info.raffle_ticket_price.clone() {
        AssetInfo::Cw20Coin(coin) => AssetInfo::Cw20Coin(Cw20Coin {
            address: coin.address,
            amount: accumulated_amount + ticket_amount,
        }),
        AssetInfo::Coin(coin) => AssetInfo::Coin(Coin {
            denom: coin.denom,
            amount: accumulated_amount + ticket_amount,
        }),
        _ => return Err(anyhow!(ContractError::WrongFundsType {})),
    };
    RAFFLE_INFO.save(deps.storage, raffle_id, &raffle_info)?;

    Ok(())
}

pub fn execute_update_randomness(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    raffle_id: u64,
    randomness: DrandRandomness,
) -> Result<Response> {
    let raffle_info = RAFFLE_INFO.load(deps.storage, raffle_id)?;
    let raffle_state = get_raffle_state(env, raffle_info);
    if raffle_state != RaffleState::Closed {
        return Err(anyhow!(ContractError::WrongStateForRandmness {
            status: raffle_state
        }));
    }

    // We assert the randomness is correct
    assert_randomness_origin_and_order(deps.as_ref(), info.sender, raffle_id, randomness)
}

pub fn execute_claim(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    raffle_id: u64,
) -> Result<Response> {
    // Loading the raffle object
    let mut raffle_info = RAFFLE_INFO.load(deps.storage, raffle_id)?;

    // We make sure the raffle is ended
    let raffle_state = get_raffle_state(env.clone(), raffle_info.clone());
    if raffle_state != RaffleState::Finished {
        return Err(anyhow!(ContractError::WrongStateForClaim {
            status: raffle_state
        }));
    }

    // If there was no participant, the winner is the raffle owner and we pay no fees whatsoever
    if raffle_info.tickets.is_empty() {
        raffle_info.winner = Some(raffle_info.owner.clone());
    } else {
        // We get the winner of the raffle and save it to the contract. The raffle is now claimed !
        let winner = get_raffle_winner(raffle_info.clone())?;
        raffle_info.winner = Some(winner);
    }
    RAFFLE_INFO.save(deps.storage, raffle_id, &raffle_info)?;

    // We send the asset to the winner
    let winner_transfer_message = get_raffle_winner_message(env.clone(), raffle_info.clone())?;
    let funds_transfer_messages =
        get_raffle_owner_finished_messages(deps.storage, env, raffle_info.clone())?;
    // We distribute the ticket prices to the owner and in part to the treasury
    Ok(Response::new()
        .add_message(winner_transfer_message)
        .add_messages(funds_transfer_messages)
        .add_attribute("action", "claim")
        .add_attribute("raffle_id", raffle_id.to_string())
        .add_attribute("winner", raffle_info.winner.unwrap()))
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
            raffle_info.randomness = Binary::from_base64(&randomness)?
                .as_slice()
                .try_into()
                .map_err(|_| StdError::generic_err("randomness parse err"))?;
            raffle_info.randomness_round = round;
            raffle_info.randomness_owner = Some(owner.clone());
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
    use base64;
    use cosmwasm_std::{
        coin, coins,
        testing::{mock_dependencies, mock_env, mock_info, MOCK_CONTRACT_ADDR},
        Api, BankMsg, Coin, SubMsg, SubMsgResponse,
    };
    use raffles_export::msg::VerifierExecuteMsg;
    const HEX_PUBKEY: &str = "868f005eb8e6e4ca0a47c8a77ceaa5309a47978a7c71bc5cce96366b5d7a569937c529eeda66c7293784a9402801af31";
    fn init_helper(deps: DepsMut) {
        let instantiate_msg = InstantiateMsg {
            name: "nft-raffle".to_string(),
            owner: None,
            random_pubkey: Binary::from_base64(&base64::encode(HEX_PUBKEY)).unwrap(),
            drand_url: None,
            verify_signature_contract: "verifier".to_string(),
            fee_addr: None,
            minimum_raffle_timeout: None,
            minimum_raffle_duration: None,
            raffle_fee: Some(Uint128::from(2u128)),
            rand_fee: None,
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
                asset: AssetInfo::Cw721Coin(Cw721Coin {
                    address: "nft".to_string(),
                    token_id: "token_id".to_string(),
                }),
                raffle_start_timestamp: None,
                raffle_duration: None,
                raffle_timeout: None,
                comment: None,
                raffle_ticket_price: AssetInfo::Coin(coin(10000u128, "uluna")),
                max_participant_number: None,
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
                asset: AssetInfo::Cw721Coin(Cw721Coin {
                    address: "nft".to_string(),
                    token_id: "token_id".to_string(),
                }),
                raffle_start_timestamp: None,
                raffle_duration: None,
                raffle_timeout: None,
                comment: None,
                raffle_ticket_price: AssetInfo::Cw20Coin(Cw20Coin {
                    address: "address".to_string(),
                    amount: Uint128::from(10000u128),
                }),
                max_participant_number: None,
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
                asset: AssetInfo::Cw1155Coin(Cw1155Coin {
                    address: "nft".to_string(),
                    token_id: "token_id".to_string(),
                    value: Uint128::from(675u128),
                }),
                raffle_start_timestamp: None,
                raffle_duration: None,
                raffle_timeout: None,
                comment: None,
                raffle_ticket_price: AssetInfo::Coin(coin(10000u128, "uluna")),
                max_participant_number: None,
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
                sent_assets: AssetInfo::Cw20Coin(Cw20Coin {
                    address: address.to_string(),
                    amount: Uint128::from(amount),
                }),
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
        raffle_info.randomness = randomness;
        raffle_info.randomness_round = 2098475u64;
        raffle_info.randomness_owner = Some(deps.api.addr_validate("rand_provider").unwrap());
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
        raffle_info.randomness = randomness;
        raffle_info.randomness_round = 2098475u64;
        raffle_info.randomness_owner = Some(deps.api.addr_validate("rand_provider").unwrap());
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
        raffle_info.randomness = randomness;
        raffle_info.randomness_round = 2098475u64;
        raffle_info.randomness_owner = Some(deps.api.addr_validate("rand_provider").unwrap());
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
                SubMsg::new(into_cosmos_msg(Cw20ExecuteMsg::Transfer {
                    recipient: "rand_provider".to_string(),
                    amount: Uint128::from(5u128)
                }, "address".to_string()).unwrap()),
                SubMsg::new(into_cosmos_msg(Cw20ExecuteMsg::Transfer {
                    recipient: "creator".to_string(),
                    amount: Uint128::from(10u128)
                }, "address".to_string()).unwrap()),
                SubMsg::new(into_cosmos_msg(Cw20ExecuteMsg::Transfer {
                    recipient: "creator".to_string(),
                    amount: Uint128::from(49985u128)
                }, "address".to_string()).unwrap()),
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
        raffle_info.randomness = randomness;
        raffle_info.randomness_round = 2098475u64;
        raffle_info.randomness_owner = Some(deps.api.addr_validate("rand_provider").unwrap());
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
            pubkey: Binary::from_base64(&base64::encode(HEX_PUBKEY)).unwrap(),
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
}
