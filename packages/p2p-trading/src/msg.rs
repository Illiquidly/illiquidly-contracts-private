use crate::state::AssetInfo;
use cosmwasm_std::{to_binary, Binary, Coin, CosmosMsg, StdError, StdResult, Uint128, WasmMsg};
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
}

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive {
        sender: String,
        amount: Uint128,
        msg: Binary,
    },
    ReceiveNft {
        sender: String,
        token_id: String,
        msg: Binary,
    },
    CreateTrade {
        whitelisted_users: Option<Vec<String>>,
    },
    AddFundsToTrade {
        trade_id: u64,
        confirm: Option<bool>,
    },
    RemoveFromTrade {
        trade_id: u64,
        assets: Vec<(usize, AssetInfo)>,
        funds: Vec<(usize, Coin)>,
    },
    AddWhitelistedUsers{
        trade_id: u64,
        whitelisted_users: Vec<String>,
    },
    RemoveWhitelistedUsers{
        trade_id: u64,
        whitelisted_users: Vec<String>,
    },
    /// Is used by the Trader to confirm they completed their end of the trade.
    ConfirmTrade {
        trade_id: u64,
    },
    /// Can be used to initiate Counter Trade, but also to add new tokens to it
    SuggestCounterTrade {
        trade_id: u64,
        confirm: Option<bool>,
    },
    AddFundsToCounterTrade {
        trade_id: u64,
        counter_id: u64,
        confirm: Option<bool>,
    },
    RemoveFromCounterTrade {
        trade_id: u64,
        counter_id: u64,
        assets: Vec<(usize, AssetInfo)>,
        funds: Vec<(usize, Coin)>,
    },
    /// Is used by the Client to confirm they completed their end of the trade.
    ConfirmCounterTrade {
        trade_id: u64,
        counter_id: u64,
    },
    /// Accept the Trade plain and simple, swap it up !
    AcceptTrade {
        trade_id: u64,
        counter_id: u64,
    },
    /// Accept the Trade plain and simple, swap it up !
    CancelTrade {
        trade_id: u64,
    },
    /// Refuse the Trade plain and simple, no madam, I'm not interested in your tokens !
    RefuseCounterTrade {
        trade_id: u64,
        counter_id: u64,
    },
    /// Some parts of the traded tokens were interesting, but you can't accept the trade as is
    ReviewCounterTrade {
        trade_id: u64,
        counter_id: u64,
        comment: Option<String>,
    },
    /// You can Withdraw funds only at specific steps of the trade, but you're allowed to try anytime !
    WithdrawPendingAssets {
        trade_id: u64,
    },
    /// You can Withdraw funds only at specific steps of the trade, but you're allowed to try anytime !
    WithdrawCancelledTrade {
        trade_id: u64,
    },
    /// You can Withdraw funds when your counter trade is aborted (refused or cancelled)
    WithdrawAbortedCounter {
        trade_id: u64,
        counter_id: u64,
    },
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveMsg {
    AddToTrade { trade_id: u64 },
    AddToCounterTrade { trade_id: u64, counter_id: u64 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    ContractInfo {},
    TradeInfo { trade_id: u64 },
    CounterTradeInfo { trade_id: u64, counter_id: u64 }, /*
                                                         }
                                                         GetAllActiveTrades{}

                                                         GetCounterTrades{
                                                             trade_id:u64,
                                                         },
                                                         GetAllActiveCounterTrades{}


                                                         */
}
