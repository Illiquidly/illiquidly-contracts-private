#[cfg(test)]
pub mod tests {
    use cosmwasm_std::testing::{
        MockApi, MockQuerierCustomHandlerResult, MockStorage, MOCK_CONTRACT_ADDR,
    };

    use cosmwasm_std::{
        from_binary, from_slice, to_binary, AllBalanceResponse, BalanceResponse, BankQuery, Binary,
        Coin, ContractResult, CustomQuery, Empty, OwnedDeps, Querier, QuerierResult, QueryRequest,
        SystemError, SystemResult, WasmQuery,
    };

    use std::marker::PhantomData;
    use serde::de::DeserializeOwned;
    use std::collections::HashMap;

    use cw_4626::query::QueryMsg as Cw4626QueryMsg;
    use cw_4626::state::AssetInfo;

    // All external requirements that can be injected for unit tests.
    /// It sets the given balance for the contract itself, nothing else
    pub fn mock_dependencies(
        contract_balance: &[Coin],
    ) -> OwnedDeps<MockStorage, MockApi, MockQuerier> {
        OwnedDeps {
            storage: MockStorage::default(),
            api: MockApi::default(),
            querier: MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]),
            custom_query_type: PhantomData,
        }
    }

    /// MockQuerier holds an immutable table of bank balances
    pub struct MockQuerier<C: DeserializeOwned = Empty> {
        bank: BankQuerier,
        #[cfg(feature = "staking")]
        staking: StakingQuerier,
        // placeholder to add support later
        wasm: CustomWasmQuerier,
        /// A handler to handle custom queries. This is set to a dummy handler that
        /// always errors by default. Update it via `with_custom_handler`.
        ///
        /// Use box to avoid the need of another generic type
        custom_handler: Box<dyn for<'a> Fn(&'a C) -> MockQuerierCustomHandlerResult>,
    }

    impl<C: DeserializeOwned> MockQuerier<C> {
        pub fn new(balances: &[(&str, &[Coin])]) -> Self {
            MockQuerier {
                bank: BankQuerier::new(balances),
                #[cfg(feature = "staking")]
                staking: StakingQuerier::default(),
                wasm: CustomWasmQuerier {},
                // strange argument notation suggested as a workaround here: https://github.com/rust-lang/rust/issues/41078#issuecomment-294296365
                custom_handler: Box::from(|_: &_| -> MockQuerierCustomHandlerResult {
                    SystemResult::Err(SystemError::UnsupportedRequest {
                        kind: "custom".to_string(),
                    })
                }),
            }
        }

        // set a new balance for the given address and return the old balance
        pub fn update_balance(
            &mut self,
            addr: impl Into<String>,
            balance: Vec<Coin>,
        ) -> Option<Vec<Coin>> {
            self.bank.balances.insert(addr.into(), balance)
        }

        #[cfg(feature = "staking")]
        pub fn update_staking(
            &mut self,
            denom: &str,
            validators: &[crate::query::Validator],
            delegations: &[crate::query::FullDelegation],
        ) {
            self.staking = StakingQuerier::new(denom, validators, delegations);
        }

        pub fn with_custom_handler<CH: 'static>(mut self, handler: CH) -> Self
        where
            CH: Fn(&C) -> MockQuerierCustomHandlerResult,
        {
            self.custom_handler = Box::from(handler);
            self
        }
    }

    impl<C: CustomQuery + DeserializeOwned> Querier for MockQuerier<C> {
        fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
            let request: QueryRequest<C> = match from_slice(bin_request) {
                Ok(v) => v,
                Err(e) => {
                    return SystemResult::Err(SystemError::InvalidRequest {
                        error: format!("Parsing query request: {}", e),
                        request: bin_request.into(),
                    })
                }
            };
            self.handle_query(&request)
        }
    }

    impl<C: CustomQuery + DeserializeOwned> MockQuerier<C> {
        pub fn handle_query(&self, request: &QueryRequest<C>) -> QuerierResult {
            match &request {
                QueryRequest::Bank(bank_query) => self.bank.query(bank_query),
                QueryRequest::Custom(custom_query) => (*self.custom_handler)(custom_query),
                #[cfg(feature = "staking")]
                QueryRequest::Staking(staking_query) => self.staking.query(staking_query),
                QueryRequest::Wasm(msg) => self.wasm.query(msg),
                #[cfg(feature = "stargate")]
                QueryRequest::Stargate { .. } => SystemResult::Err(SystemError::UnsupportedRequest {
                    kind: "Stargate".to_string(),
                }),
                #[cfg(feature = "stargate")]
                QueryRequest::Ibc(_) => SystemResult::Err(SystemError::UnsupportedRequest {
                    kind: "Ibc".to_string(),
                }),
                _ => SystemResult::Err(SystemError::UnsupportedRequest {
                    kind: "custom".to_string(),
                }),
            }
        }
    }

    #[derive(Clone, Default)]
    pub struct BankQuerier {
        balances: HashMap<String, Vec<Coin>>,
    }

    impl BankQuerier {
        pub fn new(balances: &[(&str, &[Coin])]) -> Self {
            let mut map = HashMap::new();
            for (addr, coins) in balances.iter() {
                map.insert(addr.to_string(), coins.to_vec());
            }
            BankQuerier { balances: map }
        }

        pub fn query(&self, request: &BankQuery) -> QuerierResult {
            let contract_result: ContractResult<Binary> = match request {
                BankQuery::Balance { address, denom } => {
                    // proper error on not found, serialize result on found
                    let amount = self
                        .balances
                        .get(address)
                        .and_then(|v| v.iter().find(|c| &c.denom == denom).map(|c| c.amount))
                        .unwrap_or_default();
                    let bank_res = BalanceResponse {
                        amount: Coin {
                            amount,
                            denom: denom.to_string(),
                        },
                    };
                    to_binary(&bank_res).into()
                }
                BankQuery::AllBalances { address } => {
                    // proper error on not found, serialize result on found
                    let bank_res = AllBalanceResponse {
                        amount: self.balances.get(address).cloned().unwrap_or_default(),
                    };
                    to_binary(&bank_res).into()
                }
                _ => {
                    return SystemResult::Err(SystemError::UnsupportedRequest {
                        kind: "custom".to_string(),
                    })
                }
            };
            // system result is always ok in the mock implementation
            SystemResult::Ok(contract_result)
        }
    }

    #[cfg(feature = "staking")]
    #[derive(Clone, Default)]
    pub struct StakingQuerier {
        denom: String,
        validators: Vec<Validator>,
        delegations: Vec<FullDelegation>,
    }

    #[cfg(feature = "staking")]
    impl StakingQuerier {
        pub fn new(denom: &str, validators: &[Validator], delegations: &[FullDelegation]) -> Self {
            StakingQuerier {
                denom: denom.to_string(),
                validators: validators.to_vec(),
                delegations: delegations.to_vec(),
            }
        }

        pub fn query(&self, request: &StakingQuery) -> QuerierResult {
            let contract_result: ContractResult<Binary> = match request {
                StakingQuery::BondedDenom {} => {
                    let res = BondedDenomResponse {
                        denom: self.denom.clone(),
                    };
                    to_binary(&res).into()
                }
                StakingQuery::AllValidators {} => {
                    let res = AllValidatorsResponse {
                        validators: self.validators.clone(),
                    };
                    to_binary(&res).into()
                }
                StakingQuery::Validator { address } => {
                    let validator: Option<Validator> = self
                        .validators
                        .iter()
                        .find(|validator| validator.address == *address)
                        .cloned();
                    let res = ValidatorResponse { validator };
                    to_binary(&res).into()
                }
                StakingQuery::AllDelegations { delegator } => {
                    let delegations: Vec<_> = self
                        .delegations
                        .iter()
                        .filter(|d| d.delegator.as_str() == delegator)
                        .cloned()
                        .map(|d| d.into())
                        .collect();
                    let res = AllDelegationsResponse { delegations };
                    to_binary(&res).into()
                }
                StakingQuery::Delegation {
                    delegator,
                    validator,
                } => {
                    let delegation = self
                        .delegations
                        .iter()
                        .find(|d| d.delegator.as_str() == delegator && d.validator == *validator);
                    let res = DelegationResponse {
                        delegation: delegation.cloned(),
                    };
                    to_binary(&res).into()
                }
            };
            // system result is always ok in the mock implementation
            SystemResult::Ok(contract_result)
        }
    }

    #[derive(Clone, Default)]
    struct CustomWasmQuerier {
        // FIXME: actually provide a way to call out
    }

    impl CustomWasmQuerier {
        fn query(&self, request: &WasmQuery) -> QuerierResult {
            match request {
                WasmQuery::Smart { contract_addr, msg } => {
                    if contract_addr == "vault_token" {
                        let msg: Cw4626QueryMsg = from_binary(msg).unwrap();
                        match msg {
                            Cw4626QueryMsg::Asset {} => {
                                let contract_result: ContractResult<Binary> =
                                    to_binary(&AssetInfo::Coin("utest".to_string())).into();
                                SystemResult::Ok(contract_result)
                            }
                            _ => SystemResult::Err(SystemError::UnsupportedRequest {
                                kind: "message of vault token".to_string(),
                            }),
                        }
                    } else {
                        SystemResult::Err(SystemError::NoSuchContract {
                            addr: contract_addr.clone(),
                        })
                    }
                }
                _ => SystemResult::Err(SystemError::UnsupportedRequest {
                    kind: "custom".to_string(),
                }),
            }
        }
    }
}
