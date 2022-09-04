use anyhow::{anyhow, Result};
#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    from_binary, Addr, Binary, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};

use crate::state::{
    assert_randomness_origin_and_order, can_buy_ticket, get_asset_amount,
    get_raffle_owner_finished_messages, get_raffle_state, get_raffle_winner,
    get_raffle_winner_message, CONTRACT_INFO, RAFFLE_INFO, RAFFLE_TICKETS, USER_TICKETS,
};

use crate::error::ContractError;
use raffles_export::state::{AssetInfo, Cw1155Coin, Cw20Coin, Cw721Coin, RaffleInfo, RaffleState, RaffleOptionsMsg, RaffleOptions};

use cw1155::Cw1155ExecuteMsg;
use cw20::Cw20ExecuteMsg;
use cw721::Cw721ExecuteMsg;
use raffles_export::msg::{into_cosmos_msg, DrandRandomness, ExecuteMsg};


/// Create a new raffle by depositing assets.
/// The raffle has many options, to make it most accessible : 
/// Args : 
/// owner: The address that will receive the funds when the raffle is ended. Default value : create raffle transaction sender
/// asset : The asset set up for auction. It can be a CW721 standard asset or a CW1155 standard asset.
/// This asset will be deposited with this function. Don't forget to pre-approve the contract for this asset to be able to create a raffle 
/// ReceiveNFT or Receive_CW1155 is used for people that hate approvals
/// raffle_start_timestamp : Block Timestamp from which the users can buy tickets Default : current block time
/// raffle_duration : time in seconds from the raffle_start_timestamp during which users can buy tickets. Default : contract.minimum_raffle_duration
/// raffle_timeout : time in seconds from the end of the raffle duration during which users can add randomness. Default : contract.minimum_raffle_timeout
/// comment: A simple comment to add to the raffle (because we're not machines) : Default : ""
/// raffle_ticket_price : The needed tokens (native or CW20) needed to buy a raffle ticket
/// If you want to have free tickets, specify a 0 amount on a native token (any denom)
/// max_participant_number: maximum number of participants to the raffle. Default : contract_info.max_participant_number
#[allow(clippy::too_many_arguments)]
pub fn execute_create_raffle(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: Option<String>,
    asset: AssetInfo,
    raffle_ticket_price: AssetInfo,
    raffle_options: RaffleOptionsMsg
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
    let owner = owner.map(|x|deps.api.addr_validate(&x)).transpose()?;
    let raffle_id = _create_raffle(
        deps,
        env,
        owner.clone().unwrap_or_else(|| info.sender.clone()),
        asset,
        raffle_ticket_price,
        raffle_options
    )?;

    Ok(Response::new()
        .add_message(transfer_message)
        .add_attribute("action", "create_raffle")
        .add_attribute("raffle_id", raffle_id.to_string())
        .add_attribute("owner", owner.unwrap_or_else(|| info.sender.clone()),))
}

/// Create a new raffle by depositing assets.
/// This function is used when sending an asset to the contract directly using a send_msg
/// This is used to create CW721 raffles
/// This function checks the sent message matches the sent assets and creates a raffle internally
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
            owner,
            asset,
            raffle_ticket_price,
            raffle_options
        } => {
            // First we make sure the received NFT is the one specified in the message
            match asset.clone() {
                AssetInfo::Cw721Coin(Cw721Coin {
                    address: address_received,
                    token_id: token_id_received,
                }) => {
                    if deps.api.addr_validate(&address_received)? != info.sender
                        || token_id_received != token_id
                    {
                        return  Err(anyhow!(ContractError::AssetMismatch {}))
                    }
                    // The asset is a match, we can create the raffle object and return
                    let owner = owner.map(|x| deps.api.addr_validate(&x)).transpose()?;
                    let raffle_id = _create_raffle(
                        deps,
                        env,
                        owner.clone().unwrap_or_else(||sender.clone()),
                        asset,
                        raffle_ticket_price,
                        raffle_options
                    )?;

                    Ok(Response::new()
                        .add_attribute("action", "create_raffle")
                        .add_attribute("raffle_id", raffle_id.to_string())
                        .add_attribute("owner", owner.unwrap_or(sender))
                        )
                }
                _ => Err(anyhow!(ContractError::AssetMismatch {})),
            }
        }
        _ => Err(anyhow!(ContractError::Unauthorized {})),
    }
}

