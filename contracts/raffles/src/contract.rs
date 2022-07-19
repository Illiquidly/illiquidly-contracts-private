#[cfg(not(feature = "library"))]
use anyhow::{Result, anyhow};
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, from_binary, Timestamp, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Uint128, Addr, coin
};

use cw2::set_contract_version;

use crate::error::ContractError;

use crate::state::{
    is_owner, load_raffle, CONTRACT_INFO, RAFFLE_INFO,
};
use raffles_export::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, into_cosmos_msg};
use raffles_export::state::{ContractInfo, RaffleInfo, RaffleState, Cw721Coin, Cw1155Coin, Cw20Coin,
MINIMUM_RAFFLE_DURATION,MINIMUM_RAFFLE_TIMEOUT,MINIMUM_RAND_FEE, MAXIMUM_PARTICIPANT_NUMBER, AssetInfo};

use crate::counter_trade::{
    add_asset_to_counter_trade, cancel_counter_trade, confirm_counter_trade, suggest_counter_trade,
    withdraw_all_from_counter, withdraw_counter_trade_assets_while_creating,
};

use cw1155::Cw1155ExecuteMsg;
use cw20::Cw20ExecuteMsg;
use cw721::Cw721ExecuteMsg;

use crate::trade::{
    accept_trade, add_asset_to_trade, add_nfts_wanted, add_whitelisted_users, cancel_trade,
    check_and_create_withdraw_messages, confirm_trade, create_trade, refuse_counter_trade,
    remove_nfts_wanted, remove_whitelisted_users, withdraw_all_from_trade,
    withdraw_trade_assets_while_creating,
};

use crate::messages::{review_counter_trade, set_comment};
use crate::query::{
    query_all_counter_trades, query_all_trades, query_contract_info, query_counter_trades,
};

const CONTRACT_NAME: &str = "illiquidly.io:p2p-trading";
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
        minimum_raffle_duration: msg.minimum_raffle_duration.unwrap_or(MINIMUM_RAFFLE_DURATION).max(MINIMUM_RAFFLE_DURATION),
        minimum_raffle_timeout: msg.minimum_raffle_timeout.unwrap_or(MINIMUM_RAFFLE_TIMEOUT).max(MINIMUM_RAFFLE_TIMEOUT),
        raffle_fee: msg.raffle_fee.unwrap_or(Uint128::zero()),
        rand_fee: msg.raffle_fee.unwrap_or(Uint128::from(MINIMUM_RAND_FEE)),
    };
    CONTRACT_INFO.save(deps.storage, &data)?;
    Ok(Response::default()
        .add_attribute("action", "init")
        .add_attribute("contract", "raffle")
        .add_attribute("owner", data.owner))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response> {
    match msg {

        ExecuteMsg::CreateRaffle {
            asset,
            raffle_start_timestamp, 
            raffle_duration,
            raffle_timeout,
            comment,
            raffle_ticket_price,
            max_participant_number
        } => execute_create_raffle(deps, env, info,  asset,
            raffle_start_timestamp, 
            raffle_duration,
            raffle_timeout,
            comment,
            raffle_ticket_price,
            max_participant_number),
        ExecuteMsg::BuyTicket {
            raffle_id,
            sent_assets
        } => execute_buy_ticket(deps, env, info, raffle_id, sent_assets),
        ExecuteMsg::Receive{
            sender,
            amount,
            msg,
        } => execute_receive(deps, env, info, sender, amount, msg),
        ExecuteMsg::ReceiveNft{
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
        } => execute_receive_1155(deps, env, info, from.unwrap_or(operator), token_id, amount, msg),
        ExecuteMsg::ClaimNft{
            raffle_id,
        } => execute_claim_nft(deps, env, info, raffle_id),
        ExecuteMsg::UpdateRandomness{
            raffle_id,
            randomness,
        } => execute_update_randomness(deps, env, info, raffle_id, randomness),

        // Admin messages
        ExecuteMsg::ToggleLock{
            lock,
        } => execute_toggle_lock(deps, env, info, lock),
        ExecuteMsg::Renounce{} => execute_renounce(deps, env, info),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // No state migrations performed, just returned a Response
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ContractInfo {} => to_binary(&query_contract_info(deps)?),
        QueryMsg::RaffleInfo { raffle_id } => to_binary(
            &load_raffle(deps.storage, trade_id)
                .map_err(|e| StdError::generic_err(e.to_string()))?,
        ),
        QueryMsg::TicketNumber {
            raffle_id,
            owner,
        } => query_ticket_number(deps, env, raffle_id, owner),
         QueryMsg::GetAllRaffles {
            start_after,
            limit,
            filters,
        } => query_al_raffles(deps, env, start_after, limit, filters),
         QueryMsg::GetTickets {
            raffle_id,
            start_after,
            limit,
            filters,
        } => query_tickets(deps, env, raffle_id, start_after, limit, filters),
         QueryMsg::GetAllTickets {
            start_after,
            limit,
            filters,
        } => query_all_tickets(deps, env, start_after, limit, filters),
        
    }
}

