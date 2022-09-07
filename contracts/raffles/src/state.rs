use anyhow::{anyhow, Result};
use cw_storage_plus::{Item, Map};

use cosmwasm_std::{
    coins, Addr, BankMsg, CosmosMsg, Deps, Env, Response, StdError, Storage, SubMsg, Uint128,
};

use crate::error::ContractError;
use crate::rand::Prng;
use raffles_export::msg::{into_cosmos_msg, DrandRandomness, VerifierExecuteMsg};
use raffles_export::state::{AssetInfo, ContractInfo, RaffleInfo, RaffleState};

use cw1155::Cw1155ExecuteMsg;
use cw20::Cw20ExecuteMsg;
use cw721::Cw721ExecuteMsg;

pub const CONTRACT_INFO: Item<ContractInfo> = Item::new("contract_info");
pub const RAFFLE_INFO: Map<u64, RaffleInfo> = Map::new("raffle_info");
pub const RAFFLE_TICKETS: Map<(u64, u32), Addr> = Map::new("raffle_tickets");
pub const USER_TICKETS: Map<(&Addr, u64), u32> = Map::new("user_tickets");

/// This function is largely inspired (and even directly copied) from https://github.com/LoTerra/terrand-contract-step1/
/// This function actually simply calls an external contract that checks the randomness origin
/// This architecture was chosen because the imported libraries needed to verify signatures and very heavy and won't upload.
/// Separating into 2 contracts seems to help with that
/// For more info about randomness, visit : https://drand.love/
pub fn assert_randomness_origin_and_order(
    deps: Deps,
    owner: Addr,
    raffle_id: u64,
    randomness: DrandRandomness,
) -> Result<Response> {
    let raffle_info = load_raffle(deps.storage, raffle_id)?;
    let contract_info = CONTRACT_INFO.load(deps.storage)?;

    if let Some(local_randomness) = raffle_info.randomness {
        if randomness.round <= local_randomness.randomness_round {
            return Err(anyhow!(ContractError::RandomnessNotAccepted {
                current_round: local_randomness.randomness_round
            }));
        }
    }

    let msg = VerifierExecuteMsg::Verify {
        randomness,
        pubkey: contract_info.random_pubkey,
        raffle_id,
        owner: owner.to_string(),
    };
    let res = into_cosmos_msg(msg, contract_info.verify_signature_contract.to_string())?;

    let msg = SubMsg::reply_on_success(res, 0);
    Ok(Response::new().add_submessage(msg))
}

pub fn is_owner(storage: &dyn Storage, sender: Addr) -> Result<ContractInfo, ContractError> {
    let contract_info = CONTRACT_INFO.load(storage)?;
    if sender == contract_info.owner {
        Ok(contract_info)
    } else {
        Err(ContractError::Unauthorized {})
    }
}

/// Picking the winner of the raffle was inspried by https://github.com/scrtlabs/secret-raffle/
/// We know the odds are not exactly perfect with this architecture
/// --> that's not how you select a true random number from an interval, but 1/4_294_967_295 is quite small anyway
pub fn get_raffle_winner(
    deps: Deps,
    env: Env,
    raffle_id: u64,
    raffle_info: RaffleInfo,
) -> Result<Addr> {
    // We initiate the random number generator
    if raffle_info.randomness.is_none() {
        return Err(anyhow!(ContractError::WrongStateForClaim {
            status: get_raffle_state(env, raffle_info)
        }));
    }
    let mut rng: Prng = Prng::new(&raffle_info.randomness.unwrap().randomness);

    // We pick a winner id
    let winner_id = rng.random_between(0u32, raffle_info.number_of_tickets);
    let winner = RAFFLE_TICKETS.load(deps.storage, (raffle_id, winner_id))?;

    Ok(winner)
}

/// Queries the raffle state
/// This function depends on the block time to return the RaffleState.
/// As actions can only happen in certain time-periods, you have to be careful when testing off-chain
/// If the chains stops or the block time is not accurate we might get some errors (let's hope it never happens)
pub fn get_raffle_state(env: Env, raffle_info: RaffleInfo) -> RaffleState {
    if env.block.time < raffle_info.raffle_options.raffle_start_timestamp {
        RaffleState::Created
    } else if env.block.time
        < raffle_info
            .raffle_options
            .raffle_start_timestamp
            .plus_seconds(raffle_info.raffle_options.raffle_duration)
    {
        RaffleState::Started
    } else if env.block.time
        < raffle_info
            .raffle_options
            .raffle_start_timestamp
            .plus_seconds(raffle_info.raffle_options.raffle_duration)
            .plus_seconds(raffle_info.raffle_options.raffle_timeout)
        || raffle_info.randomness.is_none()
    {
        RaffleState::Closed
    } else if raffle_info.winner.is_none() {
        RaffleState::Finished
    } else {
        RaffleState::Claimed
    }
}

