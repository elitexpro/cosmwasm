//! Internal details to be used by instance.rs only
#[cfg(feature = "iterator")]
use std::collections::HashMap;
#[cfg(feature = "iterator")]
use std::convert::TryInto;
use std::ffi::c_void;
#[cfg(not(feature = "iterator"))]
use std::marker::PhantomData;

use wasmer_runtime_core::vm::Ctx;

#[cfg(feature = "iterator")]
use cosmwasm_std::KV;

#[cfg(feature = "iterator")]
use crate::errors::{make_iterator_does_not_exist, FfiResult};
use crate::errors::{make_uninitialized_context_data, VmResult};
use crate::traits::{Querier, Storage};

/** context data **/

struct ContextData<'a, S: Storage, Q: Querier> {
    storage: Option<S>,
    storage_readonly: bool,
    querier: Option<Q>,
    #[cfg(feature = "iterator")]
    iterators: HashMap<u32, Box<dyn Iterator<Item = FfiResult<KV>> + 'a>>,
    #[cfg(not(feature = "iterator"))]
    iterators: PhantomData<&'a mut ()>,
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
        storage_readonly: false, // TODO: Change this default to true in 0.9 for extra safety
        querier: None,
        #[cfg(feature = "iterator")]
        iterators: HashMap::new(),
        #[cfg(not(feature = "iterator"))]
        iterators: PhantomData::default(),
    };
    let heap_data = Box::new(data); // move from stack to heap
    Box::into_raw(heap_data) as *mut c_void // give up ownership
}

fn destroy_unmanaged_context_data<S: Storage, Q: Querier>(ptr: *mut c_void) {
    if !ptr.is_null() {
        // obtain ownership and drop instance of ContextData when box gets out of scope
        let mut dying = unsafe { Box::from_raw(ptr as *mut ContextData<S, Q>) };
        // Ensure all iterators are dropped before the storage
        destroy_iterators(&mut dying);
    }
}

/// Get a mutable reference to the context's data. Ownership remains in the Context.
// NOTE: This is actually not really implemented safely at the moment. I did this as a
// nicer and less-terrible version of the previous solution to the following issue:
//
//                                                   +--->> Go pointer
//                                                   |
// Ctx ->> ContextData +-> iterators: Box<dyn Iterator + 'a> --+
//                     |                                       |
//                     +-> storage: impl Storage <<------------+
//                     |
//                     +-> querier: impl Querier
//
// ->  : Ownership
// ->> : Mutable borrow
//
// As you can see, there's a cyclical reference here... changing this function to return the same lifetime as it
// returns (and adjusting a few other functions to only have one lifetime instead of two) triggers an error
// elsewhere where we try to add iterators to the context. That's not legal according to Rust's rules, and it
// complains that we're trying to borrow ctx mutably twice. This needs a better solution because this function
// probably triggers unsoundness.
fn get_context_data_mut<'a, 'b, S: Storage, Q: Querier>(
    ctx: &'a mut Ctx,
) -> &'b mut ContextData<'b, S, Q> {
    let owned = unsafe {
        Box::from_raw(ctx.data as *mut ContextData<S, Q>) // obtain ownership
    };
    Box::leak(owned) // give up ownership
}

#[cfg(feature = "iterator")]
fn destroy_iterators<S: Storage, Q: Querier>(context: &mut ContextData<S, Q>) {
    context.iterators.clear();
}

#[cfg(not(feature = "iterator"))]
fn destroy_iterators<S: Storage, Q: Querier>(_context: &mut ContextData<S, Q>) {}

/// Returns the original storage and querier as owned instances, and closes any remaining
/// iterators. This is meant to be called when recycling the instance.
pub(crate) fn move_out_of_context<S: Storage, Q: Querier>(
    source: &mut Ctx,
) -> (Option<S>, Option<Q>) {
    let mut b = get_context_data_mut::<S, Q>(source);
    // Destroy all existing iterators which are (in contrast to the storage)
    // not reused between different instances.
    // This is also important because the iterators are pointers to Go memory which should not be stored long term
    // Paragraphs 5-7: https://golang.org/cmd/cgo/#hdr-Passing_pointers
    destroy_iterators(&mut b);
    (b.storage.take(), b.querier.take())
}

