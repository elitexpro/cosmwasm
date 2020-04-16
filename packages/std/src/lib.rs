// Exposed on all platforms

mod api;
mod coins;
mod encoding;
mod errors;
mod init_handle;
#[cfg(feature = "iterator")]
mod iterator;
mod query;
mod serde;
mod storage;
mod traits;
mod types;

pub use crate::api::{ApiError, ApiResult, ApiSystemError};
pub use crate::coins::{coin, coins, has_coins, Coin, Uint128};
pub use crate::encoding::Binary;
pub use crate::errors::{
    contract_err, dyn_contract_err, invalid, unauthorized, Error, InvalidRequest, NotFound,
    NullPointer, ParseErr, Result, SerializeErr,
};
pub use crate::init_handle::{
    log, CosmosMsg, HandleResponse, HandleResult, InitResponse, InitResult, LogAttribute,
};
#[cfg(feature = "iterator")]
pub use crate::iterator::{Order, KV};
pub use crate::query::{
    AllBalanceResponse, BalanceResponse, QueryRequest, QueryResponse, QueryResult,
};
pub use crate::serde::{from_binary, from_slice, to_binary, to_vec};
pub use crate::storage::MemoryStorage;
pub use crate::traits::{
    Api, ApiQuerierResponse, Extern, Querier, QuerierResponse, ReadonlyStorage, Storage,
};
pub use crate::types::{CanonicalAddr, Env, HumanAddr};

// Exposed in wasm build only

#[cfg(target_arch = "wasm32")]
mod exports;
#[cfg(target_arch = "wasm32")]
mod imports;
#[cfg(target_arch = "wasm32")]
mod memory; // Used by exports and imports only. This assumes pointers are 32 bit long, which makes it untestable on dev machines.

#[cfg(target_arch = "wasm32")]
pub use crate::exports::{do_handle, do_init, do_query};
#[cfg(target_arch = "wasm32")]
pub use crate::imports::{ExternalApi, ExternalQuerier, ExternalStorage};

// Exposed for testing only
// Both unit tests and integration tests are compiled to native code, so everything in here does not need to compile to Wasm.

#[cfg(not(target_arch = "wasm32"))]
mod mock;
#[cfg(not(target_arch = "wasm32"))]
pub mod testing {
    pub use crate::mock::{
        mock_dependencies, mock_dependencies_with_balances, mock_env, MockApi, MockQuerier,
        MockStorage,
    };
}
