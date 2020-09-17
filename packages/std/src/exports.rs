//! exports exposes the public wasm API
//!
//! cosmwasm_vm_version_3, allocate and deallocate turn into Wasm exports
//! as soon as cosmwasm_std is `use`d in the contract, even privately.
//!
//! do_init and do_wrapper should be wrapped with a extern "C" entry point
//! including the contract-specific init/handle function pointer.
use std::fmt;
use std::vec::Vec;

use schemars::JsonSchema;
use serde::{de::DeserializeOwned, Serialize};

use crate::imports::{ExternalApi, ExternalQuerier, ExternalStorage};
use crate::memory::{alloc, consume_region, release_buffer, Region};
use crate::serde::{from_slice, to_vec};
use crate::traits::Extern;
use crate::{Env, HandleResult, InitResult, MigrateResult, QueryResult};

#[cfg(feature = "staking")]
#[no_mangle]
extern "C" fn requires_staking() -> () {}

/// cosmwasm_vm_version_* exports mark which Wasm VM interface level this contract is compiled for.
/// They can be checked by cosmwasm_vm.
/// Update this whenever the Wasm VM interface breaks.
#[no_mangle]
extern "C" fn cosmwasm_vm_version_3() -> () {}

/// allocate reserves the given number of bytes in wasm memory and returns a pointer
/// to a Region defining this data. This space is managed by the calling process
/// and should be accompanied by a corresponding deallocate
#[no_mangle]
extern "C" fn allocate(size: usize) -> u32 {
    alloc(size) as u32
}

/// deallocate expects a pointer to a Region created with allocate.
/// It will free both the Region and the memory referenced by the Region.
#[no_mangle]
extern "C" fn deallocate(pointer: u32) {
    // auto-drop Region on function end
    let _ = unsafe { consume_region(pointer as *mut Region) };
}

/// do_init should be wrapped in an external "C" export, containing a contract-specific function as arg
///
/// - `M`: message type for request
/// - `C`: custom response message type (see CosmosMsg)
pub fn do_init<M, C>(
    init_fn: &dyn Fn(
        &mut Extern<ExternalStorage, ExternalApi, ExternalQuerier>,
        Env,
        M,
    ) -> InitResult<C>,
    env_ptr: u32,
    msg_ptr: u32,
) -> u32
where
    M: DeserializeOwned + JsonSchema,
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
{
    let res = _do_init(init_fn, env_ptr as *mut Region, msg_ptr as *mut Region);
    let v = to_vec(&res).unwrap();
    release_buffer(v) as u32
}

/// do_handle should be wrapped in an external "C" export, containing a contract-specific function as arg
///
/// - `M`: message type for request
/// - `C`: custom response message type (see CosmosMsg)
pub fn do_handle<M, C>(
    handle_fn: &dyn Fn(
        &mut Extern<ExternalStorage, ExternalApi, ExternalQuerier>,
        Env,
        M,
    ) -> HandleResult<C>,
    env_ptr: u32,
    msg_ptr: u32,
) -> u32
where
    M: DeserializeOwned + JsonSchema,
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
{
    let res = _do_handle(handle_fn, env_ptr as *mut Region, msg_ptr as *mut Region);
    let v = to_vec(&res).unwrap();
    release_buffer(v) as u32
}

/// do_query should be wrapped in an external "C" export, containing a contract-specific function as arg
///
/// - `M`: message type for request
pub fn do_query<M: DeserializeOwned + JsonSchema>(
    query_fn: &dyn Fn(&Extern<ExternalStorage, ExternalApi, ExternalQuerier>, M) -> QueryResult,
    msg_ptr: u32,
) -> u32 {
    let res = _do_query(query_fn, msg_ptr as *mut Region);
    let v = to_vec(&res).unwrap();
    release_buffer(v) as u32
}

/// do_migrate should be wrapped in an external "C" export, containing a contract-specific function as arg
///
/// - `M`: message type for request
/// - `C`: custom response message type (see CosmosMsg)
pub fn do_migrate<M, C>(
    migrate_fn: &dyn Fn(
        &mut Extern<ExternalStorage, ExternalApi, ExternalQuerier>,
        Env,
        M,
    ) -> MigrateResult<C>,
    env_ptr: u32,
    msg_ptr: u32,
) -> u32
where
    M: DeserializeOwned + JsonSchema,
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
{
    let res = _do_migrate(migrate_fn, env_ptr as *mut Region, msg_ptr as *mut Region);
    let v = to_vec(&res).unwrap();
    release_buffer(v) as u32
}

fn _do_init<M, C>(
    init_fn: &dyn Fn(
        &mut Extern<ExternalStorage, ExternalApi, ExternalQuerier>,
        Env,
        M,
    ) -> InitResult<C>,
    env_ptr: *mut Region,
    msg_ptr: *mut Region,
) -> InitResult<C>
where
    M: DeserializeOwned + JsonSchema,
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
{
    let env: Vec<u8> = unsafe { consume_region(env_ptr) };
    let msg: Vec<u8> = unsafe { consume_region(msg_ptr) };
    let env: Env = from_slice(&env)?;
    let msg: M = from_slice(&msg)?;
    let mut deps = make_dependencies();
    init_fn(&mut deps, env, msg)
}

fn _do_handle<M, C>(
    handle_fn: &dyn Fn(
        &mut Extern<ExternalStorage, ExternalApi, ExternalQuerier>,
        Env,
        M,
    ) -> HandleResult<C>,
    env_ptr: *mut Region,
    msg_ptr: *mut Region,
) -> HandleResult<C>
where
    M: DeserializeOwned + JsonSchema,
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
{
    let env: Vec<u8> = unsafe { consume_region(env_ptr) };
    let msg: Vec<u8> = unsafe { consume_region(msg_ptr) };

    let env: Env = from_slice(&env)?;
    let msg: M = from_slice(&msg)?;
    let mut deps = make_dependencies();
    handle_fn(&mut deps, env, msg)
}

fn _do_query<M: DeserializeOwned + JsonSchema>(
    query_fn: &dyn Fn(&Extern<ExternalStorage, ExternalApi, ExternalQuerier>, M) -> QueryResult,
    msg_ptr: *mut Region,
) -> QueryResult {
    let msg: Vec<u8> = unsafe { consume_region(msg_ptr) };

    let msg: M = from_slice(&msg)?;
    let deps = make_dependencies();
    query_fn(&deps, msg)
}

fn _do_migrate<M, C>(
    migrate_fn: &dyn Fn(
        &mut Extern<ExternalStorage, ExternalApi, ExternalQuerier>,
        Env,
        M,
    ) -> MigrateResult<C>,
    env_ptr: *mut Region,
    msg_ptr: *mut Region,
) -> MigrateResult<C>
where
    M: DeserializeOwned + JsonSchema,
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
{
    let env: Vec<u8> = unsafe { consume_region(env_ptr) };
    let msg: Vec<u8> = unsafe { consume_region(msg_ptr) };
    let env: Env = from_slice(&env)?;
    let msg: M = from_slice(&msg)?;
    let mut deps = make_dependencies();
    migrate_fn(&mut deps, env, msg)
}

/// Makes all bridges to external dependencies (i.e. Wasm imports) that are injected by the VM
fn make_dependencies() -> Extern<ExternalStorage, ExternalApi, ExternalQuerier> {
    Extern {
        storage: ExternalStorage::new(),
        api: ExternalApi::new(),
        querier: ExternalQuerier::new(),
    }
}
