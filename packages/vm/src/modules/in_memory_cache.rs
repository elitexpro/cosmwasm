use clru::CLruCache;
use wasmer::Module;

use crate::{Checksum, Size, VmResult};

const ESTIMATED_MODULE_SIZE: Size = Size::mebi(10);

/// An in-memory module cache
pub struct InMemoryCache {
    modules: CLruCache<Checksum, Module>,
}

impl InMemoryCache {
    /// Creates a new cache with the given size (in bytes)
    pub fn new(size: Size) -> Self {
        let max_entries = size.0 / ESTIMATED_MODULE_SIZE.0;
        InMemoryCache {
            modules: CLruCache::new(max_entries),
        }
    }

    pub fn store(&mut self, checksum: &Checksum, module: Module) -> VmResult<()> {
        self.modules.put(*checksum, module);
        Ok(())
    }

    /// Looks up a module in the cache and creates a new module
    pub fn load(&mut self, checksum: &Checksum) -> VmResult<Option<Module>> {
        match self.modules.get(checksum) {
            Some(module) => Ok(Some(module.clone())),
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
        cache.store(&checksum, original).unwrap();

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
