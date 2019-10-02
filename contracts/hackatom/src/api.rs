// api.rs includes the public wasm API
// when included, this whole file should be wrapped by
// #[cfg(target_arch = "wasm32")]
use failure::Error;
use serde_json::{from_slice, to_vec};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char};

use crate::{contract, storage};
use crate::types::{ContractResult, InitParams, SendParams};

fn make_error_c_string<E: Into<Error>>(error: E) -> *mut c_char {
    let error: Error = error.into();
    CString::new(to_vec(&ContractResult::Error(error.to_string())).unwrap())
        .unwrap()
        .into_raw()
}

#[no_mangle]
pub extern "C" fn init_wrapper(params_ptr: *mut c_char) -> *mut c_char {
    let params: std::vec::Vec<u8>;

    unsafe {
        params = CStr::from_ptr(params_ptr).to_bytes().to_vec();
    }

    // Catches and formats deserialization errors
    let params: InitParams = match from_slice(&params) {
        Ok(params) => params,
        Err(e) => return make_error_c_string(e),
    };

    // Catches and formats errors from the logic
    let mut store = storage::ExternalStorage{};
    let res = match contract::init(&mut store, params) {
        Ok(msgs) => ContractResult::Msgs(msgs),
        Err(e) => return make_error_c_string(e),
    };

    // Catches and formats serialization errors
    let res = match to_vec(&res) {
        Ok(res) => res,
        Err(e) => return make_error_c_string(e),
    };

    // Catches and formats CString errors
    let res = match CString::new(res) {
        Ok(res) => res,
        Err(e) => return make_error_c_string(e),
    };

    res.into_raw()
}

#[no_mangle]
pub extern "C" fn send_wrapper(params_ptr: *mut c_char) -> *mut c_char {
    let params: std::vec::Vec<u8>;

    unsafe {
        params = CStr::from_ptr(params_ptr).to_bytes().to_vec();
    }

    // Catches and formats deserialization errors
    let params: SendParams = match from_slice(&params) {
        Ok(params) => params,
        Err(e) => return make_error_c_string(e),
    };

    // Catches and formats errors from the logic
    let mut store = storage::ExternalStorage{};
    let res = match contract::send(&mut store, params) {
        Ok(msgs) => ContractResult::Msgs(msgs),
        Err(e) => return make_error_c_string(e),
    };

    // Catches and formats serialization errors
    let res = match to_vec(&res) {
        Ok(res) => res,
        Err(e) => return make_error_c_string(e),
    };

    // Catches and formats CString errors
    let res = match CString::new(res) {
        Ok(res) => res,
        Err(e) => return make_error_c_string(e),
    };

    res.into_raw()
}
