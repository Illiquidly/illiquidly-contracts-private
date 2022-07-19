use crate::state::{AssetInfo};
use cosmwasm_std::{to_binary, Binary, CosmosMsg, StdError, StdResult, WasmMsg, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn is_valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() < 3 || bytes.len() > 50 {
        return false;
    }
    true
}

pub fn into_binary<M: Serialize>(msg: M) -> StdResult<Binary> {
    to_binary(&msg)
}

pub fn into_cosmos_msg<M: Serialize, T: Into<String>>(
    message: M,
    contract_addr: T,
) -> StdResult<CosmosMsg> {
    let msg = into_binary(message)?;
    let execute = WasmMsg::Execute {
        contract_addr: contract_addr.into(),
        msg,
        funds: vec![],
    };
    Ok(execute.into())
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMsg {
    pub name: String,
    pub owner: Option<String>,
    pub fee_addr: Option<String>,
    pub last_raffle_id: Option<u64>,
    pub minimum_raffle_duration: Option<u64>,
    pub minimum_raffle_timeout: Option<u64>,
    pub raffle_fee: Option<Uint128>, // in 10_000
    pub rand_fee: Option<Uint128> // in 10_000
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct MigrateMsg {}

impl InstantiateMsg {
    pub fn validate(&self) -> StdResult<()> {
        // Check name, symbol, decimals
        if !is_valid_name(&self.name) {
            return Err(StdError::generic_err(
                "Name is not in the expected format (3-50 UTF-8 bytes)",
            ));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct DrandRandomness{
    pub round: u64,
    pub previous_signature: Binary,
    pub signature: Binary,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    CreateRaffle {
        asset: AssetInfo,
        raffle_start_timestamp: Option<u64>, // If not specified, starts immediately
        raffle_duration: Option<u64>,
        raffle_timeout: Option<u64>,
        comment: Option<String>,
        raffle_ticket_price: AssetInfo,
        max_participant_number: Option<u64>
    },
    BuyTicket {
        raffle_id: u64,
        sent_assets: AssetInfo
    },
    Receive{
        sender: String,
        amount: Uint128,
        msg: Binary
    },
    ReceiveNft{
        sender: String,
        token_id: String,
        msg: Binary
    },
    Cw1155ReceiveMsg {
        operator: String,
        from: Option<String>,
        token_id: String,
        amount: Uint128,
        msg: Binary
    },
    ClaimNft{
        raffle_id: u64
    },
    UpdateRandomness{
        raffle_id: u64,
        randomness: DrandRandomness
    },

    // Admin messages
    ToggleLock{
        lock: bool
    }, 
    Renounce{},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub struct QueryFilters {
    pub states: Option<Vec<String>>,
    pub owner: Option<String>,
    pub counterer: Option<String>,
    pub has_whitelist: Option<bool>,
    pub whitelisted_user: Option<String>,
    pub contains_token: Option<String>,
    pub wanted_nft: Option<String>,
    pub assets_withdrawn: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    ContractInfo {},
    RaffleInfo {
        raffle_id: u64,
    },
    TicketNumber {
        raffle_id: u64,
        owner: String,
    },
    GetAllRaffles {
        start_after: Option<u64>,
        limit: Option<u32>,
        filters: Option<QueryFilters>,
    },
    GetTickets {
        trade_id: u64,
        start_after: Option<u64>,
        limit: Option<u32>,
        filters: Option<QueryFilters>,
    },
    GetAllTickets {
        start_after: Option<u64>,
        limit: Option<u32>,
        filters: Option<QueryFilters>,
    },
}
