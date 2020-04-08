use std::ffi::c_void;
use std::vec::Vec;

use crate::api::{ApiResult, ApiSystemError};
use crate::encoding::Binary;
use crate::errors::{ContractErr, Result};
use crate::memory::{alloc, build_region, consume_region, Region};
use crate::query::QueryRequest;
use crate::serde::{from_slice, to_vec};
use crate::traits::{Api, Querier, QuerierResponse, ReadonlyStorage, Storage};
#[cfg(feature = "iterator")]
use crate::traits::{Order, KV};
use crate::types::{CanonicalAddr, HumanAddr};

// this is the buffer we pre-allocate in get - we should configure this somehow later
static MAX_READ: usize = 2000;

// this is the maximum allowed size for bech32
// https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki#bech32
static ADDR_BUFFER: usize = 90;

// this is the space we allocate for query responses
static QUERY_BUFFER: usize = 4000;

// This interface will compile into required Wasm imports.
// A complete documentation those functions is available in the VM that provides them:
// https://github.com/confio/cosmwasm/blob/0.7/lib/vm/src/instance.rs#L43
extern "C" {
    fn read_db(key: *const c_void, value: *mut c_void) -> i32;
    fn write_db(key: *const c_void, value: *mut c_void) -> i32;
    fn remove_db(key: *const c_void) -> i32;

    // scan creates an iterator, which can be read by consecutive next() calls
    #[cfg(feature = "iterator")]
    fn scan_db(start: *const c_void, end: *const c_void, order: i32) -> i32;
    #[cfg(feature = "iterator")]
    fn next_db(key: *mut c_void, value: *mut c_void) -> i32;

    fn canonicalize_address(human: *const c_void, canonical: *mut c_void) -> i32;
    fn humanize_address(canonical: *const c_void, human: *mut c_void) -> i32;

    // query_chain will launch a query on the chain (import)
    // different than query which will query the state of the contract (export)
    fn query_chain(request: *const c_void, response: *mut c_void) -> i32;
}

/// A stateless convenience wrapper around database imports provided by the VM.
/// This cannot be cloned as it would not copy any data. If you need to clone this, it indicates a flaw in your logic.
pub struct ExternalStorage {}

impl ExternalStorage {
    pub fn new() -> ExternalStorage {
        ExternalStorage {}
    }
}

impl ReadonlyStorage for ExternalStorage {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let key = build_region(key);
        let key_ptr = &*key as *const Region as *const c_void;
        let value = alloc(MAX_READ);

        let read = unsafe { read_db(key_ptr, value) };
        if read == -1000002 {
            panic!("Allocated memory too small to hold the database value for the given key. \
                If this is causing trouble for you, have a look at https://github.com/confio/cosmwasm/issues/126");
        } else if read < 0 {
            panic!("An unknown error occurred in the read_db call.")
        }

        match unsafe { consume_region(value) } {
            Ok(data) => {
                if data.len() == 0 {
                    None
                } else {
                    Some(data)
                }
            }
            // TODO: do we really want to convert errors to None?
            Err(_) => None,
        }
    }

    #[cfg(feature = "iterator")]
    fn range(
        &self,
        start: Option<&[u8]>,
        end: Option<&[u8]>,
        order: Order,
    ) -> Box<dyn Iterator<Item = KV>> {
        // start and end (Regions) must remain in scope as long as the start_ptr / end_ptr do
        // thus they are not inside a block
        let start = start.map(|s| build_region(s));
        let start_ptr = match start {
            Some(reg) => &*reg as *const Region as *const c_void,
            None => std::ptr::null(),
        };
        let end = end.map(|e| build_region(e));
        let end_ptr = match end {
            Some(reg) => &*reg as *const Region as *const c_void,
            None => std::ptr::null(),
        };
        let order = order as i32;

        let iter_ptr = unsafe { scan_db(start_ptr, end_ptr, order) };
        if iter_ptr < 0 {
            panic!(format!("Error creating iterator: {}", iter_ptr));
        }
        let iter = ExternalIterator {};
        Box::new(iter)
    }
}

