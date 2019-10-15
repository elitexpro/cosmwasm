use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Params {
    pub block: BlockInfo,
    pub message: MessageInfo,
    pub contract: ContractInfo,
}

#[derive(Serialize, Deserialize)]
pub struct BlockInfo {
    pub height: i64,
    // block_time is RFC3339 encoded timestamp
    pub time: String,
    pub chain_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct MessageInfo {
    pub signer: String,
    pub sent_funds: Vec<SendAmount>,
}

#[derive(Serialize, Deserialize)]
pub struct ContractInfo {
    pub address: String,
    pub balance: Vec<SendAmount>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SendAmount {
    pub denom: String,
    pub amount: String,
}

#[derive(Serialize, Deserialize)]
pub enum CosmosMsg {
    #[serde(rename = "send")]
    SendTx {
        from_address: String,
        to_address: String,
        amount: Vec<SendAmount>,
    },
}

// TODO: clean this up - let's us a normal result type??? or at least normal terms

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContractResult {
    Ok(Response),
    Err(String),
}

impl ContractResult {
    // unwrap will panic on err, or give us the real data useful for tests
    pub fn unwrap(self) -> Response {
        match self {
            ContractResult::Err(msg) => panic!("Unexpected error: {}", msg),
            ContractResult::Ok(res) => res,
        }
    }

    pub fn is_err(&self) -> bool {
        match self {
            ContractResult::Err(_) => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Response {
    // let's make the positive case a struct, it contrains Msg: {...}, but also Data, Log, maybe later Events, etc.
    pub messages: Vec<CosmosMsg>,
    pub log: Option<String>,
}

// just set signer, sent funds, and balance - rest given defaults
// this is intended for use in testcode only
pub fn mock_params(signer: &str, sent: &[SendAmount], balance: &[SendAmount]) -> Params {
    Params {
        block: BlockInfo {
            height: 12345,
            time: "2020-01-08T12:34:56Z".to_string(),
            chain_id: "cosmos-testnet-14002".to_string(),
        },
        message: MessageInfo {
            signer: signer.to_string(),
            sent_funds: sent.to_vec(),
        },
        contract: ContractInfo {
            address: "cosmos2contract".to_string(),
            balance: balance.to_vec(),
        },
    }
}

// coin is a shortcut constructor for a set of one denomination of coins
pub fn coin(amount: &str, denom: &str) -> Vec<SendAmount> {
    vec![SendAmount {
        amount: amount.to_string(),
        denom: denom.to_string(),
    }]
}