/// Moves owned instances of storage and querier into the context.
/// Should be followed by exactly one call to move_out_of_context when the instance is finished.
pub(crate) fn move_into_context<S: Storage, Q: Querier>(target: &mut Ctx, storage: S, querier: Q) {
    let b = get_context_data_mut::<S, Q>(target);
    b.storage = Some(storage);
    b.querier = Some(querier);
}

/// Returns true iff the storage is set to readonly mode
pub fn is_storage_readonly<S: Storage, Q: Querier>(ctx: &mut Ctx) -> bool {
    let context_data = get_context_data_mut::<S, Q>(ctx);
    context_data.storage_readonly
}

pub fn set_storage_readonly<S: Storage, Q: Querier>(ctx: &mut Ctx, new_value: bool) {
    let mut context_data = get_context_data_mut::<S, Q>(ctx);
    context_data.storage_readonly = new_value;
}

/// Add the iterator to the context's data. A new ID is assigned and returned.
/// IDs are guaranteed to be in the range [0, 2**31-1], i.e. fit in the non-negative part if type i32.
#[cfg(feature = "iterator")]
#[must_use = "without the returned iterator ID, the iterator cannot be accessed"]
pub fn add_iterator<'a, S: Storage, Q: Querier>(
    ctx: &mut Ctx,
    iter: Box<dyn Iterator<Item = FfiResult<KV>> + 'a>,
) -> u32 {
    let b = get_context_data_mut::<S, Q>(ctx);
    let last_id: u32 = b
        .iterators
        .len()
        .try_into()
        .expect("Found more iterator IDs than supported");
    let new_id = last_id + 1;
    static INT32_MAX_VALUE: u32 = 2_147_483_647;
    if new_id > INT32_MAX_VALUE {
        panic!("Iterator ID exceeded INT32_MAX_VALUE. This must not happen.");
    }
    b.iterators.insert(new_id, iter);
    new_id
}

