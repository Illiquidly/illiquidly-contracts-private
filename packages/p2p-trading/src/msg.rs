use cosmwasm_std::{to_binary, Binary, CosmosMsg, StdError, StdResult, Uint128, WasmMsg};
use cw20::Cw20Coin;
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Cw721Coin {
    pub address: String,
    pub token_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TokenToSend {
    Cw20Coin(Cw20Coin),
    Cw721Coin(Cw721Coin),
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
    CreateTrade {},
    AddFundsToTrade {
        trade_id: u64,
        confirm: Option<bool>,
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
    WithdrawPendingFunds {
        trade_id: u64,
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
    /// Returns the current balance of the given address, 0 if unset.
    /// Return type: BalanceResponse.
    Balance { address: String },
    /// Returns metadata on the contract - name, decimals, supply, etc.
    /// Return type: TokenInfoResponse.
    TokenInfo {},
    /// Only with "mintable" extension.
    /// Returns who can mint and the hard cap on maximum tokens after minting.
    /// Return type: MinterResponse.
    Minter {},
    /// Only with "allowance" extension.
    /// Returns how much spender can use from owner account, 0 if unset.
    /// Return type: AllowanceResponse.
    Allowance { owner: String, spender: String },
    /// Only with "enumerable" extension (and "allowances")
    /// Returns all allowances this owner has approved. Supports pagination.
    /// Return type: AllAllowancesResponse.
    AllAllowances {
        owner: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Only with "enumerable" extension
    /// Returns all accounts that have balances. Supports pagination.
    /// Return type: AllAccountsResponse.
    AllAccounts {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Only with "marketing" extension
    /// Returns more metadata on the contract to display in the client:
    /// - description, logo, project url, etc.
    /// Return type: MarketingInfoResponse
    MarketingInfo {},
    /// Only with "marketing" extension
    /// Downloads the mbeded logo data (if stored on chain). Errors if no logo data ftored for this
    /// contract.
    /// Return type: DownloadLogoResponse.
    DownloadLogo {},


    /*
    GetAllActiveTrades{}
    GetTradeInfo{
        trade_id: u64,
    }
    GetCounterTrades{
        trade_id:u64,
    },
    GetCounterTradeInfo{
        trade_id: u64,
        counter_id: u64
    }
    GetAllActiveCounterTrades{}


    */
}