// Exposed on all platforms

mod coins;
mod encoding;
mod entry_points;
mod errors;
mod init_handle;
#[cfg(feature = "iterator")]
mod iterator;
mod math;
mod query;
mod serde;
mod storage;
mod traits;
mod types;

pub use crate::coins::{coin, coins, has_coins, Coin};
pub use crate::encoding::Binary;
pub use crate::errors::{StdError, StdResult, SystemError, SystemResult};
pub use crate::init_handle::{
    log, BankMsg, Context, CosmosMsg, HandleResponse, HandleResult, InitResponse, InitResult,
    LogAttribute, MigrateResponse, MigrateResult, StakingMsg, WasmMsg,
};
#[cfg(feature = "iterator")]
pub use crate::iterator::{Order, KV};
pub use crate::math::{Decimal, Uint128};
pub use crate::query::{
    AllBalanceResponse, AllDelegationsResponse, BalanceResponse, BankQuery, BondedDenomResponse,
    Delegation, FullDelegation, QueryRequest, QueryResponse, QueryResult, StakingQuery, Validator,
    ValidatorsResponse, WasmQuery,
};
pub use crate::serde::{from_binary, from_slice, to_binary, to_vec};
pub use crate::storage::MemoryStorage;
pub use crate::traits::{Api, Extern, Querier, QuerierResult, ReadonlyStorage, Storage};
pub use crate::types::{
    BlockInfo, CanonicalAddr, ContractInfo, Empty, Env, HumanAddr, MessageInfo,
};

// Exposed in wasm build only

#[cfg(target_arch = "wasm32")]
mod exports;
#[cfg(target_arch = "wasm32")]
mod imports;
#[cfg(target_arch = "wasm32")]
mod memory; // Used by exports and imports only. This assumes pointers are 32 bit long, which makes it untestable on dev machines.

#[cfg(target_arch = "wasm32")]
pub use crate::exports::{do_handle, do_init, do_migrate, do_query};
#[cfg(target_arch = "wasm32")]
pub use crate::imports::{ExternalApi, ExternalQuerier, ExternalStorage};

// Exposed for testing only
// Both unit tests and integration tests are compiled to native code, so everything in here does not need to compile to Wasm.

#[cfg(not(target_arch = "wasm32"))]
mod mock;
#[cfg(not(target_arch = "wasm32"))]
pub mod testing {
    pub use crate::mock::{
        mock_dependencies, mock_dependencies_with_balances, mock_env, BankQuerier, MockApi,
        MockQuerier, MockQuerierCustomHandlerResult, MockStorage, StakingQuerier,
        MOCK_CONTRACT_ADDR,
    };
}
