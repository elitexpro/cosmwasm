use std::convert::TryInto;
use std::sync::Arc;
#[cfg(feature = "cranelift")]
use wasmer::Cranelift;
#[cfg(not(feature = "cranelift"))]
use wasmer::Singlepass;
use wasmer::{
    wasmparser::Operator, BaseTunables, CompilerConfig, Engine, Pages, Store, Target, JIT,
    WASM_PAGE_SIZE,
};
use wasmer_middlewares::Metering;

use crate::middleware::Deterministic;
use crate::size::Size;

use super::limiting_tunables::LimitingTunables;

/// WebAssembly linear memory objects have sizes measured in pages. Each page
/// is 65536 (2^16) bytes. In WebAssembly version 1, a linear memory can have at
/// most 65536 pages, for a total of 2^32 bytes (4 gibibytes).
/// https://github.com/WebAssembly/memory64/blob/master/proposals/memory64/Overview.md
const MAX_WASM_MEMORY: usize = 4 * 1024 * 1024 * 1024;

fn cost(operator: &Operator) -> u64 {
    match operator {
        Operator::LocalGet { .. } | Operator::I32Const { .. } => 9,
        Operator::I32Add { .. } => 12,
        _ => 10,
    }
}

/// Created a store with the default compiler and the given memory limit (in bytes).
/// If memory_limit is None, no limit is applied.
pub fn make_compile_time_store(memory_limit: Option<Size>) -> Store {
    let gas_limit = 0;
    let deterministic = Arc::new(Deterministic::new());
    let metering = Arc::new(Metering::new(gas_limit, cost));

    #[cfg(feature = "cranelift")]
    {
        let mut config = Cranelift::default();
        config.push_middleware(deterministic);
        config.push_middleware(metering);
        let engine = JIT::new(config).engine();
        make_store_with_engine(&engine, memory_limit)
    }

    #[cfg(not(feature = "cranelift"))]
    {
        let mut config = Singlepass::default();
        config.push_middleware(deterministic);
        config.push_middleware(metering);
        let engine = JIT::new(config).engine();
        make_store_with_engine(&engine, memory_limit)
    }
}

/// Created a store with no compiler and the given memory limit (in bytes)
/// If memory_limit is None, no limit is applied.
pub fn make_runtime_store(memory_limit: Size) -> Store {
    let engine = JIT::headless().engine();
    make_store_with_engine(&engine, Some(memory_limit))
}

/// Creates a store from an engine and an optional memory limit.
/// If no limit is set, the no custom tunables will be used.
fn make_store_with_engine(engine: &dyn Engine, memory_limit: Option<Size>) -> Store {
    match memory_limit {
        Some(limit) => {
            let base = BaseTunables::for_target(&Target::default());
            let tunables = LimitingTunables::new(base, limit_to_pages(limit));
            Store::new_with_tunables(engine, tunables)
        }
        None => Store::new(engine),
    }
}

fn limit_to_pages(limit: Size) -> Pages {
    let capped = std::cmp::min(limit.0, MAX_WASM_MEMORY);
    // round down to ensure the limit is less than or equal to the config
    let pages: u32 = (capped / WASM_PAGE_SIZE)
        .try_into()
        .expect("Value must be <= 4 GiB/64KiB, i.e. fit in uint32. This is a bug.");
    Pages(pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_to_pages_works() {
        // rounds down
        assert_eq!(limit_to_pages(Size(0)), Pages(0));
        assert_eq!(limit_to_pages(Size(1)), Pages(0));
        assert_eq!(limit_to_pages(Size::kibi(63)), Pages(0));
        assert_eq!(limit_to_pages(Size::kibi(64)), Pages(1));
        assert_eq!(limit_to_pages(Size::kibi(65)), Pages(1));
        // caps at 4 GiB
        assert_eq!(limit_to_pages(Size::gibi(3)), Pages(49152));
        assert_eq!(limit_to_pages(Size::gibi(4)), Pages(65536));
        assert_eq!(limit_to_pages(Size::gibi(5)), Pages(65536));
    }
}
