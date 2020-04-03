/**
Internal details to be used by instance.rs only
**/
use std::ffi::c_void;
use std::mem;

use wasmer_runtime_core::vm::Ctx;

#[cfg(feature = "iterator")]
use cosmwasm_std::KV;
use cosmwasm_std::{
    Api, ApiQuerierResponse, ApiSystemError, Binary, CanonicalAddr, HumanAddr,
    Querier, QuerierResponse, QueryRequest, Storage,
};

#[cfg(feature = "iterator")]
pub use iter_support::{
    do_next, do_scan, leave_iterator, take_iterator, ERROR_NEXT_INVALID_ITERATOR, ERROR_NO_STORAGE,
    ERROR_SCAN_INVALID_ORDER,
};

use crate::errors::Error;
use crate::memory::{read_region, write_region};
use crate::serde::{from_slice, to_vec};

static MAX_LENGTH_DB_KEY: usize = 100_000;
static MAX_LENGTH_DB_VALUE: usize = 100_000;
static MAX_LENGTH_ADDRESS: usize = 200;
static MAX_LENGTH_QUERY: usize = 100_000;

/// An unknown error occurred when writing to region
static ERROR_WRITE_TO_REGION_UNKNOWN: i32 = -1_000_001;
/// Could not write to region because it is too small
static ERROR_WRITE_TO_REGION_TOO_SMALL: i32 = -1_000_002;
/// An unknown error occurred when reading region
static ERROR_READ_FROM_REGION_UNKNOWN: i32 = -1_000_101;

/// Reads a storage entry from the VM's storage into Wasm memory
pub fn do_read<S: Storage, Q: Querier>(ctx: &Ctx, key_ptr: u32, value_ptr: u32) -> i32 {
    let key = match read_region(ctx, key_ptr, MAX_LENGTH_DB_KEY) {
        Ok(data) => data,
        Err(_) => return ERROR_READ_FROM_REGION_UNKNOWN,
    };
    let mut value: Option<Vec<u8>> = None;
    with_storage_from_context::<S, Q, _>(ctx, |store| value = store.get(&key));
    match value {
        Some(buf) => match write_region(ctx, value_ptr, &buf) {
            Ok(()) => 0,
            Err(Error::RegionTooSmallErr { .. }) => ERROR_WRITE_TO_REGION_TOO_SMALL,
            Err(_) => ERROR_WRITE_TO_REGION_UNKNOWN,
        },
        None => 0,
    }
}

/// Writes a storage entry from Wasm memory into the VM's storage
pub fn do_write<S: Storage, Q: Querier>(ctx: &Ctx, key_ptr: u32, value_ptr: u32) -> i32 {
    let key = match read_region(ctx, key_ptr, MAX_LENGTH_DB_KEY) {
        Ok(data) => data,
        Err(_) => return ERROR_READ_FROM_REGION_UNKNOWN,
    };
    let value = match read_region(ctx, value_ptr, MAX_LENGTH_DB_VALUE) {
        Ok(data) => data,
        Err(_) => return ERROR_READ_FROM_REGION_UNKNOWN,
    };
    with_storage_from_context::<S, Q, _>(ctx, |store| store.set(&key, &value));
    0
}

pub fn do_remove<S: Storage, Q: Querier>(ctx: &Ctx, key_ptr: u32) -> i32 {
    let key = match read_region(ctx, key_ptr, MAX_LENGTH_DB_KEY) {
        Ok(data) => data,
        Err(_) => return ERROR_READ_FROM_REGION_UNKNOWN,
    };
    with_storage_from_context::<S, Q, _>(ctx, |store| store.remove(&key));
    0
}

pub fn do_canonical_address<A: Api>(
    api: A,
    ctx: &mut Ctx,
    human_ptr: u32,
    canonical_ptr: u32,
) -> i32 {
    let human_data = match read_region(ctx, human_ptr, MAX_LENGTH_ADDRESS) {
        Ok(data) => data,
        Err(_) => return ERROR_READ_FROM_REGION_UNKNOWN,
    };
    let human = match String::from_utf8(human_data) {
        Ok(human_str) => HumanAddr(human_str),
        Err(_) => return -2,
    };
    match api.canonical_address(&human) {
        Ok(canon) => match write_region(ctx, canonical_ptr, canon.as_slice()) {
            Ok(()) => 0,
            Err(Error::RegionTooSmallErr { .. }) => ERROR_WRITE_TO_REGION_TOO_SMALL,
            Err(_) => ERROR_WRITE_TO_REGION_UNKNOWN,
        },
        Err(_) => -1,
    }
}

