use std::ffi::c_void;
use std::vec::Vec;

use crate::encoding::Binary;
use crate::errors::{generic_err, StdResult, SystemError};
#[cfg(feature = "iterator")]
use crate::iterator::{Order, KV};
use crate::memory::{alloc, build_region, consume_region, Region};
use crate::serde::from_slice;
use crate::traits::{Api, Querier, QuerierResult, ReadonlyStorage, Storage};
use crate::types::{CanonicalAddr, HumanAddr};

/// A kibi (kilo binary)
static KI: usize = 1024;
/// The number of bytes of the memory region we pre-allocate for the result data in ExternalIterator.next
#[cfg(feature = "iterator")]
static DB_READ_KEY_BUFFER_LENGTH: usize = 64 * KI;
/// The number of bytes of the memory region we pre-allocate for the result data in ExternalIterator.next
#[cfg(feature = "iterator")]
static DB_READ_VALUE_BUFFER_LENGTH: usize = 128 * KI;
/// The number of bytes of the memory region we pre-allocate for the result data in queries
static QUERY_RESULT_BUFFER_LENGTH: usize = 128 * KI;
// this is the maximum allowed size for bech32
// https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki#bech32
static ADDR_BUFFER_LENGTH: usize = 90;

// This interface will compile into required Wasm imports.
// A complete documentation those functions is available in the VM that provides them:
// https://github.com/confio/cosmwasm/blob/0.7/lib/vm/src/instance.rs#L43
extern "C" {
    fn db_read(key: *const c_void) -> u32;
    fn db_write(key: *const c_void, value: *mut c_void) -> i32;
    fn db_remove(key: *const c_void) -> i32;

    // scan creates an iterator, which can be read by consecutive next() calls
    #[cfg(feature = "iterator")]
    fn db_scan(start: *const c_void, end: *const c_void, order: i32) -> i32;
    #[cfg(feature = "iterator")]
    fn db_next(iterator_id: u32, key: *mut c_void, value: *mut c_void) -> i32;

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
    fn get(&self, key: &[u8]) -> StdResult<Option<Vec<u8>>> {
        let key = build_region(key);
        let key_ptr = &*key as *const Region as *const c_void;

        let read = unsafe { db_read(key_ptr) };
        if read == 0 {
            // key does not exist in external storage
            return Ok(None);
        }

        let value_ptr = read as *mut c_void;
        let data = unsafe { consume_region(value_ptr) }?;
        Ok(Some(data))
    }

    #[cfg(feature = "iterator")]
    fn range(
        &self,
        start: Option<&[u8]>,
        end: Option<&[u8]>,
        order: Order,
    ) -> StdResult<Box<dyn Iterator<Item = StdResult<KV>>>> {
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

        let scan_result = unsafe { db_scan(start_ptr, end_ptr, order) };
        if scan_result < 0 {
            return Err(generic_err(format!(
                "Error creating iterator (via db_scan). Error code: {}",
                scan_result
            )));
        }
        let iter = ExternalIterator {
            iterator_id: scan_result as u32, // Cast is safe since we tested for negative values above
        };
        Ok(Box::new(iter))
    }
}

impl Storage for ExternalStorage {
    fn set(&mut self, key: &[u8], value: &[u8]) -> StdResult<()> {
        // keep the boxes in scope, so we free it at the end (don't cast to pointers same line as build_region)
        let key = build_region(key);
        let key_ptr = &*key as *const Region as *const c_void;
        let mut value = build_region(value);
        let value_ptr = &mut *value as *mut Region as *mut c_void;
        let result = unsafe { db_write(key_ptr, value_ptr) };
        if result < 0 {
            return Err(generic_err(format!(
                "Error writing to database. Error code: {}",
                result
            )));
        }
        Ok(())
    }

    fn remove(&mut self, key: &[u8]) -> StdResult<()> {
        // keep the boxes in scope, so we free it at the end (don't cast to pointers same line as build_region)
        let key = build_region(key);
        let key_ptr = &*key as *const Region as *const c_void;
        let result = unsafe { db_remove(key_ptr) };
        if result < 0 {
            return Err(generic_err(format!(
                "Error deleting from database. Error code: {}",
                result
            )));
        }
        Ok(())
    }
}

#[cfg(feature = "iterator")]
/// ExternalIterator makes a call out to next.
/// We use the pointer to differentiate between multiple open iterators.
struct ExternalIterator {
    iterator_id: u32,
}

#[cfg(feature = "iterator")]
impl Iterator for ExternalIterator {
    type Item = StdResult<KV>;

    fn next(&mut self) -> Option<Self::Item> {
        let key_ptr = alloc(DB_READ_KEY_BUFFER_LENGTH);
        let value_ptr = alloc(DB_READ_VALUE_BUFFER_LENGTH);

        let db_next_result = unsafe { db_next(self.iterator_id, key_ptr, value_ptr) };
        if db_next_result < 0 {
            let result = Err(generic_err(format!(
                "Unknown error from db_next: {}",
                db_next_result
            )));
            return Some(result);
        }

        let key = unsafe { consume_region(key_ptr).unwrap() };
        let value = unsafe { consume_region(value_ptr).unwrap() };
        if key.is_empty() {
            return None;
        }
        Some(Ok((key, value)))
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
    fn canonical_address(&self, human: &HumanAddr) -> StdResult<CanonicalAddr> {
        let send = build_region(human.as_str().as_bytes());
        let send_ptr = &*send as *const Region as *const c_void;
        let canon = alloc(ADDR_BUFFER_LENGTH);

        let read = unsafe { canonicalize_address(send_ptr, canon) };
        if read < 0 {
            return Err(generic_err("canonicalize_address returned error"));
        }

        let out = unsafe { consume_region(canon)? };
        Ok(CanonicalAddr(Binary(out)))
    }

    fn human_address(&self, canonical: &CanonicalAddr) -> StdResult<HumanAddr> {
        let send = build_region(canonical.as_slice());
        let send_ptr = &*send as *const Region as *const c_void;
        let human = alloc(ADDR_BUFFER_LENGTH);

        let read = unsafe { humanize_address(send_ptr, human) };
        if read < 0 {
            return Err(generic_err("humanize_address returned error"));
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
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let req = build_region(bin_request);
        let request_ptr = &*req as *const Region as *const c_void;
        let response_ptr = alloc(QUERY_RESULT_BUFFER_LENGTH);

        let result_code = unsafe { query_chain(request_ptr, response_ptr) };
        if result_code < 0 {
            return Err(SystemError::Unknown {});
        }

        let process = |region_ptr| -> StdResult<QuerierResult> {
            let out = unsafe { consume_region(region_ptr)? };
            let parsed: QuerierResult = from_slice(&out)?;
            Ok(parsed)
        };

        match process(response_ptr) {
            Ok(api_response) => api_response,
            Err(err) => Ok(Err(err)),
        }
    }
}