pub fn load_raffle(storage: &dyn Storage, raffle_id: u64) -> Result<RaffleInfo> {
    RAFFLE_INFO
        .load(storage, raffle_id)
        .map_err(|_| anyhow!(ContractError::NotFoundInRaffleInfo {}))
}

/// Can only buy a ticket when the raffle has started and is not closed
pub fn can_buy_ticket(env: Env, raffle_info: RaffleInfo) -> Result<()> {
    if get_raffle_state(env, raffle_info) == RaffleState::Started {
        Ok(())
    } else {
        Err(anyhow!(ContractError::CantBuyTickets {}))
    }
}

/// Util to get the winner messages to return when claiming a Raffle (returns the raffled asset)
pub fn get_raffle_winner_message(env: Env, raffle_info: RaffleInfo) -> Result<CosmosMsg> {
    match raffle_info.asset {
        AssetInfo::Cw721Coin(nft) => {
            let message = Cw721ExecuteMsg::TransferNft {
                recipient: raffle_info.winner.unwrap().to_string(),
                token_id: nft.token_id.clone(),
            };
            into_cosmos_msg(message, nft.address)
        }
        AssetInfo::Cw1155Coin(cw1155) => {
            let message = Cw1155ExecuteMsg::SendFrom {
                from: env.contract.address.to_string(),
                to: raffle_info.winner.unwrap().to_string(),
                token_id: cw1155.token_id.clone(),
                value: cw1155.value,
                msg: None,
            };
            into_cosmos_msg(message, cw1155.address)
        }
        _ => Err(anyhow!(StdError::generic_err(
            "Unreachable, wrong asset type raffled"
        ))),
    }
}

/// Util to get the organizers and helpers messages to return when claiming a Raffle (returns the funds)
pub fn get_raffle_owner_finished_messages(
    storage: &dyn Storage,
    _env: Env,
    raffle_info: RaffleInfo,
) -> Result<Vec<CosmosMsg>> {
    let contract_info = CONTRACT_INFO.load(storage)?;
    match raffle_info.raffle_ticket_price {
        AssetInfo::Cw20Coin(coin) => {
            // We start by splitting the fees between owner, treasury and radomness provider
            let total_paid = coin.amount * Uint128::from(u128::from(raffle_info.number_of_tickets));
            let rand_amount = total_paid * contract_info.rand_fee / Uint128::from(10_000u128);
            let treasury_amount = total_paid * contract_info.raffle_fee / Uint128::from(10_000u128);
            let owner_amount = total_paid - rand_amount - treasury_amount;

            let mut messages: Vec<CosmosMsg> = vec![];
            if rand_amount != Uint128::zero() {
                messages.push(into_cosmos_msg(
                    Cw20ExecuteMsg::Transfer {
                        recipient: raffle_info.randomness.unwrap().randomness_owner.to_string(),
                        amount: rand_amount,
                    },
                    coin.address.clone(),
                )?);
            };
            if treasury_amount != Uint128::zero() {
                messages.push(into_cosmos_msg(
                    Cw20ExecuteMsg::Transfer {
                        recipient: contract_info.fee_addr.to_string(),
                        amount: treasury_amount,
                    },
                    coin.address.clone(),
                )?);
            };
            if owner_amount != Uint128::zero() {
                messages.push(into_cosmos_msg(
                    Cw20ExecuteMsg::Transfer {
                        recipient: raffle_info.owner.to_string(),
                        amount: owner_amount,
                    },
                    coin.address,
                )?);
            };
            Ok(messages)
        }
        AssetInfo::Coin(coin) => {
            // We start by splitting the fees between owner, treasury and radomness provider
            let total_paid = coin.amount * Uint128::from(u128::from(raffle_info.number_of_tickets));
            let rand_amount = total_paid * contract_info.rand_fee / Uint128::from(10_000u128);
            let treasury_amount = total_paid * contract_info.raffle_fee / Uint128::from(10_000u128);
            let owner_amount = total_paid - rand_amount - treasury_amount;

            let mut messages: Vec<CosmosMsg> = vec![];
            if rand_amount != Uint128::zero() {
                messages.push(
                    BankMsg::Send {
                        to_address: raffle_info.randomness.unwrap().randomness_owner.to_string(),
                        amount: coins(rand_amount.u128(), coin.denom.clone()),
                    }
                    .into(),
                );
            };
            if treasury_amount != Uint128::zero() {
                messages.push(
                    BankMsg::Send {
                        to_address: contract_info.fee_addr.to_string(),
                        amount: coins(treasury_amount.u128(), coin.denom.clone()),
                    }
                    .into(),
                );
            };
            if owner_amount != Uint128::zero() {
                messages.push(
                    BankMsg::Send {
                        to_address: raffle_info.owner.to_string(),
                        amount: coins(owner_amount.u128(), coin.denom),
                    }
                    .into(),
                );
            };

            Ok(messages)
        }
        _ => Err(anyhow!(ContractError::WrongFundsType {})),
    }
}
