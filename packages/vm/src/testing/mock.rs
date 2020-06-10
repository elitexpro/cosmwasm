use serde::{de::DeserializeOwned, Serialize};

use cosmwasm_std::testing::{MockQuerier as StdMockQuerier, MockQuerierCustomHandlerResult};
use cosmwasm_std::{
    to_binary, Binary, BlockInfo, CanonicalAddr, Coin, ContractInfo, Env, HumanAddr, MessageInfo,
    Never, Querier as _, QueryRequest, SystemError,
};

use super::storage::MockStorage;
use crate::{Api, Extern, FfiError, FfiResult, Querier, QuerierResult};

pub const MOCK_CONTRACT_ADDR: &str = "cosmos2contract";

/// All external requirements that can be injected for unit tests.
/// It sets the given balance for the contract itself, nothing else
pub fn mock_dependencies(
    canonical_length: usize,
    contract_balance: &[Coin],
) -> Extern<MockStorage, MockApi, MockQuerier> {
    let contract_addr = HumanAddr::from(MOCK_CONTRACT_ADDR);
    Extern {
        storage: MockStorage::default(),
        api: MockApi::new(canonical_length),
        querier: MockQuerier::new(&[(&contract_addr, contract_balance)]),
    }
}

/// Initializes the querier along with the mock_dependencies.
/// Sets all balances provided (yoy must explicitly set contract balance if desired)
pub fn mock_dependencies_with_balances(
    canonical_length: usize,
    balances: &[(&HumanAddr, &[Coin])],
) -> Extern<MockStorage, MockApi, MockQuerier> {
    Extern {
        storage: MockStorage::default(),
        api: MockApi::new(canonical_length),
        querier: MockQuerier::new(balances),
    }
}

// MockPrecompiles zero pads all human addresses to make them fit the canonical_length
// it trims off zeros for the reverse operation.
// not really smart, but allows us to see a difference (and consistent length for canonical adddresses)
#[derive(Copy, Clone)]
pub struct MockApi {
    canonical_length: usize,
    error_message: Option<&'static str>,
}

impl MockApi {
    pub fn new(canonical_length: usize) -> Self {
        MockApi {
            canonical_length,
            error_message: None,
        }
    }

    pub fn new_failing(canonical_length: usize, error_message: &'static str) -> Self {
        MockApi {
            canonical_length,
            error_message: Some(error_message),
        }
    }
}

impl Default for MockApi {
    fn default() -> Self {
        Self::new(20)
    }
}

impl Api for MockApi {
    fn canonical_address(&self, human: &HumanAddr) -> FfiResult<CanonicalAddr> {
        if let Some(error_message) = self.error_message {
            return Err(FfiError::other(error_message));
        }

        // Dummy input validation. This is more sophisticated for formats like bech32, where format and checksum are validated.
        if human.len() < 3 {
            return Err(FfiError::other("Invalid input: human address too short"));
        }
        if human.len() > self.canonical_length {
            return Err(FfiError::other("Invalid input: human address too long"));
        }

        let mut out = Vec::from(human.as_str());
        let append = self.canonical_length - out.len();
        if append > 0 {
            out.extend(vec![0u8; append]);
        }
        Ok(CanonicalAddr(Binary(out)))
    }

    fn human_address(&self, canonical: &CanonicalAddr) -> FfiResult<HumanAddr> {
        if let Some(error_message) = self.error_message {
            return Err(FfiError::other(error_message));
        }

        if canonical.len() != self.canonical_length {
            return Err(FfiError::other(
                "Invalid input: canonical address length not correct",
            ));
        }

        // remove trailing 0's (TODO: fix this - but fine for first tests)
        let trimmed: Vec<u8> = canonical
            .as_slice()
            .iter()
            .cloned()
            .filter(|&x| x != 0)
            .collect();
        // decode UTF-8 bytes into string
        let human = String::from_utf8(trimmed)
            .map_err(|_| FfiError::other("Could not parse human address result as utf-8"))?;
        Ok(HumanAddr(human))
    }
}