pub fn do_human_address<A: Api>(api: A, ctx: &mut Ctx, canonical_ptr: u32, human_ptr: u32) -> i32 {
    let canonical = match read_region(ctx, canonical_ptr, MAX_LENGTH_ADDRESS) {
        Ok(data) => Binary(data),
        Err(_) => return ERROR_READ_FROM_REGION_UNKNOWN,
    };
    match api.human_address(&CanonicalAddr(canonical)) {
        Ok(human) => match write_region(ctx, human_ptr, human.as_str().as_bytes()) {
            Ok(()) => 0,
            Err(Error::RegionTooSmallErr { .. }) => ERROR_WRITE_TO_REGION_TOO_SMALL,
            Err(_) => ERROR_WRITE_TO_REGION_UNKNOWN,
        },
        Err(_) => -1,
    }
}

pub fn do_query_chain<A: Api, S: Storage, Q: Querier>(
    _api: A,
    ctx: &mut Ctx,
    request_ptr: u32,
    response_ptr: u32,
) -> i32 {
    let request = match read_region(ctx, request_ptr, MAX_LENGTH_QUERY) {
        Ok(data) => data,
        Err(_) => return ERROR_READ_FROM_REGION_UNKNOWN,
    };

    // default result, then try real querier callback
    let mut res: QuerierResponse = Err(ApiSystemError::InvalidRequest {
        error: "no querier registered".to_string(),
    });
    match from_slice::<QueryRequest>(&request) {
        // if we parse, try to execute the query
        Ok(parsed) => {
            with_querier_from_context::<S, Q, _>(ctx, |querier| res = querier.query(&parsed))
        }
        // otherwise, return the InvalidRequest error as ApiSystemError
        Err(err) => {
            res = Err(ApiSystemError::InvalidRequest {
                error: err.to_string(),
            })
        }
    };

    let api_res: ApiQuerierResponse = res.into();

    match to_vec(&api_res) {
        Ok(serialized) => match write_region(ctx, response_ptr, &serialized) {
            Ok(()) => 0,
            Err(Error::RegionTooSmallErr { .. }) => ERROR_WRITE_TO_REGION_TOO_SMALL,
            Err(_) => ERROR_WRITE_TO_REGION_UNKNOWN,
        },
        // TODO: other error code?
        Err(_) => -1,
    }
}

#[cfg(feature = "iterator")]
mod iter_support {
    use super::*;
    use crate::memory::maybe_read_region;
    use cosmwasm_std::{Order, KV};
    use std::convert::TryInto;

    /// Invalid Order enum value passed into scan
    pub static ERROR_SCAN_INVALID_ORDER: i32 = -2_000_001;
    // Iterator pointer not registered
    pub static ERROR_NEXT_INVALID_ITERATOR: i32 = -2_000_002;
    /// Generic error - using context with no Storage attached
    pub static ERROR_NO_STORAGE: i32 = -3_000_001;

    pub fn do_scan<S: Storage + 'static, Q: Querier>(
        ctx: &Ctx,
        start_ptr: u32,
        end_ptr: u32,
        order: i32,
    ) -> i32 {
        let start = match maybe_read_region(ctx, start_ptr, MAX_LENGTH_DB_KEY) {
            Ok(data) => data,
            Err(_) => return ERROR_READ_FROM_REGION_UNKNOWN,
        };
        let end = match maybe_read_region(ctx, end_ptr, MAX_LENGTH_DB_KEY) {
            Ok(data) => data,
            Err(_) => return ERROR_READ_FROM_REGION_UNKNOWN,
        };
        let order: Order = match order.try_into() {
            Ok(o) => o,
            Err(_) => return ERROR_SCAN_INVALID_ORDER,
        };
        let storage = take_storage::<S, Q>(ctx);
        if let Some(store) = storage {
            let iter = store.range(start.as_deref(), end.as_deref(), order);
            // Unsafe: I know the iterator will be deallocated before the storage as I control the lifetime below
            // But there is no way for the compiler to know. So... let's just lie to the compiler a little bit.
            let live_forever: Box<dyn Iterator<Item = KV> + 'static> =
                unsafe { mem::transmute(iter) };
            leave_iterator::<S, Q>(ctx, live_forever);
            leave_storage::<S, Q>(ctx, Some(store));
            0
        } else {
            ERROR_NO_STORAGE
        }
    }

    pub fn do_next<S: Storage, Q: Querier>(ctx: &Ctx, key_ptr: u32, value_ptr: u32) -> i32 {
        let mut iter = match take_iterator::<S, Q>(ctx) {
            Some(i) => i,
            None => return ERROR_NEXT_INVALID_ITERATOR,
        };
        // get next item and return iterator
        let item = iter.next();
        leave_iterator::<S, Q>(ctx, iter);

        // prepare return values
        let (key, value) = match item {
            Some(item) => item,
            None => {
                return 0;
            }
        };
        match write_region(ctx, key_ptr, &key) {
            Ok(()) => 0,
            Err(Error::RegionTooSmallErr { .. }) => return ERROR_WRITE_TO_REGION_TOO_SMALL,
            Err(_) => return ERROR_WRITE_TO_REGION_UNKNOWN,
        };
        match write_region(ctx, value_ptr, &value) {
            Ok(()) => 0,
            Err(Error::RegionTooSmallErr { .. }) => ERROR_WRITE_TO_REGION_TOO_SMALL,
            Err(_) => ERROR_WRITE_TO_REGION_UNKNOWN,
        }
    }

    // if ptr is None, find a new slot.
    // otherwise, place in slot defined by ptr (only after take)
    pub fn leave_iterator<S: Storage, Q: Querier>(ctx: &Ctx, iter: Box<dyn Iterator<Item = KV>>) {
        let mut b = unsafe { get_data::<S, Q>(ctx.data) };
        // clean up old one if there was one
        let _ = b.iter.take();
        b.iter = Some(iter);
        mem::forget(b); // we do this to avoid cleanup
    }

    pub fn take_iterator<S: Storage, Q: Querier>(
        ctx: &Ctx,
    ) -> Option<Box<dyn Iterator<Item = KV>>> {
        let mut b = unsafe { get_data::<S, Q>(ctx.data) };
        let res = b.iter.take();
        mem::forget(b); // we do this to avoid cleanup
        res
    }
}

