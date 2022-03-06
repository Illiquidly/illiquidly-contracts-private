use cosmwasm_std::{to_binary, Binary, CosmosMsg, StdError, StdResult, WasmMsg};
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
    pub p2p_contract: String
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
    PayFeeAndWithdraw {
        trade_id: u64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Fee{
        trade_id: u64,
        counter_id: Option<u64>
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct FeeResponse{
    pub fee: String
}
