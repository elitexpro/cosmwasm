use std::fs::create_dir_all;
use std::path::PathBuf;

use failure::{bail, Error};
use lru::LruCache;

use cosmwasm::storage::Storage;

use crate::backends::{backend, compile};
use crate::modules::{Cache, FileSystemCache, WasmHash};
use crate::wasm_store::{load, save, wasm_hash};
use crate::wasmer::{instantiate, mod_to_instance, Instance};

pub struct CosmCache {
    wasm_path: PathBuf,
    modules: FileSystemCache,
    instances: Option<LruCache<WasmHash, Instance>>,
}

static WASM_DIR: &str = "wasm";
static MODULES_DIR: &str = "modules";

impl CosmCache {
    /// new stores the data for cache under base_dir
    pub unsafe fn new<P: Into<PathBuf>>(base_dir: P, cache_size: usize) -> Self {
        let base = base_dir.into();
        let wasm_path = base.join(WASM_DIR);
        create_dir_all(&wasm_path).unwrap();
        let modules = FileSystemCache::new(base.join(MODULES_DIR)).unwrap();
        let instances = if cache_size > 0 {
            Some(LruCache::new(cache_size))
        } else {
            None
        };
        CosmCache { modules, wasm_path, instances }
    }
}

impl CosmCache {
    pub fn save_wasm(&mut self, wasm: &[u8]) -> Result<Vec<u8>, Error> {
        let id = save(&self.wasm_path, wasm)?;
        // we fail if module doesn't compile - panic :(
        let module = compile(wasm);
        let hash = WasmHash::generate(&id);
        let saved = self.modules.store(hash, module);
        // ignore it (just log) if module cache not supported
        if let Err(e) = saved {
            println!("Cannot save module: {:?}", e);
        }
        Ok(id)
    }

    pub fn load_wasm(&self, id: &[u8]) -> Result<Vec<u8>, Error> {
        let code = load(&self.wasm_path, id)?;
        // verify hash matches (integrity check)
        let hash = wasm_hash(&code);
        if hash.ne(&id) {
            bail!("hash doesn't match stored data")
        }
        Ok(code)
    }

    /// get instance returns a wasmer Instance tied to a previously saved wasm
    pub fn get_instance<T>(&mut self, id: &[u8], storage: T) -> Result<Instance, Error>
        where T: Storage + Send + Sync + Clone + 'static {

        let hash = WasmHash::generate(&id);

        // pop from lru cache if present
        if let Some(cache) = &mut self.instances {
            let val = cache.pop(&hash);
            if let Some(inst) = val {
                // TODO: change the bound storage to this one!
                return Ok(inst);
            }
        }

        // try from the module cache
        let res = self.modules.load_with_backend(hash, backend());
        if let Ok(module) = res {
            return Ok(mod_to_instance(&module, storage));
        }

        // fall back to wasm cache (and re-compiling) - this is for backends that don't support serialization
        let wasm = self.load_wasm(id)?;
        Ok(instantiate(&wasm, storage))
    }

    pub fn store_instance<T>(&mut self, id: &[u8], instance: Instance) {
        if let Some(cache) = &mut self.instances {
            let hash = WasmHash::generate(&id);
            cache.put(hash, instance);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tempfile::TempDir;

    use crate::calls::{call_handle, call_init};
    use cosmwasm::types::{coin, mock_params};
    use cosmwasm::mock::MockStorage;

    static CONTRACT: &[u8] = include_bytes!("../testdata/contract.wasm");

    #[test]
    fn init_cached_contract() {
        let tmp_dir = TempDir::new().unwrap();
        let mut cache = unsafe { CosmCache::new(tmp_dir.path(), 10) };
        let id = cache.save_wasm(CONTRACT).unwrap();
        let storage = MockStorage::new();
        let mut instance = cache.get_instance(&id, storage).unwrap();

        // run contract
        let params = mock_params("creator", &coin("1000", "earth"), &[]);
        let msg = r#"{"verifier": "verifies", "beneficiary": "benefits"}"#.as_bytes();

        // call and check
        let res = call_init(&mut instance, &params, msg).unwrap();
        let msgs = res.unwrap().messages;
        assert_eq!(msgs.len(), 0);
    }

    #[test]
    fn run_cached_contract() {
        let tmp_dir = TempDir::new().unwrap();
        let mut cache = unsafe { CosmCache::new(tmp_dir.path(), 10) };
        let id = cache.save_wasm(CONTRACT).unwrap();
        let storage = MockStorage::new();
        let mut instance = cache.get_instance(&id, storage).unwrap();

        // init contract
        let params = mock_params("creator", &coin("1000", "earth"), &[]);
        let msg = r#"{"verifier": "verifies", "beneficiary": "benefits"}"#.as_bytes();
        let res = call_init(&mut instance, &params, msg).unwrap();
        let msgs = res.unwrap().messages;
        assert_eq!(msgs.len(), 0);

        // run contract - just sanity check - results validate in contract unit tests
        let params = mock_params("verifies", &coin("15", "earth"), &coin("1015", "earth"));
        let msg = b"{}";
        let res = call_handle(&mut instance, &params, msg).unwrap();
        let msgs = res.unwrap().messages;
        assert_eq!(1, msgs.len());
    }
}