/// Just set sender and sent funds for the message. The rest uses defaults.
/// The sender will be canonicalized internally to allow developers pasing in human readable senders.
/// This is intended for use in test code only.
pub fn mock_env<T: Api, U: Into<HumanAddr>>(api: &T, sender: U, sent: &[Coin]) -> Env {
    Env {
        block: BlockInfo {
            height: 12_345,
            time: 1_571_797_419,
            chain_id: "cosmos-testnet-14002".to_string(),
        },
        message: MessageInfo {
            sender: api.canonical_address(&sender.into()).unwrap(),
            sent_funds: sent.to_vec(),
        },
        contract: ContractInfo {
            address: api
                .canonical_address(&HumanAddr::from(MOCK_CONTRACT_ADDR))
                .unwrap(),
        },
    }
}

/// MockQuerier holds an immutable table of bank balances
/// TODO: also allow querying contracts
pub struct MockQuerier<C: DeserializeOwned = Never> {
    querier: StdMockQuerier<C>,
}

impl<C: DeserializeOwned> MockQuerier<C> {
    pub fn new(balances: &[(&HumanAddr, &[Coin])]) -> Self {
        MockQuerier {
            querier: StdMockQuerier::new(balances),
        }
    }

    // set a new balance for the given address and return the old balance
    pub fn update_balance<U: Into<HumanAddr>>(
        &mut self,
        addr: U,
        balance: Vec<Coin>,
    ) -> Option<Vec<Coin>> {
        self.querier.update_balance(addr, balance)
    }

    #[cfg(feature = "staking")]
    pub fn update_staking(
        &mut self,
        denom: &str,
        validators: &[cosmwasm_std::Validator],
        delegations: &[cosmwasm_std::FullDelegation],
    ) {
        self.querier.update_staking(denom, validators, delegations);
    }

    pub fn with_custom_handler<CH: 'static>(mut self, handler: CH) -> Self
    where
        CH: Fn(&C) -> MockQuerierCustomHandlerResult,
    {
        self.querier = self.querier.with_custom_handler(handler);
        self
    }
}

impl<C: DeserializeOwned> Querier for MockQuerier<C> {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let res = self.querier.raw_query(bin_request);
        let used_gas = (bin_request.len() + to_binary(&res).unwrap().len()) as u64;
        // We don't use FFI, so FfiResult is always Ok() regardless of error on other levels
        Ok((res, used_gas))
    }
}

