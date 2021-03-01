//! exports exposes the public wasm API
//!
//! interface_version_5, allocate and deallocate turn into Wasm exports
//! as soon as cosmwasm_std is `use`d in the contract, even privately.
//!
//! do_init and do_wrapper should be wrapped with a extern "C" entry point
//! including the contract-specific init/handle function pointer.
use std::fmt;
use std::vec::Vec;

use schemars::JsonSchema;
use serde::{de::DeserializeOwned, Serialize};

use crate::deps::OwnedDeps;
use crate::imports::{ExternalApi, ExternalQuerier, ExternalStorage};
use crate::memory::{alloc, consume_region, release_buffer, Region};
use crate::results::{ContractResult, QueryResponse, Response, SubcallResult};
use crate::serde::{from_slice, to_vec};
use crate::types::Env;
use crate::{Deps, DepsMut, MessageInfo};

#[cfg(feature = "staking")]
#[no_mangle]
extern "C" fn requires_staking() -> () {}

#[cfg(feature = "stargate")]
#[no_mangle]
extern "C" fn requires_stargate() -> () {}

/// interface_version_* exports mark which Wasm VM interface level this contract is compiled for.
/// They can be checked by cosmwasm_vm.
/// Update this whenever the Wasm VM interface breaks.
#[no_mangle]
extern "C" fn interface_version_5() -> () {}

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

// TODO: replace with https://doc.rust-lang.org/std/ops/trait.Try.html once stabilized
macro_rules! r#try_into_contract_result {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(err) => {
                return ContractResult::Err(err.to_string());
            }
        }
    };
    ($expr:expr,) => {
        $crate::try_into_contract_result!($expr)
    };
}

/// do_init should be wrapped in an external "C" export, containing a contract-specific function as arg
///
/// - `M`: message type for request
/// - `C`: custom response message type (see CosmosMsg)
/// - `E`: error type for responses
pub fn do_init<M, C, E>(
    init_fn: &dyn Fn(DepsMut, Env, MessageInfo, M) -> Result<Response<C>, E>,
    env_ptr: u32,
    info_ptr: u32,
    msg_ptr: u32,
) -> u32
where
    M: DeserializeOwned + JsonSchema,
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
    E: ToString,
{
    let res = _do_init(
        init_fn,
        env_ptr as *mut Region,
        info_ptr as *mut Region,
        msg_ptr as *mut Region,
    );
    let v = to_vec(&res).unwrap();
    release_buffer(v) as u32
}

/// do_handle should be wrapped in an external "C" export, containing a contract-specific function as arg
///
/// - `M`: message type for request
/// - `C`: custom response message type (see CosmosMsg)
/// - `E`: error type for responses
pub fn do_handle<M, C, E>(
    handle_fn: &dyn Fn(DepsMut, Env, MessageInfo, M) -> Result<Response<C>, E>,
    env_ptr: u32,
    info_ptr: u32,
    msg_ptr: u32,
) -> u32
where
    M: DeserializeOwned + JsonSchema,
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
    E: ToString,
{
    let res = _do_handle(
        handle_fn,
        env_ptr as *mut Region,
        info_ptr as *mut Region,
        msg_ptr as *mut Region,
    );
    let v = to_vec(&res).unwrap();
    release_buffer(v) as u32
}

/// do_migrate should be wrapped in an external "C" export, containing a contract-specific function as arg
///
/// - `M`: message type for request
/// - `C`: custom response message type (see CosmosMsg)
/// - `E`: error type for responses
pub fn do_migrate<M, C, E>(
    migrate_fn: &dyn Fn(DepsMut, Env, M) -> Result<Response<C>, E>,
    env_ptr: u32,
    msg_ptr: u32,
) -> u32
where
    M: DeserializeOwned + JsonSchema,
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
    E: ToString,
{
    let res = _do_migrate(migrate_fn, env_ptr as *mut Region, msg_ptr as *mut Region);
    let v = to_vec(&res).unwrap();
    release_buffer(v) as u32
}

/// do_system should be wrapped in an external "C" export, containing a contract-specific function as arg
///
/// - `M`: message type for request
/// - `C`: custom response message type (see CosmosMsg)
/// - `E`: error type for responses
pub fn do_system<M, C, E>(
    system_fn: &dyn Fn(DepsMut, Env, M) -> Result<Response<C>, E>,
    env_ptr: u32,
    msg_ptr: u32,
) -> u32
where
    M: DeserializeOwned + JsonSchema,
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
    E: ToString,
{
    let res = _do_system(system_fn, env_ptr as *mut Region, msg_ptr as *mut Region);
    let v = to_vec(&res).unwrap();
    release_buffer(v) as u32
}

/// do_subcall_response should be wrapped in an external "C" export, containing a contract-specific function as arg
/// message body is always `SubcallResult`
/// - `C`: custom response message type (see CosmosMsg)
/// - `E`: error type for responses
pub fn do_subcall_response<C, E>(
    subcall_fn: &dyn Fn(DepsMut, Env, SubcallResult) -> Result<Response<C>, E>,
    env_ptr: u32,
    msg_ptr: u32,
) -> u32
where
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
    E: ToString,
{
    let res = _do_subcall_response(subcall_fn, env_ptr as *mut Region, msg_ptr as *mut Region);
    let v = to_vec(&res).unwrap();
    release_buffer(v) as u32
}

