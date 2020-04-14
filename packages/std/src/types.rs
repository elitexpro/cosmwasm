use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::coins::Coin;
use crate::encoding::Binary;

// Added Eq and Hash to allow this to be a key in a HashMap (MockQuerier)
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq, JsonSchema, Hash)]
pub struct HumanAddr(pub String);

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, JsonSchema)]
pub struct CanonicalAddr(pub Binary);

impl HumanAddr {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for HumanAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.0)
    }
}

impl From<&str> for HumanAddr {
    fn from(addr: &str) -> Self {
        HumanAddr(addr.to_string())
    }
}

impl From<&HumanAddr> for HumanAddr {
    fn from(addr: &HumanAddr) -> Self {
        HumanAddr(addr.0.to_string())
    }
}

impl From<&&HumanAddr> for HumanAddr {
    fn from(addr: &&HumanAddr) -> Self {
        HumanAddr(addr.0.to_string())
    }
}

impl CanonicalAddr {
    pub fn as_slice(&self) -> &[u8] {
        &self.0.as_slice()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for CanonicalAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, JsonSchema)]
pub struct Env {
    pub block: BlockInfo,
    pub message: MessageInfo,
    pub contract: ContractInfo,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, JsonSchema)]
pub struct BlockInfo {
    pub height: u64,
    // time is seconds since epoch begin (Jan. 1, 1970)
    pub time: u64,
    pub chain_id: String,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, JsonSchema)]
pub struct MessageInfo {
    pub signer: CanonicalAddr,
    pub sent_funds: Vec<Coin>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, JsonSchema)]
pub struct ContractInfo {
    pub address: CanonicalAddr,
}