/** context data **/

struct ContextData<S: Storage, Q: Querier> {
    data: Option<S>,
    querier: Option<Q>,
    #[cfg(feature = "iterator")]
    iter: Option<Box<dyn Iterator<Item = KV>>>,
}

pub fn setup_context<S: Storage, Q: Querier>() -> (*mut c_void, fn(*mut c_void)) {
    (
        create_unmanaged_storage::<S, Q>(),
        destroy_unmanaged_storage::<S, Q>,
    )
}

fn create_unmanaged_storage<S: Storage, Q: Querier>() -> *mut c_void {
    let data = ContextData::<S, Q> {
        data: None,
        querier: None,
        #[cfg(feature = "iterator")]
        iter: None,
    };
    let state = Box::new(data);
    Box::into_raw(state) as *mut c_void
}

unsafe fn get_data<S: Storage, Q: Querier>(ptr: *mut c_void) -> Box<ContextData<S, Q>> {
    Box::from_raw(ptr as *mut ContextData<S, Q>)
}

#[cfg(feature = "iterator")]
fn destroy_unmanaged_storage<S: Storage, Q: Querier>(ptr: *mut c_void) {
    if !ptr.is_null() {
        let mut dead = unsafe { get_data::<S, Q>(ptr) };
        // ensure the iterator is dropped before the storage
        let _ = dead.iter.take();
    }
}

#[cfg(not(feature = "iterator"))]
fn destroy_unmanaged_storage<S: Storage, Q: Querier>(ptr: *mut c_void) {
    if !ptr.is_null() {
        let _ = unsafe { get_data::<S, Q>(ptr) };
    }
}

pub fn with_storage_from_context<S: Storage, Q: Querier, F: FnMut(&mut S)>(ctx: &Ctx, mut func: F) {
    let mut storage = take_storage::<S, Q>(ctx);
    if let Some(data) = &mut storage {
        func(data);
    }
    leave_storage::<S, Q>(ctx, storage);
}

pub fn with_querier_from_context<S: Storage, Q: Querier, F: FnMut(&Q)>(ctx: &Ctx, mut func: F) {
    let b = unsafe { get_data::<S, Q>(ctx.data) };
    // we do this to avoid cleanup
    let mut b = mem::ManuallyDrop::new(b);
    let querier = b.querier.take();
    if let Some(q) = &querier {
        func(q);
    }
    b.querier = querier;
}

pub fn take_storage<S: Storage, Q: Querier>(ctx: &Ctx) -> Option<S> {
    let b = unsafe { get_data::<S, Q>(ctx.data) };
    // we do this to avoid cleanup
    let mut b = mem::ManuallyDrop::new(b);
    b.data.take()
}

pub fn leave_storage<S: Storage, Q: Querier>(ctx: &Ctx, storage: Option<S>) {
    let b = unsafe { get_data::<S, Q>(ctx.data) };
    // we do this to avoid cleanup
    let mut b = mem::ManuallyDrop::new(b);
    b.data = storage;
}

pub fn take_context_data<S: Storage, Q: Querier>(ctx: &Ctx) -> (Option<S>, Option<Q>) {
    let b = unsafe { get_data::<S, Q>(ctx.data) };
    let mut b = mem::ManuallyDrop::new(b);
    (b.data.take(), b.querier.take())
}

pub fn leave_context_data<S: Storage, Q: Querier>(
    ctx: &Ctx,
    storage: Option<S>,
    querier: Option<Q>,
) {
    let b = unsafe { get_data::<S, Q>(ctx.data) };
    let mut b = mem::ManuallyDrop::new(b); // we do this to avoid cleanup
    b.data = storage;
    b.querier = querier;
}
