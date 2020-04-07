//! Internal details to be used by instance.rs only
use std::ffi::c_void;
use std::mem;

use wasmer_runtime_core::vm::Ctx;

#[cfg(feature = "iterator")]
use cosmwasm_std::KV;
use cosmwasm_std::{
    Api, ApiQuerierResponse, ApiSystemError, Binary, CanonicalAddr, HumanAddr, Querier,
    QuerierResponse, QueryRequest, Storage,
};

#[cfg(feature = "iterator")]
pub(crate) use iter_support::{do_next, do_scan};

use crate::errors::{Error, Result, UninitializedContextData};
use crate::memory::{read_region, write_region};
use crate::serde::{from_slice, to_vec};

static MAX_LENGTH_DB_KEY: usize = 100_000;
static MAX_LENGTH_DB_VALUE: usize = 100_000;
static MAX_LENGTH_ADDRESS: usize = 200;
static MAX_LENGTH_QUERY: usize = 100_000;

static SUCCESS: i32 = 0;
/// An unknown error occurred when writing to region
static ERROR_REGION_WRITE_UNKNOWN: i32 = -1_000_001;
/// Could not write to region because it is too small
static ERROR_REGION_WRITE_TOO_SMALL: i32 = -1_000_002;
/// An unknown error occurred when reading region
static ERROR_REGION_READ_UNKNOWN: i32 = -1_000_101;
/// The contract sent us a Region we're not willing to read because it is too big
static ERROR_REGION_READ_LENGTH_TOO_BIG: i32 = -1_000_102;
/// An unknown error when canonicalizing address
static ERROR_CANONICALIZE_UNKNOWN: i32 = -1_000_201;
/// The input address (human address) was invalid
static ERROR_CANONICALIZE_INVALID_INPUT: i32 = -1_000_202;
/// An unknonw error when humanizing address
static ERROR_HUMANIZE_UNKNOWN: i32 = -1_000_301;
/// Cannot serialize query response
static ERROR_QUERY_CHAIN_CANNOT_SERIALIZE_RESPONSE: i32 = -1_000_402;
/// Generic error - using context with no Storage attached
pub static ERROR_NO_CONTEXT_DATA: i32 = -1_000_501;
/// Generic error - An unknown error accessing the DB
static ERROR_DB_UNKNOWN: i32 = -1_000_502;

/// Reads a storage entry from the VM's storage into Wasm memory
pub fn do_read<S: Storage, Q: Querier>(ctx: &Ctx, key_ptr: u32, value_ptr: u32) -> i32 {
    let key = match read_region(ctx, key_ptr, MAX_LENGTH_DB_KEY) {
        Ok(data) => data,
        Err(Error::RegionLengthTooBigErr { .. }) => return ERROR_REGION_READ_LENGTH_TOO_BIG,
        Err(_) => return ERROR_REGION_READ_UNKNOWN,
    };
    let value: Option<Vec<u8>> =
        match with_storage_from_context::<S, Q, _, _>(ctx, |store| Ok(store.get(&key))) {
            Ok(v) => v,
            Err(Error::UninitializedContextData { .. }) => return ERROR_NO_CONTEXT_DATA,
            Err(_) => return ERROR_DB_UNKNOWN,
        };
    match value {
        Some(buf) => match write_region(ctx, value_ptr, &buf) {
            Ok(()) => SUCCESS,
            Err(Error::RegionTooSmallErr { .. }) => ERROR_REGION_WRITE_TOO_SMALL,
            Err(_) => ERROR_REGION_WRITE_UNKNOWN,
        },
        None => SUCCESS,
    }
}