/// Create a new raffle by depositing assets
/// This function is used when sending an asset to the contract directly using a send_msg
/// This is used to create CW1155 raffles
/// This function checks the sent message matches the sent assets and creates a raffle internally
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
            owner,
            asset,
            raffle_ticket_price,
            raffle_options
        } => {
            // First we make sure the received NFT is the one specified in the message
            match asset.clone() {
                AssetInfo::Cw1155Coin(Cw1155Coin {
                    address: address_received,
                    token_id: token_id_received,
                    value: value_received,
                }) => {
                    if deps.api.addr_validate(&address_received)? != info.sender
                        || token_id_received != token_id
                        || value_received != amount
                    {
                        return Err(anyhow!(ContractError::AssetMismatch {}))
                    }
                    // The asset is a match, we can create the raffle object and return
                    let owner = owner.map(|x| deps.api.addr_validate(&x)).transpose()?;
                    let raffle_id = _create_raffle(
                        deps,
                        env,
                        owner.clone().unwrap_or_else(||sender.clone()),
                        asset,
                        raffle_ticket_price,
                        raffle_options
                    )?;

                    Ok(Response::new()
                        .add_attribute("action", "create_raffle")
                        .add_attribute("raffle_id", raffle_id.to_string())
                        .add_attribute("owner", owner.unwrap_or(sender)))
                }
                _ => Err(anyhow!(ContractError::AssetMismatch {})),
            }
        }
        _ => Err(anyhow!(ContractError::Unauthorized {})),
    }
}

/// Create a new raffle and assign it a unique id
/// Internal function that doesn't check anything and creates a raffle.
/// The arguments are described on the create_raffle function above.
#[allow(clippy::too_many_arguments)]
pub fn _create_raffle(
    deps: DepsMut,
    env: Env,
    owner: Addr,
    asset: AssetInfo,
    raffle_ticket_price: AssetInfo,
    raffle_options: RaffleOptionsMsg
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
            raffle_ticket_price: raffle_ticket_price.clone(),
            accumulated_ticket_fee: match raffle_ticket_price {
                AssetInfo::Cw20Coin(coin) => AssetInfo::cw20(0u128, &coin.address),
                AssetInfo::Coin(coin) => AssetInfo::coin(0u128, &coin.denom),
                _ => return Err(ContractError::WrongFundsType {}),
            },
            number_of_tickets: 0u32,
            randomness: None,
            winner: None,
            raffle_options: RaffleOptions::new(env, raffle_options, contract_info)
        }),
    })?;
    Ok(raffle_id)
}

