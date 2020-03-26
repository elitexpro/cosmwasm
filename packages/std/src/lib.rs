// Exposed on all platforms

mod encoding;
mod errors;
mod serde;
mod storage;
mod traits;
mod transactions;
mod types;

pub use crate::encoding::Binary;
pub use crate::errors::{
    contract_err, dyn_contract_err, invalid, unauthorized, Error, NotFound, NullPointer, ParseErr,
    Result, SerializeErr,
};
pub use crate::serde::{from_slice, to_vec};
pub use crate::storage::MemoryStorage;
pub use crate::traits::{Api, Extern, ReadonlyStorage, Storage};
#[cfg(feature = "iterator")]
pub use crate::traits::{Order, Pair};
pub use crate::transactions::{transactional, transactional_deps, RepLog, StorageTransaction};
pub use crate::types::{
    coin, log, CanonicalAddr, ContractResult, CosmosMsg, Env, HumanAddr, QueryResult, Response,
};

// Exposed in wasm build only

#[cfg(target_arch = "wasm32")]
mod exports;
#[cfg(target_arch = "wasm32")]
mod imports;
#[cfg(target_arch = "wasm32")]
mod memory; // Used by exports and imports only. This assumes pointers are 32 bit long, which makes it untestable on dev machines.

#[cfg(target_arch = "wasm32")]
pub use crate::exports::{allocate, deallocate, do_handle, do_init, do_query};
#[cfg(target_arch = "wasm32")]
pub use crate::imports::{ExternalApi, ExternalStorage};

// Exposed for testing only
// Both unit tests and integration tests are compiled to native code, so everything in here does not need to compile to Wasm.

#[cfg(not(target_arch = "wasm32"))]
mod mock;
#[cfg(not(target_arch = "wasm32"))]
pub mod testing {
    pub use crate::mock::{mock_dependencies, mock_env, MockApi, MockStorage};
}
