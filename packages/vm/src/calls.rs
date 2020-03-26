use snafu::ResultExt;

use cosmwasm_std::{from_slice, to_vec, Api, ContractResult, Env, QueryResult, Storage};

use crate::errors::{Error, ParseErr, RuntimeErr, SerializeErr};
use crate::instance::{Func, Instance};

pub fn call_init<S: Storage + 'static, A: Api + 'static>(
    instance: &mut Instance<S, A>,
    env: &Env,
    msg: &[u8],
) -> Result<ContractResult, Error> {
    let env = to_vec(env).context(SerializeErr {})?;
    let data = call_init_raw(instance, &env, msg)?;
    let res: ContractResult = from_slice(&data).context(ParseErr {})?;
    Ok(res)
}

pub fn call_handle<S: Storage + 'static, A: Api + 'static>(
    instance: &mut Instance<S, A>,
    env: &Env,
    msg: &[u8],
) -> Result<ContractResult, Error> {
    let env = to_vec(env).context(SerializeErr {})?;
    let data = call_handle_raw(instance, &env, msg)?;
    let res: ContractResult = from_slice(&data).context(ParseErr {})?;
    Ok(res)
}

pub fn call_query<S: Storage + 'static, A: Api + 'static>(
    instance: &mut Instance<S, A>,
    msg: &[u8],
) -> Result<QueryResult, Error> {
    let data = call_query_raw(instance, msg)?;
    let res: QueryResult = from_slice(&data).context(ParseErr {})?;
    Ok(res)
}

pub fn call_query_raw<S: Storage + 'static, A: Api + 'static>(
    instance: &mut Instance<S, A>,
    msg: &[u8],
) -> Result<Vec<u8>, Error> {
    // we cannot resuse the call_raw functionality as it assumes a param variable... just do it inline
    let msg_region_ptr = instance.allocate(msg)?;
    let func: Func<u32, u32> = instance.func("query")?;
    let res_region_ptr = func.call(msg_region_ptr).context(RuntimeErr {})?;
    let data = instance.memory(res_region_ptr);
    // free return value in wasm (arguments were freed in wasm code)
    instance.deallocate(res_region_ptr)?;
    Ok(data)
}

pub fn call_init_raw<S: Storage + 'static, A: Api + 'static>(
    instance: &mut Instance<S, A>,
    env: &[u8],
    msg: &[u8],
) -> Result<Vec<u8>, Error> {
    call_raw(instance, "init", env, msg)
}

pub fn call_handle_raw<S: Storage + 'static, A: Api + 'static>(
    instance: &mut Instance<S, A>,
    env: &[u8],
    msg: &[u8],
) -> Result<Vec<u8>, Error> {
    call_raw(instance, "handle", env, msg)
}

fn call_raw<S: Storage + 'static, A: Api + 'static>(
    instance: &mut Instance<S, A>,
    name: &str,
    env: &[u8],
    msg: &[u8],
) -> Result<Vec<u8>, Error> {
    let param_offset = instance.allocate(env)?;
    let msg_offset = instance.allocate(msg)?;

    let func: Func<(u32, u32), u32> = instance.func(name)?;
    let res_region_ptr = func.call(param_offset, msg_offset).context(RuntimeErr {})?;

    let data = instance.memory(res_region_ptr);
    // free return value in wasm (arguments were freed in wasm code)
    instance.deallocate(res_region_ptr)?;
    Ok(data)
}