/// Writes a storage entry from Wasm memory into the VM's storage
pub fn do_write<S: Storage, Q: Querier>(ctx: &Ctx, key_ptr: u32, value_ptr: u32) -> i32 {
    let key = match read_region(ctx, key_ptr, MAX_LENGTH_DB_KEY) {
        Ok(data) => data,
        Err(Error::RegionLengthTooBigErr { .. }) => return ERROR_REGION_READ_LENGTH_TOO_BIG,
        Err(_) => return ERROR_REGION_READ_UNKNOWN,
    };
    let value = match read_region(ctx, value_ptr, MAX_LENGTH_DB_VALUE) {
        Ok(data) => data,
        Err(Error::RegionLengthTooBigErr { .. }) => return ERROR_REGION_READ_LENGTH_TOO_BIG,
        Err(_) => return ERROR_REGION_READ_UNKNOWN,
    };
    match with_storage_from_context::<S, Q, _, ()>(ctx, |store| {
        store.set(&key, &value);
        Ok(())
    }) {
        Ok(_) => SUCCESS,
        Err(Error::UninitializedContextData { .. }) => ERROR_NO_CONTEXT_DATA,
        Err(_) => ERROR_DB_UNKNOWN,
    }
}

pub fn do_remove<S: Storage, Q: Querier>(ctx: &Ctx, key_ptr: u32) -> i32 {
    let key = match read_region(ctx, key_ptr, MAX_LENGTH_DB_KEY) {
        Ok(data) => data,
        Err(Error::RegionLengthTooBigErr { .. }) => return ERROR_REGION_READ_LENGTH_TOO_BIG,
        Err(_) => return ERROR_REGION_READ_UNKNOWN,
    };
    match with_storage_from_context::<S, Q, _, ()>(ctx, |store| {
        store.remove(&key);
        Ok(())
    }) {
        Ok(_) => SUCCESS,
        Err(Error::UninitializedContextData { .. }) => ERROR_NO_CONTEXT_DATA,
        Err(_) => ERROR_DB_UNKNOWN,
    }
}

pub fn do_canonicalize_address<A: Api>(
    api: A,
    ctx: &mut Ctx,
    human_ptr: u32,
    canonical_ptr: u32,
) -> i32 {
    let human_data = match read_region(ctx, human_ptr, MAX_LENGTH_ADDRESS) {
        Ok(data) => data,
        Err(Error::RegionLengthTooBigErr { .. }) => return ERROR_REGION_READ_LENGTH_TOO_BIG,
        Err(_) => return ERROR_REGION_READ_UNKNOWN,
    };
    let human = match String::from_utf8(human_data) {
        Ok(human_str) => HumanAddr(human_str),
        Err(_) => return ERROR_CANONICALIZE_INVALID_INPUT,
    };
    match api.canonical_address(&human) {
        Ok(canon) => match write_region(ctx, canonical_ptr, canon.as_slice()) {
            Ok(()) => SUCCESS,
            Err(Error::RegionTooSmallErr { .. }) => ERROR_REGION_WRITE_TOO_SMALL,
            Err(_) => ERROR_REGION_WRITE_UNKNOWN,
        },
        Err(_) => ERROR_CANONICALIZE_UNKNOWN,
    }
}

