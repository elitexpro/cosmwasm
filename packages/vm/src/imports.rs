//! Import implementations

#[cfg(feature = "iterator")]
use std::convert::TryInto;

#[cfg(feature = "iterator")]
use cosmwasm_std::Order;
use cosmwasm_std::{Binary, CanonicalAddr, HumanAddr};
use wasmer_runtime_core::vm::Ctx;

#[cfg(feature = "iterator")]
use crate::context::{add_iterator, with_iterator_from_context};
use crate::context::{
    is_storage_readonly, process_gas_info, with_func_from_context, with_querier_from_context,
    with_storage_from_context,
};
use crate::conversion::to_u32;
use crate::errors::{CommunicationError, VmError, VmResult};
#[cfg(feature = "iterator")]
use crate::memory::maybe_read_region;
use crate::memory::{read_region, write_region};
use crate::serde::to_vec;
use crate::traits::{Api, Querier, Storage};

/// A kibi (kilo binary)
const KI: usize = 1024;
/// Max key length for db_write (i.e. when VM reads from Wasm memory)
const MAX_LENGTH_DB_KEY: usize = 64 * KI;
/// Max key length for db_write (i.e. when VM reads from Wasm memory)
const MAX_LENGTH_DB_VALUE: usize = 128 * KI;
/// Typically 20 (Cosmos SDK, Ethereum) or 32 (Nano, Substrate)
const MAX_LENGTH_CANONICAL_ADDRESS: usize = 32;
/// The maximum allowed size for bech32 (https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki#bech32)
const MAX_LENGTH_HUMAN_ADDRESS: usize = 90;
const MAX_LENGTH_QUERY_CHAIN_REQUEST: usize = 64 * KI;

/// Reads a storage entry from the VM's storage into Wasm memory
pub fn do_read<S: Storage, Q: Querier>(ctx: &mut Ctx, key_ptr: u32) -> VmResult<u32> {
    let key = read_region(ctx, key_ptr, MAX_LENGTH_DB_KEY)?;
    // `Ok(expr?)` used to convert the error variant.
    let (value, gas_info) =
        with_storage_from_context::<S, Q, _, _>(ctx, |store| Ok(store.get(&key)?))?;
    process_gas_info::<S, Q>(ctx, gas_info)?;

    let out_data = match value {
        Some(data) => data,
        None => return Ok(0),
    };
    write_to_contract::<S, Q>(ctx, &out_data)
}

/// Writes a storage entry from Wasm memory into the VM's storage
pub fn do_write<S: Storage, Q: Querier>(
    ctx: &mut Ctx,
    key_ptr: u32,
    value_ptr: u32,
) -> VmResult<()> {
    if is_storage_readonly::<S, Q>(ctx) {
        return Err(VmError::write_access_denied());
    }

    let key = read_region(ctx, key_ptr, MAX_LENGTH_DB_KEY)?;
    let value = read_region(ctx, value_ptr, MAX_LENGTH_DB_VALUE)?;
    let (_, gas_info) =
        with_storage_from_context::<S, Q, _, _>(ctx, |store| Ok(store.set(&key, &value)?))?;
    process_gas_info::<S, Q>(ctx, gas_info)?;

    Ok(())
}

pub fn do_remove<S: Storage, Q: Querier>(ctx: &mut Ctx, key_ptr: u32) -> VmResult<()> {
    if is_storage_readonly::<S, Q>(ctx) {
        return Err(VmError::write_access_denied());
    }

    let key = read_region(ctx, key_ptr, MAX_LENGTH_DB_KEY)?;
    let (_, gas_info) =
        with_storage_from_context::<S, Q, _, _>(ctx, |store| Ok(store.remove(&key)?))?;
    process_gas_info::<S, Q>(ctx, gas_info)?;

    Ok(())
}

pub fn do_canonicalize_address<A: Api, S: Storage, Q: Querier>(
    api: A,
    ctx: &mut Ctx,
    source_ptr: u32,
    destination_ptr: u32,
) -> VmResult<u32> {
    let source_data = read_region(ctx, source_ptr, MAX_LENGTH_HUMAN_ADDRESS)?;
    if source_data.is_empty() {
        return Ok(write_to_contract::<S, Q>(ctx, b"Input is empty")?);
    }

    let source_string = match String::from_utf8(source_data) {
        Ok(s) => s,
        Err(_) => return Ok(write_to_contract::<S, Q>(ctx, b"Input is not valid UTF-8")?),
    };
    let human: HumanAddr = source_string.into();

    let (canonical, gas_info) = api.canonical_address(&human)?;
    process_gas_info::<S, Q>(ctx, gas_info)?;

    write_region(ctx, destination_ptr, canonical.as_slice())?;
    Ok(0)
}

