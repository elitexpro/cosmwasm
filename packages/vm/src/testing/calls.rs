//! This file has some helpers for integration tests.
//! They should be imported via full path to ensure there is no confusion
//! use cosmwasm_vm::testing::X
use schemars::JsonSchema;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt;

use cosmwasm_std::{ContractResult, Env, MessageInfo, QueryResponse, Response, SubCallResult};

use crate::calls::{
    call_handle, call_init, call_migrate, call_query, call_subcall_response, call_system,
};
use crate::instance::Instance;
use crate::serde::to_vec;
use crate::{BackendApi, Querier, Storage};

// init mimicks the call signature of the smart contracts.
// thus it moves env and msg rather than take them as reference.
// this is inefficient here, but only used in test code
pub fn init<A, S, Q, M, U>(
    instance: &mut Instance<A, S, Q>,
    env: Env,
    info: MessageInfo,
    msg: M,
) -> ContractResult<Response<U>>
where
    A: BackendApi + 'static,
    S: Storage + 'static,
    Q: Querier + 'static,
    M: Serialize + JsonSchema,
    U: DeserializeOwned + Clone + PartialEq + JsonSchema + fmt::Debug,
{
    let serialized_msg = to_vec(&msg).expect("Testing error: Could not seralize request message");
    call_init(instance, &env, &info, &serialized_msg).expect("VM error")
}

// handle mimicks the call signature of the smart contracts.
// thus it moves env and msg rather than take them as reference.
// this is inefficient here, but only used in test code
pub fn handle<A, S, Q, M, U>(
    instance: &mut Instance<A, S, Q>,
    env: Env,
    info: MessageInfo,
    msg: M,
) -> ContractResult<Response<U>>
where
    A: BackendApi + 'static,
    S: Storage + 'static,
    Q: Querier + 'static,
    M: Serialize + JsonSchema,
    U: DeserializeOwned + Clone + PartialEq + JsonSchema + fmt::Debug,
{
    let serialized_msg = to_vec(&msg).expect("Testing error: Could not seralize request message");
    call_handle(instance, &env, &info, &serialized_msg).expect("VM error")
}

// migrate mimicks the call signature of the smart contracts.
// thus it moves env and msg rather than take them as reference.
// this is inefficient here, but only used in test code
pub fn migrate<A, S, Q, M, U>(
    instance: &mut Instance<A, S, Q>,
    env: Env,
    msg: M,
) -> ContractResult<Response<U>>
where
    A: BackendApi + 'static,
    S: Storage + 'static,
    Q: Querier + 'static,
    M: Serialize + JsonSchema,
    U: DeserializeOwned + Clone + PartialEq + JsonSchema + fmt::Debug,
{
    let serialized_msg = to_vec(&msg).expect("Testing error: Could not seralize request message");
    call_migrate(instance, &env, &serialized_msg).expect("VM error")
}

// system mimicks the call signature of the smart contracts.
// thus it moves env and msg rather than take them as reference.
// this is inefficient here, but only used in test code
pub fn system<A, S, Q, M, U>(
    instance: &mut Instance<A, S, Q>,
    env: Env,
    msg: M,
) -> ContractResult<Response<U>>
where
    A: BackendApi + 'static,
    S: Storage + 'static,
    Q: Querier + 'static,
    M: Serialize + JsonSchema,
    U: DeserializeOwned + Clone + PartialEq + JsonSchema + fmt::Debug,
{
    let serialized_msg = to_vec(&msg).expect("Testing error: Could not seralize request message");
    call_system(instance, &env, &serialized_msg).expect("VM error")
}

// subcall_response mimicks the call signature of the smart contracts.
// thus it moves env and msg rather than take them as reference.
// this is inefficient here, but only used in test code
pub fn subcall_response<A, S, Q, U>(
    instance: &mut Instance<A, S, Q>,
    env: Env,
    msg: SubCallResult,
) -> ContractResult<Response<U>>
where
    A: BackendApi + 'static,
    S: Storage + 'static,
    Q: Querier + 'static,
    U: DeserializeOwned + Clone + PartialEq + JsonSchema + fmt::Debug,
{
    call_subcall_response(instance, &env, &msg).expect("VM error")
}

// query mimicks the call signature of the smart contracts.
// thus it moves env and msg rather than take them as reference.
// this is inefficient here, but only used in test code
pub fn query<A, S, Q, M>(
    instance: &mut Instance<A, S, Q>,
    env: Env,
    msg: M,
) -> ContractResult<QueryResponse>
where
    A: BackendApi + 'static,
    S: Storage + 'static,
    Q: Querier + 'static,
    M: Serialize + JsonSchema,
{
    let serialized_msg = to_vec(&msg).expect("Testing error: Could not seralize request message");
    call_query(instance, &env, &serialized_msg).expect("VM error")
}