pub fn do_humanize_address<A: Api>(
    api: A,
    ctx: &mut Ctx,
    canonical_ptr: u32,
    human_ptr: u32,
) -> i32 {
    let canonical = match read_region(ctx, canonical_ptr, MAX_LENGTH_ADDRESS) {
        Ok(data) => Binary(data),
        Err(Error::RegionLengthTooBigErr { .. }) => return ERROR_REGION_READ_LENGTH_TOO_BIG,
        Err(_) => return ERROR_REGION_READ_UNKNOWN,
    };
    match api.human_address(&CanonicalAddr(canonical)) {
        Ok(human) => match write_region(ctx, human_ptr, human.as_str().as_bytes()) {
            Ok(()) => SUCCESS,
            Err(Error::RegionTooSmallErr { .. }) => ERROR_REGION_WRITE_TOO_SMALL,
            Err(_) => ERROR_REGION_WRITE_UNKNOWN,
        },
        Err(_) => ERROR_HUMANIZE_UNKNOWN,
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
        Err(Error::RegionLengthTooBigErr { .. }) => return ERROR_REGION_READ_LENGTH_TOO_BIG,
        Err(_) => return ERROR_REGION_READ_UNKNOWN,
    };

    let res = match from_slice::<QueryRequest>(&request) {
        // if we parse, try to execute the query
        Ok(parsed) => {
            let qr: QuerierResponse =
                with_querier_from_context::<S, Q, _, _>(ctx, |querier: &Q| querier.query(&parsed));
            qr
        }
        // otherwise, return the InvalidRequest error as ApiSystemError
        Err(err) => Err(ApiSystemError::InvalidRequest {
            error: err.to_string(),
        }),
    };

    let api_res: ApiQuerierResponse = res.into();

    match to_vec(&api_res) {
        Ok(serialized) => match write_region(ctx, response_ptr, &serialized) {
            Ok(()) => SUCCESS,
            Err(Error::RegionTooSmallErr { .. }) => ERROR_REGION_WRITE_TOO_SMALL,
            Err(_) => ERROR_REGION_WRITE_UNKNOWN,
        },
        Err(_) => ERROR_QUERY_CHAIN_CANNOT_SERIALIZE_RESPONSE,
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

    pub fn do_scan<S: Storage + 'static, Q: Querier>(
        ctx: &Ctx,
        start_ptr: u32,
        end_ptr: u32,
        order: i32,
    ) -> i32 {
        let start = match maybe_read_region(ctx, start_ptr, MAX_LENGTH_DB_KEY) {
            Ok(data) => data,
            Err(Error::RegionLengthTooBigErr { .. }) => return ERROR_REGION_READ_LENGTH_TOO_BIG,
            Err(_) => return ERROR_REGION_READ_UNKNOWN,
        };
        let end = match maybe_read_region(ctx, end_ptr, MAX_LENGTH_DB_KEY) {
            Ok(data) => data,
            Err(Error::RegionLengthTooBigErr { .. }) => return ERROR_REGION_READ_LENGTH_TOO_BIG,
            Err(_) => return ERROR_REGION_READ_UNKNOWN,
        };
        let order: Order = match order.try_into() {
            Ok(o) => o,
            Err(_) => return ERROR_SCAN_INVALID_ORDER,
        };
        let res = with_storage_from_context::<S, Q, _, _>(ctx, |store| {
            let iter = store.range(start.as_deref(), end.as_deref(), order);
            // Unsafe: I know the iterator will be deallocated before the storage as I control the lifetime below
            // But there is no way for the compiler to know. So... let's just lie to the compiler a little bit.
            let live_forever: Box<dyn Iterator<Item = KV> + 'static> =
                unsafe { mem::transmute(iter) };
            set_iterator::<S, Q>(ctx, live_forever);
            Ok(())
        });
        match res {
            Ok(_) => SUCCESS,
            Err(_) => ERROR_NO_CONTEXT_DATA,
        }
    }

    pub fn do_next<S: Storage, Q: Querier>(ctx: &Ctx, key_ptr: u32, value_ptr: u32) -> i32 {
        let item = match with_iterator_from_context::<S, Q, _, _>(ctx, |iter| Ok(iter.next())) {
            Ok(i) => i,
            Err(Error::UninitializedContextData { .. }) => return ERROR_NO_CONTEXT_DATA,
            Err(_) => return ERROR_NEXT_INVALID_ITERATOR,
        };

        // prepare return values
        let (key, value) = match item {
            Some(item) => item,
            None => return SUCCESS, // Return early without writing key. Empty key will later be treated as _no more element_.
        };

        match write_region(ctx, key_ptr, &key) {
            Ok(()) => (),
            Err(Error::RegionTooSmallErr { .. }) => return ERROR_REGION_WRITE_TOO_SMALL,
            Err(_) => return ERROR_REGION_WRITE_UNKNOWN,
        };
        match write_region(ctx, value_ptr, &value) {
            Ok(()) => (),
            Err(Error::RegionTooSmallErr { .. }) => return ERROR_REGION_WRITE_TOO_SMALL,
            Err(_) => return ERROR_REGION_WRITE_UNKNOWN,
        };
        SUCCESS
    }

    pub(crate) fn with_iterator_from_context<S, Q, F, T>(ctx: &Ctx, mut func: F) -> Result<T, Error>
    where
        S: Storage,
        Q: Querier,
        F: FnMut(&mut dyn Iterator<Item = KV>) -> Result<T, Error>,
    {
        let b = unsafe { get_data::<S, Q>(ctx.data) };
        let mut b = mem::ManuallyDrop::new(b);
        let mut iter = b.iter.take();
        let res = match &mut iter {
            Some(data) => func(data),
            None => UninitializedContextData { kind: "iterator" }.fail(),
        };
        b.iter = iter;
        res
    }

    // set the iterator, overwriting any possible iterator previously set
    fn set_iterator<S: Storage, Q: Querier>(ctx: &Ctx, iter: Box<dyn Iterator<Item = KV>>) {
        let b = unsafe { get_data::<S, Q>(ctx.data) };
        let mut b = mem::ManuallyDrop::new(b); // we do this to avoid cleanup
        b.iter = Some(iter);
    }
}