impl MockQuerier {
    pub fn handle_query<T: Serialize>(&self, request: &QueryRequest<T>) -> QuerierResult {
        // encode the request, then call raw_query
        let bin = match to_binary(request) {
            Ok(raw) => raw,
            Err(e) => {
                let used_gas = e.to_string().len() as u64;
                return Ok((
                    Err(SystemError::InvalidRequest {
                        error: format!("Serializing query request: {}", e),
                        request: Binary(b"N/A".to_vec()),
                    }),
                    used_gas,
                ));
            }
        };
        self.raw_query(bin.as_slice())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::FfiError;
    use cosmwasm_std::{
        coin, coins, from_binary, AllBalanceResponse, BalanceResponse, BankQuery, Never,
    };

    #[test]
    fn mock_env_arguments() {
        let name = HumanAddr("my name".to_string());
        let api = MockApi::new(20);

        // make sure we can generate with &str, &HumanAddr, and HumanAddr
        let a = mock_env(&api, "my name", &coins(100, "atom"));
        let b = mock_env(&api, &name, &coins(100, "atom"));
        let c = mock_env(&api, name, &coins(100, "atom"));

        // and the results are the same
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn flip_addresses() {
        let api = MockApi::new(20);
        let human = HumanAddr("shorty".to_string());
        let canon = api.canonical_address(&human).unwrap();
        assert_eq!(canon.len(), 20);
        assert_eq!(&canon.as_slice()[0..6], human.as_str().as_bytes());
        assert_eq!(&canon.as_slice()[6..], &[0u8; 14]);

        let recovered = api.human_address(&canon).unwrap();
        assert_eq!(human, recovered);
    }

    #[test]
    fn human_address_input_length() {
        let api = MockApi::new(10);
        let input = CanonicalAddr(Binary(vec![61; 11]));
        match api.human_address(&input).unwrap_err() {
            FfiError::Other { .. } => {}
            err => panic!("Unexpected error: {}", err),
        }
    }

    #[test]
    fn canonical_address_min_input_length() {
        let api = MockApi::new(10);
        let human = HumanAddr("1".to_string());
        match api.canonical_address(&human).unwrap_err() {
            FfiError::Other { .. } => {}
            err => panic!("Unexpected error: {}", err),
        }
    }

    #[test]
    fn canonical_address_max_input_length() {
        let api = MockApi::new(10);
        let human = HumanAddr("longer-than-10".to_string());
        match api.canonical_address(&human).unwrap_err() {
            FfiError::Other { .. } => {}
            err => panic!("Unexpected error: {}", err),
        }
    }

    #[test]
    fn bank_querier_all_balances() {
        let addr = HumanAddr::from("foobar");
        let balance = vec![coin(123, "ELF"), coin(777, "FLY")];
        let querier = MockQuerier::new(&[(&addr, &balance)]);

        // all
        let all = querier
            .handle_query::<Never>(
                &BankQuery::AllBalances {
                    address: addr.clone(),
                }
                .into(),
            )
            .unwrap()
            .0
            .unwrap()
            .unwrap();
        let res: AllBalanceResponse = from_binary(&all).unwrap();
        assert_eq!(&res.amount, &balance);
    }

    #[test]
    fn bank_querier_one_balance() {
        let addr = HumanAddr::from("foobar");
        let balance = vec![coin(123, "ELF"), coin(777, "FLY")];
        let querier = MockQuerier::new(&[(&addr, &balance)]);

        // one match
        let fly = querier
            .handle_query::<Never>(
                &BankQuery::Balance {
                    address: addr.clone(),
                    denom: "FLY".to_string(),
                }
                .into(),
            )
            .unwrap()
            .0
            .unwrap()
            .unwrap();
        let res: BalanceResponse = from_binary(&fly).unwrap();
        assert_eq!(res.amount, coin(777, "FLY"));

        // missing denom
        let miss = querier
            .handle_query::<Never>(
                &BankQuery::Balance {
                    address: addr.clone(),
                    denom: "MISS".to_string(),
                }
                .into(),
            )
            .unwrap()
            .0
            .unwrap()
            .unwrap();
        let res: BalanceResponse = from_binary(&miss).unwrap();
        assert_eq!(res.amount, coin(0, "MISS"));
    }

    #[test]
    fn bank_querier_missing_account() {
        let addr = HumanAddr::from("foobar");
        let balance = vec![coin(123, "ELF"), coin(777, "FLY")];
        let querier = MockQuerier::new(&[(&addr, &balance)]);

        // all balances on empty account is empty vec
        let all = querier
            .handle_query::<Never>(
                &BankQuery::AllBalances {
                    address: HumanAddr::from("elsewhere"),
                }
                .into(),
            )
            .unwrap()
            .0
            .unwrap()
            .unwrap();
        let res: AllBalanceResponse = from_binary(&all).unwrap();
        assert_eq!(res.amount, vec![]);

        // any denom on balances on empty account is empty coin
        let miss = querier
            .handle_query::<Never>(
                &BankQuery::Balance {
                    address: HumanAddr::from("elsewhere"),
                    denom: "ELF".to_string(),
                }
                .into(),
            )
            .unwrap()
            .0
            .unwrap()
            .unwrap();
        let res: BalanceResponse = from_binary(&miss).unwrap();
        assert_eq!(res.amount, coin(0, "ELF"));
    }
}