/// Replace the current contract owner with the provided owner address
/// * `owner` must be a valid Terra address
/// The owner has limited power on this contract :
/// 1. Change the contract owner
/// 2. Change the fee contract
pub fn execute_renounce(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut contract_info = is_owner(deps.storage, info.sender)?;

    contract_info.owner = env.contract.address.clone();
    CONTRACT_INFO.save(deps.storage, &contract_info)?;

    Ok(Response::new()
        .add_attribute("action", "modify_parameter")
        .add_attribute("parameter", "owner")
        .add_attribute("value", contract_info.owner))
}

/// Replace the current fee_contract with the provided fee_contract address
/// * `fee_contract` must be a valid Terra address
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

/// Create a new raffle by depositing assets
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
    max_participant_number: Option<u64>
) -> Result<Response> {

    // First we physcially transfer the AssetInfo
    let transfer_message = match asset{
        AssetInfo::Cw721Coin(token) => {
            let message = Cw721ExecuteMsg::TransferNft {
                recipient: env.contract.address.into(),
                token_id: token.token_id.clone(),
            };

           into_cosmos_msg(message, token.address.clone())?
        }
        AssetInfo::Cw1155Coin(token) => {
            let message = Cw1155ExecuteMsg::SendFrom {
                from: info.sender.to_string(),
                to: env.contract.address.into(),
                token_id: token.token_id.clone(),
                value: token.value,
                msg: None,
            };

            into_cosmos_msg(message, token.address.clone())?
        }
        _ => return Err(anyhow!(ContractError::WrongAssetType{}))
    };
    // Then we create the internal raffle structure
    let raffle_id = _create_raffle(
        deps,
        env,
        info.sender,
        asset,
        raffle_start_timestamp,
        raffle_duration,
        raffle_timeout,
        comment,
        raffle_ticket_price,
        max_participant_number
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


    match from_binary(&msg)? {
        ExecuteMsg::CreateRaffle {
            asset,
            raffle_start_timestamp, 
            raffle_duration,
            raffle_timeout,
            comment,
            raffle_ticket_price,
            max_participant_number
        } => {
            // First we make sure the received NFT is the one specified in the message

            match asset {
                AssetInfo::Cw721Coin(Cw721Coin{
                    address: address_received, token_id: token_id_received
                }) => {
                    if deps.api.addr_validate(&address_received)? == info.sender && token_id_received == token_id{
                        // The asset is a match, we can create the raffle object and return
                        let raffle_id = _create_raffle(
                            deps,
                            env,
                            deps.api.addr_validate(&sender)?,
                            asset,
                            raffle_start_timestamp,
                            raffle_duration,
                            raffle_timeout,
                            comment,
                            raffle_ticket_price,
                            max_participant_number
                        )?;

                        Ok(Response::new()
                            .add_attribute("action", "create_raffle")
                            .add_attribute("raffle_id", raffle_id.to_string())
                            .add_attribute("owner", info.sender))
                    }else{
                        Err(anyhow!(ContractError::AssetMismatch{}))
                    }
                }
                _ => Err(anyhow!(ContractError::AssetMismatch{}))
            }


        },
        _ => Err(anyhow!(ContractError::Unauthorized{}))
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

    match from_binary(&msg)? {
        ExecuteMsg::CreateRaffle {
            asset,
            raffle_start_timestamp, 
            raffle_duration,
            raffle_timeout,
            comment,
            raffle_ticket_price,
            max_participant_number
        } => {
            // First we make sure the received NFT is the one specified in the message
            match asset {
                AssetInfo::Cw1155Coin(Cw1155Coin{
                    address: address_received, token_id: token_id_received, value: value_received
                }) => {
                    if deps.api.addr_validate(&address_received)? == info.sender && token_id_received == token_id && value_received == amount{
                        // The asset is a match, we can create the raffle object and return
                        let raffle_id = _create_raffle(
                            deps,
                            env,
                            deps.api.addr_validate(&from)?,
                            asset,
                            raffle_start_timestamp,
                            raffle_duration,
                            raffle_timeout,
                            comment,
                            raffle_ticket_price,
                            max_participant_number
                        )?;

                        Ok(Response::new()
                            .add_attribute("action", "create_raffle")
                            .add_attribute("raffle_id", raffle_id.to_string())
                            .add_attribute("owner", info.sender))
                    }else{
                        Err(anyhow!(ContractError::AssetMismatch{}))
                    }
                }
                _ => Err(anyhow!(ContractError::AssetMismatch{}))
            }
        },
        _ => Err(anyhow!(ContractError::Unauthorized{}))
    }
}


/// Create a new raffle and assign it a unique id
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
    max_participant_number: Option<u64>
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
            raffle_start_timestamp: raffle_start_timestamp.map(|x| Timestamp::from_seconds(x)).unwrap_or(env.block.time),
            raffle_duration: raffle_duration.unwrap_or(contract_info.minimum_raffle_duration).max(contract_info.minimum_raffle_duration),
            raffle_timeout: raffle_timeout.unwrap_or(contract_info.minimum_raffle_timeout).max(contract_info.minimum_raffle_timeout),
            comment,
            raffle_ticket_price,
            tickets: vec![],
            current_randomness: Binary::default(),
            randomness_round: 0,
            max_participant_number: max_participant_number.unwrap_or(MAXIMUM_PARTICIPANT_NUMBER).min(MAXIMUM_PARTICIPANT_NUMBER),
        }),
    })?;
    Ok(raffle_id)
}


