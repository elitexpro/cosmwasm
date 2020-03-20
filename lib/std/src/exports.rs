#![cfg(target_arch = "wasm32")]

//! exports exposes the public wasm API
//! allocate and deallocate should be re-exported as is
//! do_init and do_wrapper should be wrapped with a extern "C" entry point
//! including the contract-specific init/handle function pointer.
use std::fmt::Display;
use std::os::raw::c_void;
use std::vec::Vec;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use snafu::ResultExt;

use crate::encoding::Binary;
use crate::errors::{Error, ParseErr, SerializeErr};
use crate::imports::{dependencies, ExternalApi, ExternalStorage};
use crate::memory::{alloc, consume_region, release_buffer};
use crate::serde::{from_slice, to_vec};
use crate::traits::Extern;
use crate::types::{ContractResult, Env, QueryResult, Response};

/// cosmwasm_api_* exports mark which api level this contract is compiled with (and compatible with).
/// they can be checked by cosmwasm_vm.
/// Update this at major releases, so we can follow contract compatibility in the frontend
#[no_mangle]
pub extern "C" fn cosmwasm_api_0_6() -> i32 {
    0x0603
}

/// allocate reserves the given number of bytes in wasm memory and returns a pointer
/// to a Region defining this data. This space is managed by the calling process
/// and should be accompanied by a corresponding deallocate
#[no_mangle]
pub extern "C" fn allocate(size: usize) -> *mut c_void {
    alloc(size)
}

/// deallocate expects a pointer to a Region created with allocate.
/// It will free both the Region and the memory referenced by the Region.
#[no_mangle]
pub extern "C" fn deallocate(pointer: *mut c_void) {
    // auto-drop Region on function end
    let _ = unsafe { consume_region(pointer) };
}

/// do_init should be wrapped in an external "C" export, containing a contract-specific function as arg
pub fn do_init<T: DeserializeOwned + JsonSchema>(
    init_fn: &dyn Fn(&mut Extern<ExternalStorage, ExternalApi>, Env, T) -> Result<Response, Error>,
    env_ptr: *mut c_void,
    msg_ptr: *mut c_void,
) -> *mut c_void {
    match _do_init(init_fn, env_ptr, msg_ptr) {
        Ok(res) => res,
        Err(err) => make_error_c_string(err),
    }
}

/// do_handle should be wrapped in an external "C" export, containing a contract-specific function as arg
pub fn do_handle<T: DeserializeOwned + JsonSchema>(
    handle_fn: &dyn Fn(
        &mut Extern<ExternalStorage, ExternalApi>,
        Env,
        T,
    ) -> Result<Response, Error>,
    env_ptr: *mut c_void,
    msg_ptr: *mut c_void,
) -> *mut c_void {
    match _do_handle(handle_fn, env_ptr, msg_ptr) {
        Ok(res) => res,
        Err(err) => make_error_c_string(err),
    }
}

/// do_query should be wrapped in an external "C" export, containing a contract-specific function as arg
pub fn do_query<T: DeserializeOwned + JsonSchema>(
    query_fn: &dyn Fn(&Extern<ExternalStorage, ExternalApi>, T) -> Result<Vec<u8>, Error>,
    msg_ptr: *mut c_void,
) -> *mut c_void {
    match _do_query(query_fn, msg_ptr) {
        Ok(res) => res,
        Err(err) => make_query_error_c_string(err),
    }
}

fn _do_init<T: DeserializeOwned + JsonSchema>(
    init_fn: &dyn Fn(&mut Extern<ExternalStorage, ExternalApi>, Env, T) -> Result<Response, Error>,
    env_ptr: *mut c_void,
    msg_ptr: *mut c_void,
) -> Result<*mut c_void, Error> {
    let env: Vec<u8> = unsafe { consume_region(env_ptr)? };
    let msg: Vec<u8> = unsafe { consume_region(msg_ptr)? };
    let env: Env = from_slice(&env).context(ParseErr { kind: "Env" })?;
    let msg: T = from_slice(&msg).context(ParseErr { kind: "InitMsg" })?;
    let mut deps = dependencies();
    let res = init_fn(&mut deps, env, msg)?;
    let json = to_vec(&ContractResult::Ok(res)).context(SerializeErr {
        kind: "ContractResult",
    })?;
    Ok(release_buffer(json))
}

fn _do_handle<T: DeserializeOwned + JsonSchema>(
    handle_fn: &dyn Fn(
        &mut Extern<ExternalStorage, ExternalApi>,
        Env,
        T,
    ) -> Result<Response, Error>,
    env_ptr: *mut c_void,
    msg_ptr: *mut c_void,
) -> Result<*mut c_void, Error> {
    let env: Vec<u8> = unsafe { consume_region(env_ptr)? };
    let msg: Vec<u8> = unsafe { consume_region(msg_ptr)? };

    let env: Env = from_slice(&env).context(ParseErr { kind: "Env" })?;
    let msg: T = from_slice(&msg).context(ParseErr { kind: "HandleMsg" })?;
    let mut deps = dependencies();
    let res = handle_fn(&mut deps, env, msg)?;
    let json = to_vec(&ContractResult::Ok(res)).context(SerializeErr {
        kind: "ContractResult",
    })?;
    Ok(release_buffer(json))
}

fn _do_query<T: DeserializeOwned + JsonSchema>(
    query_fn: &dyn Fn(&Extern<ExternalStorage, ExternalApi>, T) -> Result<Vec<u8>, Error>,
    msg_ptr: *mut c_void,
) -> Result<*mut c_void, Error> {
    let msg: Vec<u8> = unsafe { consume_region(msg_ptr)? };

    let msg: T = from_slice(&msg).context(ParseErr { kind: "QueryMsg" })?;
    let deps = dependencies();
    let res = Binary(query_fn(&deps, msg)?);
    let json = to_vec(&QueryResult::Ok(res)).context(SerializeErr {
        kind: "QueryResult",
    })?;
    Ok(release_buffer(json))
}

fn make_error_c_string<T: Display>(error: T) -> *mut c_void {
    let v = to_vec(&ContractResult::Err(error.to_string())).unwrap();
    release_buffer(v)
}

fn make_query_error_c_string<T: Display>(error: T) -> *mut c_void {
    let v = to_vec(&QueryResult::Err(error.to_string())).unwrap();
    release_buffer(v)
}