pub fn do_humanize_address<A: Api, S: Storage, Q: Querier>(
    api: A,
    ctx: &mut Ctx,
    source_ptr: u32,
    destination_ptr: u32,
) -> VmResult<u32> {
    let canonical = Binary(read_region(ctx, source_ptr, MAX_LENGTH_CANONICAL_ADDRESS)?);

    // TODO: how to report API errors back to the contract?
    let (human, gas_info) = api.human_address(&CanonicalAddr(canonical))?;
    process_gas_info::<S, Q>(ctx, gas_info)?;

    write_region(ctx, destination_ptr, human.as_str().as_bytes())?;
    Ok(0)
}

/// Creates a Region in the contract, writes the given data to it and returns the memory location
fn write_to_contract<S: Storage, Q: Querier>(ctx: &mut Ctx, input: &[u8]) -> VmResult<u32> {
    let target_ptr = with_func_from_context::<S, Q, u32, u32, _, _>(ctx, "allocate", |allocate| {
        let out_size = to_u32(input.len())?;
        let ptr = allocate.call(out_size)?;
        if ptr == 0 {
            return Err(CommunicationError::zero_address().into());
        }
        Ok(ptr)
    })?;
    write_region(ctx, target_ptr, input)?;
    Ok(target_ptr)
}

pub fn do_query_chain<S: Storage, Q: Querier>(ctx: &mut Ctx, request_ptr: u32) -> VmResult<u32> {
    let request = read_region(ctx, request_ptr, MAX_LENGTH_QUERY_CHAIN_REQUEST)?;

    let (res, used_gas) =
        with_querier_from_context::<S, Q, _, _>(ctx, |querier| Ok(querier.raw_query(&request)?))?;
    process_gas_info::<S, Q>(ctx, used_gas)?;

    let serialized = to_vec(&res)?;
    write_to_contract::<S, Q>(ctx, &serialized)
}

#[cfg(feature = "iterator")]
pub fn do_scan<S: Storage + 'static, Q: Querier>(
    ctx: &mut Ctx,
    start_ptr: u32,
    end_ptr: u32,
    order: i32,
) -> VmResult<u32> {
    let start = maybe_read_region(ctx, start_ptr, MAX_LENGTH_DB_KEY)?;
    let end = maybe_read_region(ctx, end_ptr, MAX_LENGTH_DB_KEY)?;
    let order: Order = order
        .try_into()
        .map_err(|_| CommunicationError::invalid_order(order))?;
    let (iterator, used_gas) = with_storage_from_context::<S, Q, _, _>(ctx, |store| {
        Ok(store.range(start.as_deref(), end.as_deref(), order)?)
    })?;
    // Gas is consumed for creating an iterator if the first key in the DB has a value
    process_gas_info::<S, Q>(ctx, used_gas)?;

    let new_id = add_iterator::<S, Q>(ctx, iterator);
    Ok(new_id)
}

