use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::ApiError;
use crate::encoding::Binary;
use crate::types::{Coin, HumanAddr};

pub type QueryResponse = Binary;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryResult {
    Ok(QueryResponse),
    Err(ApiError),
}

impl QueryResult {
    // unwrap will panic on err, or give us the real data useful for tests
    pub fn unwrap(self) -> QueryResponse {
        match self {
            QueryResult::Err(msg) => panic!("Unexpected error: {}", msg),
            QueryResult::Ok(res) => res,
        }
    }

    pub fn is_err(&self) -> bool {
        match self {
            QueryResult::Err(_) => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryRequest {
    // this queries the public API of another contract at a known address (with known ABI)
    // msg is the json-encoded QueryMsg struct
    // return value is whatever the contract returns (caller should know)
    Contract {
        contract_addr: HumanAddr,
        msg: Binary, // we pass this in as Vec<u8> to the contract, so allow any binary encoding (later, limit to rawjson?)
    },
    // this calls into the native bank module
    // return value is BalanceResponse
    Balance {
        address: HumanAddr,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct BalanceResponse {
    pub amount: Option<Vec<Coin>>,
}