/** context data **/

struct ContextData<S: Storage, Q: Querier> {
    storage: Option<S>,
    querier: Option<Q>,
    #[cfg(feature = "iterator")]
    iter: Option<Box<dyn Iterator<Item = KV>>>,
}

pub fn setup_context<S: Storage, Q: Querier>() -> (*mut c_void, fn(*mut c_void)) {
    (
        create_unmanaged_context_data::<S, Q>(),
        destroy_unmanaged_context_data::<S, Q>,
    )
}

fn create_unmanaged_context_data<S: Storage, Q: Querier>() -> *mut c_void {
    let data = ContextData::<S, Q> {
        storage: None,
        querier: None,
        #[cfg(feature = "iterator")]
        iter: None,
    };
    let state = Box::new(data);
    Box::into_raw(state) as *mut c_void
}

fn destroy_unmanaged_context_data<S: Storage, Q: Querier>(ptr: *mut c_void) {
    if !ptr.is_null() {
        let mut dead = unsafe { get_data::<S, Q>(ptr) };
        // ensure the iterator (if any) is dropped before the storage
        free_iterator(&mut dead);
    }
}

unsafe fn get_data<S: Storage, Q: Querier>(ptr: *mut c_void) -> Box<ContextData<S, Q>> {
    Box::from_raw(ptr as *mut ContextData<S, Q>)
}

#[cfg(feature = "iterator")]
fn free_iterator<S: Storage, Q: Querier>(context: &mut ContextData<S, Q>) {
    let _ = context.iter.take();
}

#[cfg(not(feature = "iterator"))]
fn free_iterator<S: Storage, Q: Querier>(_context: &mut ContextData<S, Q>) {}

pub(crate) fn with_storage_from_context<S, Q, F, T>(ctx: &Ctx, mut func: F) -> Result<T, Error>
where
    S: Storage,
    Q: Querier,
    F: FnMut(&mut S) -> Result<T, Error>,
{
    let b = unsafe { get_data::<S, Q>(ctx.data) };
    let mut b = mem::ManuallyDrop::new(b);
    let mut storage = b.storage.take();
    let res = match &mut storage {
        Some(data) => func(data),
        None => UninitializedContextData { kind: "storage" }.fail(),
    };
    b.storage = storage;
    res
}