impl Storage for ExternalStorage {
    fn set(&mut self, key: &[u8], value: &[u8]) {
        // keep the boxes in scope, so we free it at the end (don't cast to pointers same line as build_region)
        let key = build_region(key);
        let key_ptr = &*key as *const Region as *const c_void;
        let mut value = build_region(value);
        let value_ptr = &mut *value as *mut Region as *mut c_void;
        let result = unsafe { write_db(key_ptr, value_ptr) };
        if result < 0 {
            // TODO: convert to Result, but this requires changing the trait
            panic!("Error writing to database");
        }
    }

    fn remove(&mut self, key: &[u8]) {
        // keep the boxes in scope, so we free it at the end (don't cast to pointers same line as build_region)
        let key = build_region(key);
        let key_ptr = &*key as *const Region as *const c_void;
        let result = unsafe { remove_db(key_ptr) };
        if result < 0 {
            // TODO: convert to Result, but this requires changing the trait
            panic!("Error deleting from database");
        }
    }
}

#[cfg(feature = "iterator")]
/// ExternalIterator makes a call out to next
/// We only allow one open iterator at a time, so no need to pass references
/// it automatically refers to result of last range call
struct ExternalIterator {}

#[cfg(feature = "iterator")]
impl Iterator for ExternalIterator {
    type Item = KV;

    fn next(&mut self) -> Option<Self::Item> {
        let key_ptr = alloc(MAX_READ);
        let value_ptr = alloc(MAX_READ);

        let read = unsafe { next_db(key_ptr, value_ptr) };
        if read < 0 {
            panic!(format!("Unknown error on next: {}", read));
        }

        let key = unsafe { consume_region(key_ptr).unwrap() };
        let value = unsafe { consume_region(value_ptr).unwrap() };
        if key.is_empty() {
            return None;
        }
        Some((key, value))
    }
}

/// A stateless convenience wrapper around imports provided by the VM
#[derive(Copy, Clone)]
pub struct ExternalApi {}

impl ExternalApi {
    pub fn new() -> ExternalApi {
        ExternalApi {}
    }
}

impl Api for ExternalApi {
    fn canonical_address(&self, human: &HumanAddr) -> Result<CanonicalAddr> {
        let send = build_region(human.as_str().as_bytes());
        let send_ptr = &*send as *const Region as *const c_void;
        let canon = alloc(ADDR_BUFFER);

        let read = unsafe { canonicalize_address(send_ptr, canon) };
        if read < 0 {
            return ContractErr {
                msg: "canonicalize_address returned error",
            }
            .fail();
        }

        let out = unsafe { consume_region(canon)? };
        Ok(CanonicalAddr(Binary(out)))
    }

    fn human_address(&self, canonical: &CanonicalAddr) -> Result<HumanAddr> {
        let send = build_region(canonical.as_slice());
        let send_ptr = &*send as *const Region as *const c_void;
        let human = alloc(ADDR_BUFFER);

        let read = unsafe { humanize_address(send_ptr, human) };
        if read < 0 {
            return ContractErr {
                msg: "humanize_address returned error",
            }
            .fail();
        }

        let out = unsafe { consume_region(human)? };
        // we know input was correct when created, so let's save some bytes
        let result = unsafe { String::from_utf8_unchecked(out) };
        Ok(HumanAddr(result))
    }
}

/// A stateless convenience wrapper around imports provided by the VM
#[derive(Copy, Clone)]
pub struct ExternalQuerier {}

impl ExternalQuerier {
    pub fn new() -> ExternalQuerier {
        ExternalQuerier {}
    }
}

impl Querier for ExternalQuerier {
    fn query(&self, request: &QueryRequest) -> QuerierResponse {
        let bin_request = to_vec(request).or(Err(ApiSystemError::Unknown {}))?;
        let req = build_region(&bin_request);
        let req_ptr = &*req as *const Region as *const c_void;
        let resp = alloc(QUERY_BUFFER);

        let ret = unsafe { query_chain(req_ptr, resp) };
        if ret < 0 {
            return Err(ApiSystemError::Unknown {});
        }

        let parse = |r| -> Result<QuerierResponse> {
            let out = unsafe { consume_region(r)? };
            let parsed: ApiResult<ApiResult<Binary>, ApiSystemError> = from_slice(&out)?;
            Ok(parsed.into())
        };

        match parse(resp) {
            Ok(api_response) => api_response,
            Err(err) => Ok(Err(err.into())),
        }
    }
}