#[cfg(feature = "iterator")]
pub fn do_next<S: Storage, Q: Querier>(ctx: &mut Ctx, iterator_id: u32) -> VmResult<u32> {
    let item = with_iterator_from_context::<S, Q, _, _>(ctx, iterator_id, |iter| Ok(iter.next()))?;

    let (kv, used_gas) = item?;
    process_gas_info::<S, Q>(ctx, used_gas)?;

    // Empty key will later be treated as _no more element_.
    let (key, value) = kv.unwrap_or_else(|| (Vec::<u8>::new(), Vec::<u8>::new()));

    // Build value || key || keylen
    let keylen_bytes = to_u32(key.len())?.to_be_bytes();
    let mut out_data = value;
    out_data.reserve(key.len() + 4);
    out_data.extend(key);
    out_data.extend_from_slice(&keylen_bytes);

    write_to_contract::<S, Q>(ctx, &out_data)
}

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::{
        coins, from_binary, AllBalanceResponse, BankQuery, Empty, HumanAddr, QueryRequest,
        SystemError, WasmQuery,
    };
    use std::ptr::NonNull;
    use wasmer_runtime_core::{imports, typed_func::Func, Instance as WasmerInstance};

    use crate::backends::compile;
    use crate::context::{
        move_into_context, set_storage_readonly, set_wasmer_instance, setup_context,
    };
    use crate::testing::{MockApi, MockQuerier, MockStorage};
    use crate::traits::Storage;
    use crate::FfiError;

    static CONTRACT: &[u8] = include_bytes!("../testdata/contract.wasm");

    // shorthands for function generics below
    type MA = MockApi;
    type MS = MockStorage;
    type MQ = MockQuerier;

    // prepared data
    const KEY1: &[u8] = b"ant";
    const VALUE1: &[u8] = b"insect";
    const KEY2: &[u8] = b"tree";
    const VALUE2: &[u8] = b"plant";

    // this account has some coins
    const INIT_ADDR: &str = "someone";
    const INIT_AMOUNT: u128 = 500;
    const INIT_DENOM: &str = "TOKEN";

    const GAS_LIMIT: u64 = 5_000_000;

    fn make_instance() -> Box<WasmerInstance> {
        let module = compile(&CONTRACT).unwrap();
        // we need stubs for all required imports
        let import_obj = imports! {
            || { setup_context::<MockStorage, MockQuerier>(GAS_LIMIT) },
            "env" => {
                "db_read" => Func::new(|_a: u32| -> u32 { 0 }),
                "db_write" => Func::new(|_a: u32, _b: u32| {}),
                "db_remove" => Func::new(|_a: u32| {}),
                "db_scan" => Func::new(|_a: u32, _b: u32, _c: i32| -> u32 { 0 }),
                "db_next" => Func::new(|_a: u32| -> u32 { 0 }),
                "query_chain" => Func::new(|_a: u32| -> u32 { 0 }),
                "canonicalize_address" => Func::new(|_a: i32, _b: i32| -> u32 { 0 }),
                "humanize_address" => Func::new(|_a: i32, _b: i32| -> u32 { 0 }),
            },
        };
        let mut instance = Box::from(module.instantiate(&import_obj).unwrap());

        let instance_ptr = NonNull::from(instance.as_ref());
        set_wasmer_instance::<MS, MQ>(instance.context_mut(), Some(instance_ptr));
        set_storage_readonly::<MS, MQ>(instance.context_mut(), false);

        instance
    }

    fn leave_default_data(ctx: &mut Ctx) {
        // create some mock data
        let mut storage = MockStorage::new();
        storage.set(KEY1, VALUE1).expect("error setting");
        storage.set(KEY2, VALUE2).expect("error setting");
        let querier: MockQuerier<Empty> =
            MockQuerier::new(&[(&HumanAddr::from(INIT_ADDR), &coins(INIT_AMOUNT, INIT_DENOM))]);
        move_into_context(ctx, storage, querier);
    }

    fn write_data(wasmer_instance: &mut WasmerInstance, data: &[u8]) -> u32 {
        let allocate: Func<u32, u32> = wasmer_instance
            .exports
            .get("allocate")
            .expect("error getting function");
        let region_ptr = allocate
            .call(data.len() as u32)
            .expect("error calling allocate");
        write_region(wasmer_instance.context_mut(), region_ptr, data).expect("error writing");
        region_ptr
    }

    fn create_empty(wasmer_instance: &mut WasmerInstance, capacity: u32) -> u32 {
        let allocate: Func<u32, u32> = wasmer_instance
            .exports
            .get("allocate")
            .expect("error getting function");
        let region_ptr = allocate.call(capacity).expect("error calling allocate");
        region_ptr
    }

    /// A Region reader that is just good enough for the tests in this file
    fn force_read(ctx: &mut Ctx, region_ptr: u32) -> Vec<u8> {
        read_region(ctx, region_ptr, 5000).unwrap()
    }

    #[test]
    fn do_read_works() {
        let mut instance = make_instance();
        leave_default_data(instance.context_mut());

        let key_ptr = write_data(&mut instance, KEY1);
        let ctx = instance.context_mut();
        let result = do_read::<MS, MQ>(ctx, key_ptr);
        let value_ptr = result.unwrap();
        assert!(value_ptr > 0);
        assert_eq!(force_read(ctx, value_ptr as u32), VALUE1);
    }

    #[test]
    fn do_read_works_for_non_existent_key() {
        let mut instance = make_instance();
        leave_default_data(instance.context_mut());

        let key_ptr = write_data(&mut instance, b"I do not exist in storage");
        let ctx = instance.context_mut();
        let result = do_read::<MS, MQ>(ctx, key_ptr);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn do_read_fails_for_large_key() {
        let mut instance = make_instance();
        leave_default_data(instance.context_mut());

        let key_ptr = write_data(&mut instance, &vec![7u8; 300 * 1024]);
        let ctx = instance.context_mut();
        let result = do_read::<MS, MQ>(ctx, key_ptr);
        match result.unwrap_err() {
            VmError::CommunicationErr {
                source: CommunicationError::RegionLengthTooBig { length, .. },
            } => assert_eq!(length, 300 * 1024),
            e => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn do_write_works() {
        let mut instance = make_instance();

        let key_ptr = write_data(&mut instance, b"new storage key");
        let value_ptr = write_data(&mut instance, b"new value");

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        do_write::<MS, MQ>(ctx, key_ptr, value_ptr).unwrap();

        let (val, _used_gas) = with_storage_from_context::<MS, MQ, _, _>(ctx, |store| {
            Ok(store.get(b"new storage key").expect("error getting value"))
        })
        .unwrap();
        assert_eq!(val, Some(b"new value".to_vec()));
    }

    #[test]
    fn do_write_can_override() {
        let mut instance = make_instance();

        let key_ptr = write_data(&mut instance, KEY1);
        let value_ptr = write_data(&mut instance, VALUE2);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        do_write::<MS, MQ>(ctx, key_ptr, value_ptr).unwrap();

        let (val, _used_gas) = with_storage_from_context::<MS, MQ, _, _>(ctx, |store| {
            Ok(store.get(KEY1).expect("error getting value"))
        })
        .unwrap();
        assert_eq!(val, Some(VALUE2.to_vec()));
    }

    #[test]
    fn do_write_works_for_empty_value() {
        let mut instance = make_instance();

        let key_ptr = write_data(&mut instance, b"new storage key");
        let value_ptr = write_data(&mut instance, b"");

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        do_write::<MS, MQ>(ctx, key_ptr, value_ptr).unwrap();

        let (val, _used_gas) = with_storage_from_context::<MS, MQ, _, _>(ctx, |store| {
            Ok(store.get(b"new storage key").expect("error getting value"))
        })
        .unwrap();
        assert_eq!(val, Some(b"".to_vec()));
    }

    #[test]
    fn do_write_fails_for_large_key() {
        let mut instance = make_instance();

        let key_ptr = write_data(&mut instance, &vec![4u8; 300 * 1024]);
        let value_ptr = write_data(&mut instance, b"new value");

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let result = do_write::<MS, MQ>(ctx, key_ptr, value_ptr);
        match result.unwrap_err() {
            VmError::CommunicationErr {
                source:
                    CommunicationError::RegionLengthTooBig {
                        length, max_length, ..
                    },
            } => {
                assert_eq!(length, 300 * 1024);
                assert_eq!(max_length, MAX_LENGTH_DB_KEY);
            }
            err => panic!("unexpected error: {:?}", err),
        };
    }

    #[test]
    fn do_write_fails_for_large_value() {
        let mut instance = make_instance();

        let key_ptr = write_data(&mut instance, b"new storage key");
        let value_ptr = write_data(&mut instance, &vec![5u8; 300 * 1024]);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let result = do_write::<MS, MQ>(ctx, key_ptr, value_ptr);
        match result.unwrap_err() {
            VmError::CommunicationErr {
                source:
                    CommunicationError::RegionLengthTooBig {
                        length, max_length, ..
                    },
            } => {
                assert_eq!(length, 300 * 1024);
                assert_eq!(max_length, MAX_LENGTH_DB_VALUE);
            }
            err => panic!("unexpected error: {:?}", err),
        };
    }

    #[test]
    fn do_write_is_prohibited_in_readonly_contexts() {
        let mut instance = make_instance();

        let key_ptr = write_data(&mut instance, b"new storage key");
        let value_ptr = write_data(&mut instance, b"new value");

        let ctx = instance.context_mut();
        leave_default_data(ctx);
        set_storage_readonly::<MS, MQ>(ctx, true);

        let result = do_write::<MS, MQ>(ctx, key_ptr, value_ptr);
        match result.unwrap_err() {
            VmError::WriteAccessDenied { .. } => {}
            e => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn do_remove_works() {
        let mut instance = make_instance();

        let existing_key = KEY1;
        let key_ptr = write_data(&mut instance, existing_key);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        do_remove::<MS, MQ>(ctx, key_ptr).unwrap();

        let (value, _used_gas) = with_storage_from_context::<MS, MQ, _, _>(ctx, |store| {
            Ok(store.get(existing_key).expect("error getting value"))
        })
        .unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn do_remove_works_for_non_existent_key() {
        let mut instance = make_instance();

        let non_existent_key = b"I do not exist";
        let key_ptr = write_data(&mut instance, non_existent_key);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        // Note: right now we cannot differnetiate between an existent and a non-existent key
        do_remove::<MS, MQ>(ctx, key_ptr).unwrap();

        let (value, _used_gas) = with_storage_from_context::<MS, MQ, _, _>(ctx, |store| {
            Ok(store.get(non_existent_key).expect("error getting value"))
        })
        .unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn do_remove_fails_for_large_key() {
        let mut instance = make_instance();

        let key_ptr = write_data(&mut instance, &vec![26u8; 300 * 1024]);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let result = do_remove::<MS, MQ>(ctx, key_ptr);
        match result.unwrap_err() {
            VmError::CommunicationErr {
                source:
                    CommunicationError::RegionLengthTooBig {
                        length, max_length, ..
                    },
            } => {
                assert_eq!(length, 300 * 1024);
                assert_eq!(max_length, MAX_LENGTH_DB_KEY);
            }
            err => panic!("unexpected error: {:?}", err),
        };
    }

    #[test]
    fn do_remove_is_prohibited_in_readonly_contexts() {
        let mut instance = make_instance();

        let key_ptr = write_data(&mut instance, b"a storage key");

        let ctx = instance.context_mut();
        leave_default_data(ctx);
        set_storage_readonly::<MS, MQ>(ctx, true);

        let result = do_remove::<MS, MQ>(ctx, key_ptr);
        match result.unwrap_err() {
            VmError::WriteAccessDenied { .. } => {}
            e => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn do_canonicalize_address_works() {
        let mut instance = make_instance();

        let source_ptr = write_data(&mut instance, b"foo");
        let dest_ptr = create_empty(&mut instance, 8);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let api = MockApi::new(8);
        do_canonicalize_address::<MA, MS, MQ>(api, ctx, source_ptr, dest_ptr).unwrap();
        assert_eq!(force_read(ctx, dest_ptr), b"foo\0\0\0\0\0");
    }

    #[test]
    fn do_canonicalize_address_fails_for_invalid_input() {
        let mut instance = make_instance();

        let source_ptr1 = write_data(&mut instance, b"fo\x80o"); // invalid UTF-8 (fo�o)
        let source_ptr2 = write_data(&mut instance, b""); // empty
        let source_ptr3 = write_data(&mut instance, b"addressexceedingaddressspace"); // too long
        let dest_ptr = create_empty(&mut instance, 8);

        let ctx = instance.context_mut();
        leave_default_data(ctx);
        let api = MockApi::new(8);

        let res = do_canonicalize_address::<MA, MS, MQ>(api, ctx, source_ptr1, dest_ptr).unwrap();
        assert_ne!(res, 0);
        let err = String::from_utf8(force_read(ctx, res)).unwrap();
        assert_eq!(err, "Input is not valid UTF-8");

        let res = do_canonicalize_address::<MA, MS, MQ>(api, ctx, source_ptr2, dest_ptr).unwrap();
        assert_ne!(res, 0);
        let err = String::from_utf8(force_read(ctx, res)).unwrap();
        assert_eq!(err, "Input is empty");

        let result = do_canonicalize_address::<MA, MS, MQ>(api, ctx, source_ptr3, dest_ptr);
        match result.unwrap_err() {
            VmError::FfiErr {
                source: FfiError::UserErr { msg, .. },
            } => {
                assert_eq!(msg, "Invalid input: human address too long");
            }
            err => panic!("Incorrect error returned: {:?}", err),
        }
    }

    #[test]
    fn do_canonicalize_address_fails_for_large_inputs() {
        let mut instance = make_instance();

        let source_ptr = write_data(&mut instance, &vec![61; 100]);
        let dest_ptr = create_empty(&mut instance, 8);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let api = MockApi::new(8);
        let result = do_canonicalize_address::<MA, MS, MQ>(api, ctx, source_ptr, dest_ptr);
        match result.unwrap_err() {
            VmError::CommunicationErr {
                source:
                    CommunicationError::RegionLengthTooBig {
                        length, max_length, ..
                    },
            } => {
                assert_eq!(length, 100);
                assert_eq!(max_length, 90);
            }
            err => panic!("Incorrect error returned: {:?}", err),
        }
    }

    #[test]
    fn do_canonicalize_address_fails_for_small_destination_region() {
        let mut instance = make_instance();

        let source_ptr = write_data(&mut instance, b"foo");
        let dest_ptr = create_empty(&mut instance, 7);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let api = MockApi::new(8);
        let result = do_canonicalize_address::<MA, MS, MQ>(api, ctx, source_ptr, dest_ptr);
        match result.unwrap_err() {
            VmError::CommunicationErr {
                source: CommunicationError::RegionTooSmall { size, required, .. },
            } => {
                assert_eq!(size, 7);
                assert_eq!(required, 8);
            }
            err => panic!("Incorrect error returned: {:?}", err),
        }
    }

    #[test]
    fn do_humanize_address_works() {
        let mut instance = make_instance();

        let source_ptr = write_data(&mut instance, b"foo\0\0\0\0\0");
        let dest_ptr = create_empty(&mut instance, 50);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let api = MockApi::new(8);
        let error_ptr = do_humanize_address::<MA, MS, MQ>(api, ctx, source_ptr, dest_ptr).unwrap();
        assert_eq!(error_ptr, 0);
        assert_eq!(force_read(ctx, dest_ptr), b"foo");
    }

    #[test]
    fn do_humanize_address_fails_for_invalid_canonical_length() {
        let mut instance = make_instance();

        let source_ptr = write_data(&mut instance, b"foo\0\0");
        let dest_ptr = create_empty(&mut instance, 50);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let api = MockApi::new(8);
        let result = do_humanize_address::<MA, MS, MQ>(api, ctx, source_ptr, dest_ptr);
        match result.unwrap_err() {
            VmError::FfiErr {
                source: FfiError::UserErr { .. },
            } => {}
            err => panic!("Incorrect error returned: {:?}", err),
        };
    }

    #[test]
    fn do_humanize_address_fails_for_input_too_long() {
        let mut instance = make_instance();

        let source_ptr = write_data(&mut instance, &vec![61; 33]);
        let dest_ptr = create_empty(&mut instance, 50);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let api = MockApi::new(8);
        let result = do_humanize_address::<MA, MS, MQ>(api, ctx, source_ptr, dest_ptr);
        match result.unwrap_err() {
            VmError::CommunicationErr {
                source:
                    CommunicationError::RegionLengthTooBig {
                        length, max_length, ..
                    },
            } => {
                assert_eq!(length, 33);
                assert_eq!(max_length, 32);
            }
            err => panic!("Incorrect error returned: {:?}", err),
        }
    }

    #[test]
    fn do_humanize_address_fails_for_destination_region_too_small() {
        let mut instance = make_instance();

        let source_ptr = write_data(&mut instance, b"foo\0\0\0\0\0");
        let dest_ptr = create_empty(&mut instance, 2);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let api = MockApi::new(8);
        let result = do_humanize_address::<MA, MS, MQ>(api, ctx, source_ptr, dest_ptr);
        match result.unwrap_err() {
            VmError::CommunicationErr {
                source: CommunicationError::RegionTooSmall { size, required, .. },
            } => {
                assert_eq!(size, 2);
                assert_eq!(required, 3);
            }
            err => panic!("Incorrect error returned: {:?}", err),
        }
    }

    #[test]
    fn do_query_chain_works() {
        let mut instance = make_instance();

        let request: QueryRequest<Empty> = QueryRequest::Bank(BankQuery::AllBalances {
            address: HumanAddr::from(INIT_ADDR),
        });
        let request_data = cosmwasm_std::to_vec(&request).unwrap();
        let request_ptr = write_data(&mut instance, &request_data);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let response_ptr = do_query_chain::<MS, MQ>(ctx, request_ptr).unwrap();
        let response = force_read(ctx, response_ptr);

        let query_result: cosmwasm_std::QuerierResult =
            cosmwasm_std::from_slice(&response).unwrap();
        let query_result_inner = query_result.unwrap();
        let query_result_inner_inner = query_result_inner.unwrap();
        let parsed_again: AllBalanceResponse = from_binary(&query_result_inner_inner).unwrap();
        assert_eq!(parsed_again.amount, coins(INIT_AMOUNT, INIT_DENOM));
    }

    #[test]
    fn do_query_chain_fails_for_broken_request() {
        let mut instance = make_instance();

        let request = b"Not valid JSON for sure";
        let request_ptr = write_data(&mut instance, request);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let response_ptr = do_query_chain::<MS, MQ>(ctx, request_ptr).unwrap();
        let response = force_read(ctx, response_ptr);

        let query_result: cosmwasm_std::QuerierResult =
            cosmwasm_std::from_slice(&response).unwrap();
        match query_result {
            Ok(_) => panic!("This must not succeed"),
            Err(SystemError::InvalidRequest { request: err, .. }) => {
                assert_eq!(err.as_slice(), request)
            }
            Err(error) => panic!("Unexpeted error: {:?}", error),
        }
    }

    #[test]
    fn do_query_chain_fails_for_missing_contract() {
        let mut instance = make_instance();

        let request: QueryRequest<Empty> = QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: HumanAddr::from("non-existent"),
            msg: Binary::from(b"{}" as &[u8]),
        });
        let request_data = cosmwasm_std::to_vec(&request).unwrap();
        let request_ptr = write_data(&mut instance, &request_data);

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let response_ptr = do_query_chain::<MS, MQ>(ctx, request_ptr).unwrap();
        let response = force_read(ctx, response_ptr);

        let query_result: cosmwasm_std::QuerierResult =
            cosmwasm_std::from_slice(&response).unwrap();
        match query_result {
            Ok(_) => panic!("This must not succeed"),
            Err(SystemError::NoSuchContract { addr }) => {
                assert_eq!(addr, HumanAddr::from("non-existent"))
            }
            Err(error) => panic!("Unexpeted error: {:?}", error),
        }
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn do_scan_unbound_works() {
        let mut instance = make_instance();
        let ctx = instance.context_mut();
        leave_default_data(ctx);

        // set up iterator over all space
        let id = do_scan::<MS, MQ>(ctx, 0, 0, Order::Ascending.into()).unwrap();
        assert_eq!(1, id);

        let item =
            with_iterator_from_context::<MS, MQ, _, _>(ctx, id, |iter| Ok(iter.next())).unwrap();
        assert_eq!(item.unwrap().0.unwrap(), (KEY1.to_vec(), VALUE1.to_vec()));

        let item =
            with_iterator_from_context::<MS, MQ, _, _>(ctx, id, |iter| Ok(iter.next())).unwrap();
        assert_eq!(item.unwrap().0.unwrap(), (KEY2.to_vec(), VALUE2.to_vec()));

        let item =
            with_iterator_from_context::<MS, MQ, _, _>(ctx, id, |iter| Ok(iter.next())).unwrap();
        assert!(item.unwrap().0.is_none());
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn do_scan_unbound_descending_works() {
        let mut instance = make_instance();
        let ctx = instance.context_mut();
        leave_default_data(ctx);

        // set up iterator over all space
        let id = do_scan::<MS, MQ>(ctx, 0, 0, Order::Descending.into()).unwrap();
        assert_eq!(1, id);

        let item =
            with_iterator_from_context::<MS, MQ, _, _>(ctx, id, |iter| Ok(iter.next())).unwrap();
        assert_eq!(item.unwrap().0.unwrap(), (KEY2.to_vec(), VALUE2.to_vec()));

        let item =
            with_iterator_from_context::<MS, MQ, _, _>(ctx, id, |iter| Ok(iter.next())).unwrap();
        assert_eq!(item.unwrap().0.unwrap(), (KEY1.to_vec(), VALUE1.to_vec()));

        let item =
            with_iterator_from_context::<MS, MQ, _, _>(ctx, id, |iter| Ok(iter.next())).unwrap();
        assert!(item.unwrap().0.is_none());
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn do_scan_bound_works() {
        let mut instance = make_instance();

        let start = write_data(&mut instance, b"anna");
        let end = write_data(&mut instance, b"bert");

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let id = do_scan::<MS, MQ>(ctx, start, end, Order::Ascending.into()).unwrap();

        let item =
            with_iterator_from_context::<MS, MQ, _, _>(ctx, id, |iter| Ok(iter.next())).unwrap();
        assert_eq!(item.unwrap().0.unwrap(), (KEY1.to_vec(), VALUE1.to_vec()));

        let item =
            with_iterator_from_context::<MS, MQ, _, _>(ctx, id, |iter| Ok(iter.next())).unwrap();
        assert!(item.unwrap().0.is_none());
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn do_scan_multiple_iterators() {
        let mut instance = make_instance();
        let ctx = instance.context_mut();
        leave_default_data(ctx);

        // unbounded, ascending and descending
        let id1 = do_scan::<MS, MQ>(ctx, 0, 0, Order::Ascending.into()).unwrap();
        let id2 = do_scan::<MS, MQ>(ctx, 0, 0, Order::Descending.into()).unwrap();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        // first item, first iterator
        let item =
            with_iterator_from_context::<MS, MQ, _, _>(ctx, id1, |iter| Ok(iter.next())).unwrap();
        assert_eq!(item.unwrap().0.unwrap(), (KEY1.to_vec(), VALUE1.to_vec()));

        // second item, first iterator
        let item =
            with_iterator_from_context::<MS, MQ, _, _>(ctx, id1, |iter| Ok(iter.next())).unwrap();
        assert_eq!(item.unwrap().0.unwrap(), (KEY2.to_vec(), VALUE2.to_vec()));

        // first item, second iterator
        let item =
            with_iterator_from_context::<MS, MQ, _, _>(ctx, id2, |iter| Ok(iter.next())).unwrap();
        assert_eq!(item.unwrap().0.unwrap(), (KEY2.to_vec(), VALUE2.to_vec()));

        // end, first iterator
        let item =
            with_iterator_from_context::<MS, MQ, _, _>(ctx, id1, |iter| Ok(iter.next())).unwrap();
        assert!(item.unwrap().0.is_none());

        // second item, second iterator
        let item =
            with_iterator_from_context::<MS, MQ, _, _>(ctx, id2, |iter| Ok(iter.next())).unwrap();
        assert_eq!(item.unwrap().0.unwrap(), (KEY1.to_vec(), VALUE1.to_vec()));
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn do_scan_errors_for_invalid_order_value() {
        let mut instance = make_instance();
        let ctx = instance.context_mut();
        leave_default_data(ctx);

        // set up iterator over all space
        let result = do_scan::<MS, MQ>(ctx, 0, 0, 42);
        match result.unwrap_err() {
            VmError::CommunicationErr {
                source: CommunicationError::InvalidOrder { .. },
            } => {}
            e => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn do_next_works() {
        let mut instance = make_instance();

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let id = do_scan::<MS, MQ>(ctx, 0, 0, Order::Ascending.into()).unwrap();

        // Entry 1
        let kv_region_ptr = do_next::<MS, MQ>(ctx, id).unwrap();
        assert_eq!(
            force_read(ctx, kv_region_ptr),
            [VALUE1, KEY1, b"\0\0\0\x03"].concat()
        );

        // Entry 2
        let kv_region_ptr = do_next::<MS, MQ>(ctx, id).unwrap();
        assert_eq!(
            force_read(ctx, kv_region_ptr),
            [VALUE2, KEY2, b"\0\0\0\x04"].concat()
        );

        // End
        let kv_region_ptr = do_next::<MS, MQ>(ctx, id).unwrap();
        assert_eq!(force_read(ctx, kv_region_ptr), b"\0\0\0\0");
        // API makes no guarantees for value_ptr in this case
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn do_next_fails_for_non_existent_id() {
        let mut instance = make_instance();

        let ctx = instance.context_mut();
        leave_default_data(ctx);

        let non_existent_id = 42u32;
        let result = do_next::<MS, MQ>(ctx, non_existent_id);
        match result.unwrap_err() {
            VmError::IteratorDoesNotExist { id, .. } => assert_eq!(id, non_existent_id),
            e => panic!("Unexpected error: {:?}", e),
        }
    }
}
