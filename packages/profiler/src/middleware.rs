use std::sync::{Arc, Mutex};

use loupe::MemoryUsage;
use wasmer::{
    wasmparser::Operator, FunctionMiddleware, FunctionType, ModuleMiddleware, Type, ValueType,
};
use wasmer_types::{FunctionIndex, ImportIndex};

use crate::{code_blocks::BlockStore, operators::OperatorSymbol};

#[non_exhaustive]
#[derive(Debug, MemoryUsage)]
pub struct Profiling {
    block_store: Arc<Mutex<BlockStore>>,
    indexes: Mutex<Option<ProfilingIndexes>>,
}

impl Profiling {
    pub fn new() -> Self {
        Self {
            block_store: Arc::new(Mutex::new(BlockStore::new())),
            indexes: Mutex::new(None),
        }
    }
}

impl ModuleMiddleware for Profiling {
    fn generate_function_middleware(
        &self,
        _local_function_index: wasmer::LocalFunctionIndex,
    ) -> Box<dyn wasmer::FunctionMiddleware> {
        Box::new(FunctionProfiling::new(
            self.block_store.clone(),
            self.indexes.lock().unwrap().clone().unwrap(),
        ))
    }

    fn transform_module_info(&self, module_info: &mut wasmer_vm::ModuleInfo) {
        let mut indexes = self.indexes.lock().unwrap();

        if indexes.is_some() {
            panic!("Profiling::transform_module_info: Attempting to use a `Profiling` middleware from multiple modules.");
        }

        let sig = module_info
            .signatures
            .push(FunctionType::new([], [Type::I32]));
        let fn1 = module_info.functions.push(sig);
        let import_index = module_info.imports().len();
        module_info.imports.insert(
            (
                "profiling".to_string(),
                "start_measurement".to_string(),
                import_index as u32,
            ),
            ImportIndex::Function(fn1),
        );

        let sig = module_info
            .signatures
            .push(FunctionType::new([Type::I32, Type::I64], []));
        let fn2 = module_info.functions.push(sig);
        let import_index = module_info.imports().len();
        module_info.imports.insert(
            (
                "profiling".to_string(),
                "take_measurement".to_string(),
                import_index as u32,
            ),
            ImportIndex::Function(fn2),
        );

        *indexes = Some(ProfilingIndexes {
            start_measurement: fn1,
            take_measurement: fn2,
        });
    }
}

#[derive(Debug)]
struct FunctionProfiling {
    block_store: Arc<Mutex<BlockStore>>,
    accumulated_ops: Vec<OperatorSymbol>,
    indexes: ProfilingIndexes,
}

impl FunctionProfiling {
    fn new(block_store: Arc<Mutex<BlockStore>>, indexes: ProfilingIndexes) -> Self {
        Self {
            block_store,
            accumulated_ops: Vec::new(),
            indexes,
        }
    }
}

impl FunctionMiddleware for FunctionProfiling {
    fn feed<'a>(
        &mut self,
        operator: wasmer::wasmparser::Operator<'a>,
        state: &mut wasmer::MiddlewareReaderState<'a>,
    ) -> Result<(), wasmer::MiddlewareError> {
        // Possible sources and targets of a branch. Finalize the cost of the previous basic block and perform necessary checks.
        match operator {
            Operator::Loop { .. } // loop headers are branch targets
            | Operator::End // block ends are branch targets
            | Operator::Else // "else" is the "end" of an if branch
            | Operator::Br { .. } // branch source
            | Operator::BrTable { .. } // branch source
            | Operator::BrIf { .. } // branch source
            | Operator::Call { .. } // function call - branch source
            | Operator::CallIndirect { .. } // function call - branch source
            | Operator::Return // end of function - branch source
            => {
                if !self.accumulated_ops.is_empty() {
                    let mut store = self.block_store.lock().unwrap();
                    store.register_block(std::mem::take(&mut self.accumulated_ops));
                }
            }
            _ => {
                self.accumulated_ops.push((&operator).into());
            }
        }

        state.push_operator(operator);
        Ok(())
    }
}

#[derive(Debug, MemoryUsage, Clone)]
struct ProfilingIndexes {
    start_measurement: FunctionIndex,
    take_measurement: FunctionIndex,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::code_blocks::CodeBlock;

    use std::sync::Arc;
    use wasmer::{
        imports, wat2wasm, CompilerConfig, Cranelift, Function, Instance, Module, Store, Universal,
    };
    use wasmer_types::Value;

    const WAT: &[u8] = br#"
    (module
    (type $t0 (func (param i32) (result i32)))
    (func $add_one (export "add_one") (type $t0) (param $p0 i32) (result i32)
        get_local $p0
        i32.const 1
        i32.add)
    (func $multisub (export "multisub") (type $t0) (param $p0 i32) (result i32)
        get_local $p0
        i32.const 2
        i32.mul
        call $sub_one
        i32.const 1
        i32.sub)
    (func $sub_one (type $t0) (param $p0 i32) (result i32)
        get_local $p0
        i32.const 1
        i32.sub))
    "#;

    #[test]
    fn middleware_registers_code_blocks() {
        let profiling = Arc::new(Profiling::new());

        // Create the module with our middleware.
        let mut compiler_config = Cranelift::default();
        compiler_config.push_middleware(profiling.clone());
        let store = Store::new(&Universal::new(compiler_config).engine());
        let wasm = wat2wasm(WAT).unwrap();
        let module = Module::new(&store, wasm).unwrap();

        // println!("{:?}", module.info());
        // panic!();

        // Mock imports that do nothing.
        let imports = imports! {
            "profiling" => {
                "start_measurement" => Function::new_native(&store, || 0),
                "take_measurement" => Function::new_native(&store, |_: u32, _: u64| {}),
            }
        };
        let instance = Instance::new(&module, &imports).unwrap();

        let add_one = instance.exports.get_function("add_one").unwrap();
        let result = add_one.call(&[Value::I32(42)]).unwrap();
        assert_eq!(result[0], Value::I32(43));

        let multisub = instance.exports.get_function("multisub").unwrap();
        let result = multisub.call(&[Value::I32(4)]).unwrap();
        assert_eq!(result[0], Value::I32(6));

        let block_store = profiling.block_store.lock().unwrap();
        assert_eq!(block_store.len(), 4);

        // The body of $add_one.
        let expected_block = CodeBlock::from(vec![
            OperatorSymbol::LocalGet,
            OperatorSymbol::I32Const,
            OperatorSymbol::I32Add,
        ]);
        let block = block_store.get_block(expected_block.get_hash());
        assert_eq!(block, Some(&expected_block));

        // The body of $sub_one
        let expected_block = CodeBlock::from(vec![
            OperatorSymbol::LocalGet,
            OperatorSymbol::I32Const,
            OperatorSymbol::I32Sub,
        ]);
        let block = block_store.get_block(expected_block.get_hash());
        assert_eq!(block, Some(&expected_block));

        // The body of $multisub until the `call` instruction.
        let expected_block = CodeBlock::from(vec![
            OperatorSymbol::LocalGet,
            OperatorSymbol::I32Const,
            OperatorSymbol::I32Mul,
        ]);
        let block = block_store.get_block(expected_block.get_hash());
        assert_eq!(block, Some(&expected_block));

        // The body of $multisub after the `call` instruction.
        let expected_block =
            CodeBlock::from(vec![OperatorSymbol::I32Const, OperatorSymbol::I32Sub]);
        let block = block_store.get_block(expected_block.get_hash());
        assert_eq!(block, Some(&expected_block));
    }
}
