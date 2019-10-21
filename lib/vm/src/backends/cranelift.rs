use wasmer_clif_backend::CraneliftCompiler;
use wasmer_runtime::{compile_with, Module};

pub fn compile(code: &[u8]) -> Module {
    compile_with(code, &CraneliftCompiler::new()).unwrap()
}
