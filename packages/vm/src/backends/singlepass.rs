#![cfg(any(feature = "singlepass", feature = "default-singlepass"))]

// use wasmer_middleware_common::metering;
use wasmer::{Module, Store};
use wasmer_compiler_singlepass::Singlepass;
use wasmer_engine_jit::JIT;

use crate::context::Env;
use crate::errors::VmResult;
use crate::traits::{Querier, Storage};
// use crate::middleware::DeterministicMiddleware;

/// In Wasmer, the gas limit is set on modules during compilation and is included in the cached modules.
/// This causes issues when trying to instantiate the same compiled module with a different gas limit.
/// A fix for this is proposed here: https://github.com/wasmerio/wasmer/pull/996.
///
/// To work around this limitation, we set the gas limit of all Wasmer instances to this very high value,
/// assuming users won't request more than this amount of gas. In order to set the real gas limit, we pretend
/// to consume the difference between the two in `set_gas_left` ("points used" in the metering middleware).
/// Since we observed overflow behaviour in the points used, we ensure both MAX_GAS_LIMIT and points used stay
/// far below u64::MAX.
const MAX_GAS_LIMIT: u64 = u64::MAX / 2;

const FAKE_GAS_AVAILABLE: u64 = 1_000_000;

pub fn compile(code: &[u8]) -> VmResult<Module> {
    let compiler = Singlepass::default();
    let engine = JIT::new(&compiler).engine();
    let store = Store::new(&engine);
    let module = Module::new(&store, code)?;
    Ok(module)
}

pub fn backend() -> &'static str {
    "singlepass"
}

/// Set the amount of gas units that can be used in the context.
pub fn set_gas_left<S: Storage, Q: Querier>(_env: &mut Env<S, Q>, _amount: u64) {}

/// Get how many more gas units can be used in the context.
pub fn get_gas_left<S: Storage, Q: Querier>(_env: &Env<S, Q>) -> u64 {
    FAKE_GAS_AVAILABLE
}

// /// Set the amount of gas units that can be used in the context.
// pub fn set_gas_left(ctx: &mut Ctx, amount: u64) {
//     if amount > MAX_GAS_LIMIT {
//         panic!(
//             "Attempted to set gas limit larger than max gas limit (got: {}; maximum: {}).",
//             amount, MAX_GAS_LIMIT
//         );
//     } else {
//         let used = MAX_GAS_LIMIT - amount;
//         metering::set_points_used_ctx(ctx, used);
//     }
// }

// /// Get how many more gas units can be used in the context.
// pub fn get_gas_left(ctx: &Ctx) -> u64 {
//     let used = metering::get_points_used_ctx(ctx);
//     // when running out of gas, get_points_used can exceed MAX_GAS_LIMIT
//     MAX_GAS_LIMIT.saturating_sub(used)
// }

#[cfg(test)]
mod test {
    use super::*;
    use crate::testing::{MockQuerier, MockStorage};
    use std::ptr::NonNull;
    use wabt::wat2wasm;
    use wasmer::{imports, Instance as WasmerInstance};

    type MS = MockStorage;
    type MQ = MockQuerier;
    const GAS_LIMIT: u64 = 5_000_000;

    fn instantiate(code: &[u8]) -> (Env<MS, MQ>, Box<WasmerInstance>) {
        let mut env = Env::new(GAS_LIMIT);
        let module = compile(code).unwrap();
        let import_obj = imports! { "env" => {}, };
        let instance = Box::from(WasmerInstance::new(&module, &import_obj).unwrap());

        let instance_ptr = NonNull::from(instance.as_ref());
        env.set_wasmer_instance(Some(instance_ptr));

        (env, instance)
    }

    #[test]
    fn get_gas_left_defaults_to_constant() {
        let wasm = wat2wasm("(module)").unwrap();
        let (env, _) = instantiate(&wasm);
        let gas_left = get_gas_left(&env);
        assert_eq!(gas_left, MAX_GAS_LIMIT);
    }

    #[test]
    fn set_gas_left_works() {
        let wasm = wat2wasm("(module)").unwrap();
        let (mut env, _) = instantiate(&wasm);

        let limit = 3456789;
        set_gas_left(&mut env, limit);
        assert_eq!(get_gas_left(&env), limit);

        let limit = 1;
        set_gas_left(&mut env, limit);
        assert_eq!(get_gas_left(&env), limit);

        let limit = 0;
        set_gas_left(&mut env, limit);
        assert_eq!(get_gas_left(&env), limit);

        let limit = MAX_GAS_LIMIT;
        set_gas_left(&mut env, limit);
        assert_eq!(get_gas_left(&env), limit);
    }

    #[test]
    #[should_panic(
        expected = "Attempted to set gas limit larger than max gas limit (got: 9223372036854775808; maximum: 9223372036854775807)."
    )]
    fn set_gas_left_panic_for_values_too_large() {
        let wasm = wat2wasm("(module)").unwrap();
        let (mut env, _) = instantiate(&wasm);

        let limit = MAX_GAS_LIMIT + 1;
        set_gas_left(&mut env, limit);
    }
}