/// Buy a ticket for a specific raffle
/// Argument description :
/// raffle_id: The id of the raffle you want to buy a ticket to/
/// assets : the assets you want to deposit against a raffle ticket. 
/// These assets can either be a native coin or a CW20 token
/// These must correspond to the raffle_info.raffle_ticket_price exactly
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

            if coin.amount != Uint128::zero() && info.funds.len() != 1 {
                return Err(anyhow!(ContractError::AssetMismatch {}));
            }
            if coin.amount != Uint128::zero() && info.funds[0].denom != coin.denom || info.funds[0].amount != coin.amount {
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

/// Buy a ticket for a specific raffle
/// This function is used when sending an asset to the contract directly using a send_msg
/// This is used to buy a ticket using CW20 tokens
/// This function checks the sent message matches the sent assets and buys a ticket internally
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
            // First we make sure the received Asset is the one specified in the message
            match sent_assets.clone() {
                AssetInfo::Cw20Coin(Cw20Coin {
                    address: address_received,
                    amount: amount_received,
                }) => {
                    if deps.api.addr_validate(&address_received)? == info.sender
                        && amount_received == amount
                    {
                        // The asset is a match, we can create the raffle object and return
                        _buy_ticket(deps, env, sender.clone(), raffle_id, sent_assets)?;

                        Ok(Response::new()
                            .add_attribute("action", "buy_ticket")
                            .add_attribute("raffle_id", raffle_id.to_string())
                            .add_attribute("owner", sender))
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

/// Create a new raffle tickets and assigns it to the sender
/// Internal function that doesn't check anything and buy a ticket
/// The arguments are described on the create_raffle function above.
pub fn _buy_ticket(
    deps: DepsMut,
    env: Env,
    owner: Addr,
    raffle_id: u64,
    assets: AssetInfo,
) -> Result<()> {
    let mut raffle_info = RAFFLE_INFO.load(deps.storage, raffle_id)?;

    // We first check the sent assets match the raffle assets
    if raffle_info.raffle_ticket_price != assets {
        return Err(anyhow!(ContractError::PaiementNotSufficient {
            assets_wanted: raffle_info.raffle_ticket_price,
            assets_received: assets
        }));
    }

    // We then check the raffle is in the right state
    can_buy_ticket(env, raffle_info.clone())?;

    // Then we check the user has the right to buy one more ticket
    if let Some(max_ticket_per_address) = raffle_info.raffle_options.max_ticket_per_address{
        let current_ticket_number = USER_TICKETS.load(deps.storage, (&owner, raffle_id)).unwrap_or(0);
        if current_ticket_number >= max_ticket_per_address{
            return Err(anyhow!(ContractError::TooMuchTickets {}));
        }
    }

    // Then we check there are some ticket left to buy
     if let Some(max_participant_number) = raffle_info.raffle_options.max_participant_number{
        if raffle_info.number_of_tickets >= max_participant_number{
            return Err(anyhow!(ContractError::TooMuchTickets {}));
        }
    };

    // Then we save the sender to the bought tickets
    RAFFLE_TICKETS.save(
        deps.storage,
        (raffle_id, raffle_info.number_of_tickets),
        &owner,
    )?;
    USER_TICKETS.update::<_, anyhow::Error>(deps.storage, (&owner, raffle_id), |x| match x {
        Some(ticket_number) => Ok(ticket_number + 1),
        None => Ok(1),
    })?;
    raffle_info.number_of_tickets += 1;

    let ticket_amount = get_asset_amount(raffle_info.raffle_ticket_price.clone())?;
    let accumulated_amount = get_asset_amount(raffle_info.accumulated_ticket_fee)?;
    raffle_info.accumulated_ticket_fee = match raffle_info.raffle_ticket_price.clone() {
        AssetInfo::Cw20Coin(coin) => {
            AssetInfo::cw20_raw(accumulated_amount + ticket_amount, &coin.address)
        }
        AssetInfo::Coin(coin) => {
            AssetInfo::coin_raw(accumulated_amount + ticket_amount, &coin.denom)
        }
        _ => return Err(anyhow!(ContractError::WrongFundsType {})),
    };
    RAFFLE_INFO.save(deps.storage, raffle_id, &raffle_info)?;

    Ok(())
}


/// Update the randomness assigned to a raffle
/// The function receives and checks the randomness against the drand public_key registered with the account.
/// This allows trustless and un-predictable randomness to the raffle contract.
/// The randomness providers will get a small cut of the raffle tickets (to reimburse the tx fees and incentivize adding randomness)
pub fn execute_update_randomness(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    raffle_id: u64,
    randomness: DrandRandomness,
) -> Result<Response> {
    // We check the raffle can receive randomness (good state)
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

/// Claim and end a raffle
/// This function can be called by anyone
/// This function has 4 purposes : 
/// 1. Compute the winner of a raffle and save it in the contract
/// 2. Send the Asset to the winner
/// 3. Send the accumulated tiket prices to the raffle owner
/// 4. Send the fees (a cut of the accumulated ticket prices) to the treasury and the randomness provider
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
    if raffle_info.number_of_tickets == 0u32 {
        raffle_info.winner = Some(raffle_info.owner.clone());
    } else {
        // We get the winner of the raffle and save it to the contract. The raffle is now claimed !
        let winner = get_raffle_winner(deps.as_ref(), env.clone(), raffle_id, raffle_info.clone())?;
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