pub(crate) fn with_querier_from_context<S, Q, F, T>(
    ctx: &Ctx,
    mut func: F,
) -> Result<T, ApiSystemError>
where
    S: Storage,
    Q: Querier,
    F: FnMut(&Q) -> Result<T, ApiSystemError>,
{
    let b = unsafe { get_data::<S, Q>(ctx.data) };
    // we do this to avoid cleanup
    let mut b = mem::ManuallyDrop::new(b);
    let querier = b.querier.take();
    let res = match &querier {
        Some(q) => func(q),
        None => Err(ApiSystemError::Unknown {}),
    };
    b.querier = querier;
    res
}

/// take_context_data will return the original storage and querier, and closes any remaining
/// iterators. This is meant to be called when recycling the instance
pub(crate) fn move_into_context<S: Storage, Q: Querier>(ctx: &Ctx) -> (Option<S>, Option<Q>) {
    let b = unsafe { get_data::<S, Q>(ctx.data) };
    let mut b = mem::ManuallyDrop::new(b);
    // free out the iterator as this finalizes the instance
    free_iterator(&mut b);
    (b.storage.take(), b.querier.take())
}

/// leave_context_data sets the original storage and querier. These must both be set.
/// Should be followed by exactly one call to take_context_data when the instance is finished.
pub(crate) fn move_from_context<S: Storage, Q: Querier>(ctx: &Ctx, storage: S, querier: Q) {
    let b = unsafe { get_data::<S, Q>(ctx.data) };
    let mut b = mem::ManuallyDrop::new(b); // we do this to avoid cleanup
    b.storage = Some(storage);
    b.querier = Some(querier);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::backends::compile;
    use cosmwasm_std::testing::{MockQuerier, MockStorage};
    use cosmwasm_std::{coin, from_binary, BalanceResponse, ReadonlyStorage};
    use wasmer_runtime_core::{imports, instance::Instance, typed_func::Func};

    #[cfg(feature = "iterator")]
    use super::iter_support::with_iterator_from_context;

    static CONTRACT: &[u8] = include_bytes!("../testdata/contract.wasm");

    // shorthand for function generics below
    type S = MockStorage;
    type Q = MockQuerier;

    // prepared data
    static INIT_KEY: &[u8] = b"foo";
    static INIT_VALUE: &[u8] = b"bar";
    // this account has some coins
    static INIT_ADDR: &str = "someone";
    static INIT_AMOUNT: &str = "500";
    static INIT_DENOM: &str = "TOKEN";

    fn make_instance() -> Instance {
        let module = compile(&CONTRACT).unwrap();
        // we need stubs for all required imports
        let import_obj = imports! {
            || { setup_context::<MockStorage, MockQuerier>() },
            "env" => {
                "read_db" => Func::new(|_a: i32, _b: i32| -> i32 { 0 }),
                "write_db" => Func::new(|_a: i32, _b: i32| -> i32 { 0 }),
                "remove_db" => Func::new(|_a: i32| -> i32 { 0 }),
                "scan_db" => Func::new(|_a: i32, _b: i32, _c: i32| -> i32 { 0 }),
                "next_db" => Func::new(|_a: i32, _b: i32| -> i32 { 0 }),
                "query_chain" => Func::new(|_a: i32, _b: i32| -> i32 { 0 }),
                "canonicalize_address" => Func::new(|_a: i32, _b: i32| -> i32 { 0 }),
                "humanize_address" => Func::new(|_a: i32, _b: i32| -> i32 { 0 }),
            },
        };
        let instance = module.instantiate(&import_obj).unwrap();
        instance
    }

    fn leave_default_data(instance: &Instance) {
        // create some mock data
        let mut storage = MockStorage::new();
        storage.set(INIT_KEY, INIT_VALUE);
        let querier =
            MockQuerier::new(&[(&HumanAddr::from(INIT_ADDR), &coin(INIT_AMOUNT, INIT_DENOM))]);
        move_from_context(instance.context(), storage, querier);
    }

    #[test]
    fn leave_and_take_context_data() {
        // this creates an instance
        let instance = make_instance();

        // empty data on start
        let (inits, initq) = move_into_context::<S, Q>(instance.context());
        assert!(inits.is_none());
        assert!(initq.is_none());

        // store it on the instance
        leave_default_data(&instance);
        let (s, q) = move_into_context::<S, Q>(instance.context());
        assert!(s.is_some());
        assert!(q.is_some());
        assert_eq!(s.unwrap().get(INIT_KEY), Some(INIT_VALUE.to_vec()));

        // now is empty again
        let (ends, endq) = move_into_context::<S, Q>(instance.context());
        assert!(ends.is_none());
        assert!(endq.is_none());
    }

    #[test]
    fn with_storage_set_get() {
        // this creates an instance
        let instance = make_instance();
        leave_default_data(&instance);
        let ctx = instance.context();

        let val =
            with_storage_from_context::<S, Q, _, _>(ctx, |store| Ok(store.get(INIT_KEY))).unwrap();
        assert_eq!(val, Some(INIT_VALUE.to_vec()));

        let set_key: &[u8] = b"more";
        let set_value: &[u8] = b"data";

        with_storage_from_context::<S, Q, _, _>(ctx, |store| {
            store.set(set_key, set_value);
            Ok(())
        })
        .unwrap();

        with_storage_from_context::<S, Q, _, _>(ctx, |store| {
            assert_eq!(store.get(INIT_KEY), Some(INIT_VALUE.to_vec()));
            assert_eq!(store.get(set_key), Some(set_value.to_vec()));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn with_iterator_miss_and_hit() {
        // this creates an instance
        let instance = make_instance();
        leave_default_data(&instance);
        let ctx = instance.context();

        let miss = with_iterator_from_context::<S, Q, _, ()>(ctx, |_iter| {
            panic!("this should be empty / not callled");
        });
        match miss {
            Ok(_) => panic!("Expected error"),
            Err(Error::UninitializedContextData { .. }) => assert!(true),
            Err(e) => panic!("Unexpected error: {}", e),
        }

        // set up iterator over all space
        let scan = do_scan::<S, Q>(ctx, 0, 0, cosmwasm_std::Order::Ascending.into());
        assert_eq!(0, scan);

        let item = with_iterator_from_context::<S, Q, _, _>(ctx, |iter| Ok(iter.next())).unwrap();
        assert_eq!(item, Some((INIT_KEY.to_vec(), INIT_VALUE.to_vec())));

        let item = with_iterator_from_context::<S, Q, _, _>(ctx, |iter| Ok(iter.next())).unwrap();
        assert_eq!(item, None);
    }

    #[test]
    fn with_query_success() {
        // this creates an instance
        let instance = make_instance();
        leave_default_data(&instance);
        let ctx = instance.context();

        let res = with_querier_from_context::<S, Q, _, _>(ctx, |querier| {
            let req = QueryRequest::Balance {
                address: HumanAddr::from(INIT_ADDR),
            };
            querier.query(&req)
        })
        .unwrap()
        .unwrap();
        let balance: BalanceResponse = from_binary(&res).unwrap();

        assert_eq!(balance.amount.unwrap(), coin(INIT_AMOUNT, INIT_DENOM));
    }

    #[test]
    #[should_panic]
    fn with_storage_handles_panics() {
        // this creates an instance
        let instance = make_instance();
        leave_default_data(&instance);
        let ctx = instance.context();

        with_storage_from_context::<S, Q, _, ()>(ctx, |_store| {
            panic!("fails, but shouldn't cause segfault")
        })
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn with_query_handles_panics() {
        // this creates an instance
        let instance = make_instance();
        leave_default_data(&instance);
        let ctx = instance.context();

        with_querier_from_context::<S, Q, _, ()>(ctx, |_querier| {
            panic!("fails, but shouldn't cause segfault")
        })
        .unwrap();
    }
}