/// do_query should be wrapped in an external "C" export, containing a contract-specific function as arg
///
/// - `M`: message type for request
/// - `E`: error type for responses
pub fn do_query<M, E>(
    query_fn: &dyn Fn(Deps, Env, M) -> Result<QueryResponse, E>,
    env_ptr: u32,
    msg_ptr: u32,
) -> u32
where
    M: DeserializeOwned + JsonSchema,
    E: ToString,
{
    let res = _do_query(query_fn, env_ptr as *mut Region, msg_ptr as *mut Region);
    let v = to_vec(&res).unwrap();
    release_buffer(v) as u32
}

fn _do_init<M, C, E>(
    init_fn: &dyn Fn(DepsMut, Env, MessageInfo, M) -> Result<Response<C>, E>,
    env_ptr: *mut Region,
    info_ptr: *mut Region,
    msg_ptr: *mut Region,
) -> ContractResult<Response<C>>
where
    M: DeserializeOwned + JsonSchema,
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
    E: ToString,
{
    let env: Vec<u8> = unsafe { consume_region(env_ptr) };
    let info: Vec<u8> = unsafe { consume_region(info_ptr) };
    let msg: Vec<u8> = unsafe { consume_region(msg_ptr) };

    let env: Env = try_into_contract_result!(from_slice(&env));
    let info: MessageInfo = try_into_contract_result!(from_slice(&info));
    let msg: M = try_into_contract_result!(from_slice(&msg));

    let mut deps = make_dependencies();
    init_fn(deps.as_mut(), env, info, msg).into()
}

fn _do_handle<M, C, E>(
    handle_fn: &dyn Fn(DepsMut, Env, MessageInfo, M) -> Result<Response<C>, E>,
    env_ptr: *mut Region,
    info_ptr: *mut Region,
    msg_ptr: *mut Region,
) -> ContractResult<Response<C>>
where
    M: DeserializeOwned + JsonSchema,
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
    E: ToString,
{
    let env: Vec<u8> = unsafe { consume_region(env_ptr) };
    let info: Vec<u8> = unsafe { consume_region(info_ptr) };
    let msg: Vec<u8> = unsafe { consume_region(msg_ptr) };

    let env: Env = try_into_contract_result!(from_slice(&env));
    let info: MessageInfo = try_into_contract_result!(from_slice(&info));
    let msg: M = try_into_contract_result!(from_slice(&msg));

    let mut deps = make_dependencies();
    handle_fn(deps.as_mut(), env, info, msg).into()
}

fn _do_migrate<M, C, E>(
    migrate_fn: &dyn Fn(DepsMut, Env, M) -> Result<Response<C>, E>,
    env_ptr: *mut Region,
    msg_ptr: *mut Region,
) -> ContractResult<Response<C>>
where
    M: DeserializeOwned + JsonSchema,
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
    E: ToString,
{
    let env: Vec<u8> = unsafe { consume_region(env_ptr) };
    let msg: Vec<u8> = unsafe { consume_region(msg_ptr) };

    let env: Env = try_into_contract_result!(from_slice(&env));
    let msg: M = try_into_contract_result!(from_slice(&msg));

    let mut deps = make_dependencies();
    migrate_fn(deps.as_mut(), env, msg).into()
}

fn _do_system<M, C, E>(
    system_fn: &dyn Fn(DepsMut, Env, M) -> Result<Response<C>, E>,
    env_ptr: *mut Region,
    msg_ptr: *mut Region,
) -> ContractResult<Response<C>>
where
    M: DeserializeOwned + JsonSchema,
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
    E: ToString,
{
    let env: Vec<u8> = unsafe { consume_region(env_ptr) };
    let msg: Vec<u8> = unsafe { consume_region(msg_ptr) };

    let env: Env = try_into_contract_result!(from_slice(&env));
    let msg: M = try_into_contract_result!(from_slice(&msg));

    let mut deps = make_dependencies();
    system_fn(deps.as_mut(), env, msg).into()
}

fn _do_subcall_response<C, E>(
    subcall_fn: &dyn Fn(DepsMut, Env, SubcallResult) -> Result<Response<C>, E>,
    env_ptr: *mut Region,
    msg_ptr: *mut Region,
) -> ContractResult<Response<C>>
where
    C: Serialize + Clone + fmt::Debug + PartialEq + JsonSchema,
    E: ToString,
{
    let env: Vec<u8> = unsafe { consume_region(env_ptr) };
    let msg: Vec<u8> = unsafe { consume_region(msg_ptr) };

    let env: Env = try_into_contract_result!(from_slice(&env));
    let msg: SubcallResult = try_into_contract_result!(from_slice(&msg));

    let mut deps = make_dependencies();
    subcall_fn(deps.as_mut(), env, msg).into()
}

fn _do_query<M, E>(
    query_fn: &dyn Fn(Deps, Env, M) -> Result<QueryResponse, E>,
    env_ptr: *mut Region,
    msg_ptr: *mut Region,
) -> ContractResult<QueryResponse>
where
    M: DeserializeOwned + JsonSchema,
    E: ToString,
{
    let env: Vec<u8> = unsafe { consume_region(env_ptr) };
    let msg: Vec<u8> = unsafe { consume_region(msg_ptr) };

    let env: Env = try_into_contract_result!(from_slice(&env));
    let msg: M = try_into_contract_result!(from_slice(&msg));

    let deps = make_dependencies();
    query_fn(deps.as_ref(), env, msg).into()
}

/// Makes all bridges to external dependencies (i.e. Wasm imports) that are injected by the VM
pub(crate) fn make_dependencies() -> OwnedDeps<ExternalStorage, ExternalApi, ExternalQuerier> {
    OwnedDeps {
        storage: ExternalStorage::new(),
        api: ExternalApi::new(),
        querier: ExternalQuerier::new(),
    }
}