pub(crate) fn with_storage_from_context<'a, 'b, S, Q, F, T>(
    ctx: &'a mut Ctx,
    func: F,
) -> VmResult<T>
where
    S: Storage,
    Q: Querier,
    F: FnOnce(&'b mut S) -> VmResult<T>,
{
    let b = get_context_data_mut::<S, Q>(ctx);
    match b.storage.as_mut() {
        Some(data) => func(data),
        None => Err(make_uninitialized_context_data("storage")),
    }
}

pub(crate) fn with_querier_from_context<'a, 'b, S, Q, F, T>(
    ctx: &'a mut Ctx,
    func: F,
) -> VmResult<T>
where
    S: Storage,
    Q: Querier,
    F: FnOnce(&'b mut Q) -> VmResult<T>,
{
    let b = get_context_data_mut::<S, Q>(ctx);
    match b.querier.as_mut() {
        Some(q) => func(q),
        None => Err(make_uninitialized_context_data("querier")),
    }
}

#[cfg(feature = "iterator")]
pub(crate) fn with_iterator_from_context<'a, 'b, S, Q, F, T>(
    ctx: &'a mut Ctx,
    iterator_id: u32,
    func: F,
) -> VmResult<T>
where
    S: Storage,
    Q: Querier,
    F: FnOnce(&'b mut (dyn Iterator<Item = FfiResult<KV>>)) -> VmResult<T>,
{
    let b = get_context_data_mut::<S, Q>(ctx);
    match b.iterators.get_mut(&iterator_id) {
        Some(iterator) => func(iterator),
        None => Err(make_iterator_does_not_exist(iterator_id)),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::backends::compile;
    #[cfg(feature = "iterator")]
    use crate::errors::VmError;
    use crate::testing::{MockQuerier, MockStorage};
    use crate::traits::ReadonlyStorage;
    use cosmwasm_std::{
        coins, from_binary, to_vec, AllBalanceResponse, BankQuery, HumanAddr, Never, QueryRequest,
    };
    use wasmer_runtime_core::{imports, typed_func::Func, Instance as WasmerInstance};

    static CONTRACT: &[u8] = include_bytes!("../testdata/contract.wasm");

    // shorthands for function generics below
    type MS = MockStorage;
    type MQ = MockQuerier;

    // prepared data
    static INIT_KEY: &[u8] = b"foo";
    static INIT_VALUE: &[u8] = b"bar";
    // this account has some coins
    static INIT_ADDR: &str = "someone";
    static INIT_AMOUNT: u128 = 500;
    static INIT_DENOM: &str = "TOKEN";

    fn make_instance() -> Box<WasmerInstance> {
        let module = compile(&CONTRACT).unwrap();
        // we need stubs for all required imports
        let import_obj = imports! {
            || { setup_context::<MockStorage, MockQuerier>() },
            "env" => {
                "db_read" => Func::new(|_a: i32, _b: i32| -> i32 { 0 }),
                "db_write" => Func::new(|_a: i32, _b: i32| -> i32 { 0 }),
                "db_remove" => Func::new(|_a: i32| -> i32 { 0 }),
                "db_scan" => Func::new(|_a: i32, _b: i32, _c: i32| -> i32 { 0 }),
                "db_next" => Func::new(|_a: u32, _b: i32, _c: i32| -> i32 { 0 }),
                "query_chain" => Func::new(|_a: i32, _b: i32| -> i32 { 0 }),
                "canonicalize_address" => Func::new(|_a: i32, _b: i32| -> i32 { 0 }),
                "humanize_address" => Func::new(|_a: i32, _b: i32| -> i32 { 0 }),
            },
        };
        let instance = Box::from(module.instantiate(&import_obj).unwrap());
        instance
    }

    fn leave_default_data(ctx: &mut Ctx) {
        // create some mock data
        let mut storage = MockStorage::new();
        storage
            .set(INIT_KEY, INIT_VALUE)
            .expect("error setting value");
        let querier =
            MockQuerier::new(&[(&HumanAddr::from(INIT_ADDR), &coins(INIT_AMOUNT, INIT_DENOM))]);
        move_into_context(ctx, storage, querier);
    }

    #[test]
    fn leave_and_take_context_data() {
        // this creates an instance
        let mut instance = make_instance();
        let ctx = instance.context_mut();

        // empty data on start
        let (inits, initq) = move_out_of_context::<MS, MQ>(ctx);
        assert!(inits.is_none());
        assert!(initq.is_none());

        // store it on the instance
        leave_default_data(ctx);
        let (s, q) = move_out_of_context::<MS, MQ>(ctx);
        assert!(s.is_some());
        assert!(q.is_some());
        assert_eq!(s.unwrap().get(INIT_KEY).unwrap(), Some(INIT_VALUE.to_vec()));

        // now is empty again
        let (ends, endq) = move_out_of_context::<MS, MQ>(ctx);
        assert!(ends.is_none());
        assert!(endq.is_none());
    }

    #[test]
    fn is_storage_readonly_defaults_to_false() {
        let mut instance = make_instance();
        let ctx = instance.context_mut();
        leave_default_data(ctx);

        assert_eq!(is_storage_readonly::<MS, MQ>(ctx), false);
    }

    #[test]
    fn set_storage_readonly_can_change_flag() {
        let mut instance = make_instance();
        let ctx = instance.context_mut();
        leave_default_data(ctx);

        // change
        set_storage_readonly::<MS, MQ>(ctx, true);
        assert_eq!(is_storage_readonly::<MS, MQ>(ctx), true);

        // still true
        set_storage_readonly::<MS, MQ>(ctx, true);
        assert_eq!(is_storage_readonly::<MS, MQ>(ctx), true);

        // change back
        set_storage_readonly::<MS, MQ>(ctx, false);
        assert_eq!(is_storage_readonly::<MS, MQ>(ctx), false);
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn add_iterator_works() {
        let mut instance = make_instance();
        let ctx = instance.context_mut();
        leave_default_data(ctx);

        assert_eq!(get_context_data_mut::<MS, MQ>(ctx).iterators.len(), 0);
        let id1 = add_iterator::<MS, MQ>(ctx, Box::new(std::iter::empty()));
        let id2 = add_iterator::<MS, MQ>(ctx, Box::new(std::iter::empty()));
        let id3 = add_iterator::<MS, MQ>(ctx, Box::new(std::iter::empty()));
        assert_eq!(get_context_data_mut::<MS, MQ>(ctx).iterators.len(), 3);
        assert!(get_context_data_mut::<MS, MQ>(ctx)
            .iterators
            .contains_key(&id1));
        assert!(get_context_data_mut::<MS, MQ>(ctx)
            .iterators
            .contains_key(&id2));
        assert!(get_context_data_mut::<MS, MQ>(ctx)
            .iterators
            .contains_key(&id3));
    }

    #[test]
    fn with_storage_from_context_set_get() {
        let mut instance = make_instance();
        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let val = with_storage_from_context::<MS, MQ, _, _>(ctx, |store| {
            Ok(store.get(INIT_KEY).expect("error getting value"))
        })
        .unwrap();
        assert_eq!(val, Some(INIT_VALUE.to_vec()));

        let set_key: &[u8] = b"more";
        let set_value: &[u8] = b"data";

        with_storage_from_context::<MS, MQ, _, _>(ctx, |store| {
            store.set(set_key, set_value).expect("error setting value");
            Ok(())
        })
        .unwrap();

        with_storage_from_context::<MS, MQ, _, _>(ctx, |store| {
            assert_eq!(store.get(INIT_KEY).unwrap(), Some(INIT_VALUE.to_vec()));
            assert_eq!(store.get(set_key).unwrap(), Some(set_value.to_vec()));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "A panic occurred in the callback.")]
    fn with_storage_from_context_handles_panics() {
        let mut instance = make_instance();
        let ctx = instance.context_mut();
        leave_default_data(ctx);

        with_storage_from_context::<MS, MQ, _, ()>(ctx, |_store| {
            panic!("A panic occurred in the callback.")
        })
        .unwrap();
    }

    #[test]
    fn with_querier_from_context_works() {
        let mut instance = make_instance();
        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let res = with_querier_from_context::<MS, MQ, _, _>(ctx, |querier| {
            let req: QueryRequest<Never> = QueryRequest::Bank(BankQuery::AllBalances {
                address: HumanAddr::from(INIT_ADDR),
            });
            Ok(querier.raw_query(&to_vec(&req).unwrap())?)
        })
        .unwrap()
        .unwrap()
        .unwrap();
        let balance: AllBalanceResponse = from_binary(&res).unwrap();

        assert_eq!(balance.amount, coins(INIT_AMOUNT, INIT_DENOM));
    }

    #[test]
    #[should_panic(expected = "A panic occurred in the callback.")]
    fn with_querier_from_context_handles_panics() {
        let mut instance = make_instance();
        let ctx = instance.context_mut();
        leave_default_data(ctx);

        with_querier_from_context::<MS, MQ, _, ()>(ctx, |_querier| {
            panic!("A panic occurred in the callback.")
        })
        .unwrap();
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn with_iterator_from_context_works() {
        let mut instance = make_instance();
        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let id = add_iterator::<MS, MQ>(ctx, Box::new(std::iter::empty()));
        with_iterator_from_context::<MS, MQ, _, ()>(ctx, id, |iter| {
            assert!(iter.next().is_none());
            Ok(())
        })
        .expect("must not error");
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn with_iterator_from_context_errors_for_non_existent_iterator_id() {
        let mut instance = make_instance();
        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let missing_id = 42u32;
        let result = with_iterator_from_context::<MS, MQ, _, ()>(ctx, missing_id, |_iter| {
            panic!("this should not be called");
        });
        match result.unwrap_err() {
            VmError::IteratorDoesNotExist { id, .. } => assert_eq!(id, missing_id),
            e => panic!("Unexpected error: {}", e),
        }
    }
}
