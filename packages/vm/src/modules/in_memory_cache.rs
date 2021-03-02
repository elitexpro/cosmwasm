use clru::{CLruCache, CLruCacheConfig, WeightScale};
use std::collections::hash_map::RandomState;
use wasmer::Module;

use crate::{Checksum, Size, VmError, VmResult};
use std::num::NonZeroUsize;

// Minimum module size.
// Based on `examples/module_size.sh`, and the cosmwasm-plus contracts.
// We use an estimated *minimum* module size in order to compute a number of pre-allocated entries
// that are enough to handle a size-limited cache without requiring re-allocation / resizing.
// This will incurr an extra memory cost for the unused entries, but it's negligible:
// Assuming the cost per entry is 48 bytes, 10000 entries will have an extra cost of just ~500 kB.
// Which is a very small percentage (~0.03%) of our typical cache memory budget (2 GB).
const MINIMUM_MODULE_SIZE: Size = Size::kibi(250);

#[derive(Debug)]
struct SizedModule {
    pub module: Module,
    pub size: usize,
}

#[derive(Debug)]
struct SizeScale;

impl WeightScale<Checksum, SizedModule> for SizeScale {
    #[inline]
    fn weight(&self, _key: &Checksum, value: &SizedModule) -> usize {
        value.size
    }
}

/// An in-memory module cache
pub struct InMemoryCache {
    modules: CLruCache<Checksum, SizedModule, RandomState, SizeScale>,
}

impl InMemoryCache {
    /// Creates a new cache with the given size (in bytes)
    /// and pre-allocated entries.
    pub fn new(size: Size) -> Self {
        let preallocated_entries = size.0 / MINIMUM_MODULE_SIZE.0;

        InMemoryCache {
            modules: CLruCache::with_config(
                CLruCacheConfig::new(NonZeroUsize::new(size.0).unwrap())
                    .with_memory(preallocated_entries)
                    .with_scale(SizeScale),
            ),
        }
    }

    pub fn store(&mut self, checksum: &Checksum, module: Module, size: usize) -> VmResult<()> {
        self.modules
            .put_with_weight(*checksum, SizedModule { module, size })
            .map_err(|e| VmError::cache_err(format!("{:?}", e)))?;
        Ok(())
    }

    /// Looks up a module in the cache and creates a new module
    pub fn load(&mut self, checksum: &Checksum) -> VmResult<Option<Module>> {
        match self.modules.get(checksum) {
            Some(sized_module) => Ok(Some(sized_module.module.clone())),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::size::Size;
    use crate::wasm_backend::compile;
    use std::mem;
    use wasmer::{imports, Instance as WasmerInstance};
    use wasmer_middlewares::metering::set_remaining_points;

    const TESTING_GAS_LIMIT: u64 = 5_000;
    // Based on `examples/module_size.sh`
    const TESTING_WASM_SIZE_FACTOR: usize = 18;

    #[test]
    fn check_element_sizes() {
        let key_size = mem::size_of::<Checksum>();
        assert_eq!(key_size, 32);

        // A Module consists of a Store (2 Arcs) and an Arc to the Engine.
        // This is 3 * 64bit of data, but we don't get any guarantee how the Rust structs
        // Module and Store are aligned (https://doc.rust-lang.org/reference/type-layout.html#the-default-representation).
        // So we get this value by trial and error. It can change over time and across platforms.
        let value_size = mem::size_of::<Module>();
        assert_eq!(value_size, 48);

        // Just in case we want to go that route
        let boxed_value_size = mem::size_of::<Box<Module>>();
        assert_eq!(boxed_value_size, 8);
    }

    #[test]
    fn in_memory_cache_run() {
        let mut cache = InMemoryCache::new(Size::mebi(200));

        // Create module
        let wasm = wat::parse_str(
            r#"(module
            (type $t0 (func (param i32) (result i32)))
            (func $add_one (export "add_one") (type $t0) (param $p0 i32) (result i32)
                get_local $p0
                i32.const 1
                i32.add)
            )"#,
        )
        .unwrap();
        let checksum = Checksum::generate(&wasm);

        // Module does not exist
        let cache_entry = cache.load(&checksum).unwrap();
        assert!(cache_entry.is_none());

        // Compile module
        let original = compile(&wasm, None).unwrap();

        // Ensure original module can be executed
        {
            let instance = WasmerInstance::new(&original, &imports! {}).unwrap();
            set_remaining_points(&instance, TESTING_GAS_LIMIT);
            let add_one = instance.exports.get_function("add_one").unwrap();
            let result = add_one.call(&[42.into()]).unwrap();
            assert_eq!(result[0].unwrap_i32(), 43);
        }

        // Store module
        let size = wasm.len() * TESTING_WASM_SIZE_FACTOR;
        cache.store(&checksum, original, size).unwrap();

        // Load module
        let cached = cache.load(&checksum).unwrap().unwrap();

        // Ensure cached module can be executed
        {
            let instance = WasmerInstance::new(&cached, &imports! {}).unwrap();
            set_remaining_points(&instance, TESTING_GAS_LIMIT);
            let add_one = instance.exports.get_function("add_one").unwrap();
            let result = add_one.call(&[42.into()]).unwrap();
            assert_eq!(result[0].unwrap_i32(), 43);
        }
    }
}