pub fn execute_buy_ticket(deps: DepsMut, env: Env, info: MessageInfo, raffle_id: u64,  assets: AssetInfo) -> Result<Response>{
    // First we physcially transfer the AssetInfo
    let transfer_messages = match assets{
        AssetInfo::Cw20Coin(token) => {
            let message = Cw20ExecuteMsg::Transfer {
                recipient: env.contract.address.into(),
                amount: token.amount
            };

           vec![into_cosmos_msg(message, token.address.clone())?]
        }
        AssetInfo::Coin(_) => {
            vec![]
        }
        _ => return Err(anyhow!(ContractError::WrongAssetType{}))
    };
    // Then we verify the funds sent match the raffle conditions and we save the ticket that was bought
    _buy_ticket(
        deps,
        env,
        info.sender,
        raffle_id,
        assets
    )?;

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
    msg:Binary
) -> Result<Response> {

    match from_binary(&msg)? {
        ExecuteMsg::BuyTicket {
            raffle_id,
            sent_assets
        } => {
            // First we make sure the received NFT is the one specified in the message
            match sent_assets {
                AssetInfo::Cw20Coin(Cw20Coin{
                    address: address_received, amount: amount_received
                }) => {
                    if deps.api.addr_validate(&address_received)? == info.sender && amount_received == amount{
                        // The asset is a match, we can create the raffle object and return
                        _buy_ticket(
                            deps,
                            env,
                            deps.api.addr_validate(&sender)?,
                            raffle_id,
                            sent_assets
                        )?;

                        Ok(Response::new()
                            .add_attribute("action", "create_raffle")
                            .add_attribute("raffle_id", raffle_id.to_string())
                            .add_attribute("owner", info.sender))
                    }else{
                        Err(anyhow!(ContractError::AssetMismatch{}))
                    }
                }
                _ => Err(anyhow!(ContractError::AssetMismatch{}))
            }
        },
        _ => Err(anyhow!(ContractError::Unauthorized{}))
    }
}


pub fn _buy_ticket(deps: DepsMut, env: Env, owner: Addr, raffle_id: u64, assets: AssetInfo) -> Result<()>{
    let mut raffle_info = RAFFLE_INFO.load(deps.storage, raffle_id)?;
    // We first check the sent assets match the raffle assets
    if raffle_info.raffle_ticket_price != assets{
        return Err(anyhow!(ContractError::PaiementNotSufficient{
            assets_wanted: raffle_info.raffle_ticket_price,
            assets_received: assets
        }))
    }
    // Then we save the sender to the bought tickets
    if raffle_info.tickets.len() >= raffle_info.max_participant_number as usize{
        return Err(anyhow!(ContractError::TooMuchTickets{}))
    }
    raffle_info.tickets.push(owner);
    RAFFLE_INFO.save(deps.storage, raffle_id, &raffle_info)?;

    Ok(())
}

/// Remove some assets from a trade when creating it.
pub fn withdraw_assets_while_creating(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    trade_id: u64,
    counter_id: Option<u64>,
    assets: Vec<(u16, AssetInfo)>, // We chose to number the withdrawn assets to prevent looping over all deposited assets
) -> Result<Response, ContractError> {
    match counter_id {
        Some(counter_id) => withdraw_counter_trade_assets_while_creating(
            deps, env, info, trade_id, counter_id, assets,
        ),
        None => withdraw_trade_assets_while_creating(deps, env, info, trade_id, assets),
    }
}

/// Withdraw assets from an accepted trade.
/// The trader will withdraw assets from the counter_trade
/// The counter_trader will withdraw assets from the trade
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

    // We load the corresponding counter_trade
    let counter_id = trade_info
        .accepted_info
        .clone()
        .ok_or(ContractError::ContractBug {})?
        .counter_id;
    let mut counter_info = load_counter_trade(deps.storage, trade_id, counter_id)?;

    let trader = deps.api.addr_validate(&trader)?;
    let (res, trade_type);

    // We indentify who the transaction sender is (trader or counter-trader)
    if trade_info.owner == trader {
        // In case the trader wants to withdraw the exchanged funds (from the counter_info object)
        res = check_and_create_withdraw_messages(env, &trader, &counter_info)?;

        trade_type = "counter";
        counter_info.assets_withdrawn = true;
        COUNTER_TRADE_INFO.save(deps.storage, (trade_id, counter_id), &counter_info)?;
    } else if counter_info.owner == trader {
        // In case the counter_trader wants to withdraw the exchanged funds (from the trade_info object)
        res = check_and_create_withdraw_messages(env, &trader, &trade_info)?;

        trade_type = "trade";
        trade_info.assets_withdrawn = true;
        TRADE_INFO.save(deps.storage, trade_id, &trade_info)?;
    } else {
        return Err(ContractError::NotWithdrawableByYou {});
    }

    Ok(res
        .add_attribute("action", "withdraw_funds")
        .add_attribute("type", trade_type)
        .add_attribute("trade", trade_id.to_string())
        .add_attribute("counter", counter_id.to_string())
        .add_attribute("trader", trade_info.owner)
        .add_attribute("counter_trader", counter_info.owner))
}
        