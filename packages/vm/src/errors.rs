use std::fmt::Debug;
use std::io;

use snafu::Snafu;
use wasmer_runtime_core::error as core_error;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub")]
pub enum VmError {
    #[snafu(display("Cache error: {}", msg))]
    CacheErr {
        msg: String,
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Couldn't convert from {} to {}. Input: {}", from_type, to_type, input))]
    ConversionErr {
        from_type: &'static str,
        to_type: &'static str,
        input: String,
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Compiling wasm: {}", source))]
    CompileErr {
        source: core_error::CompileError,
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Filesystem error: {}", source))]
    IoErr {
        source: io::Error,
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Iterator with ID {} does not exist", id))]
    IteratorDoesNotExist {
        id: u32,
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Hash doesn't match stored data"))]
    IntegrityErr { backtrace: snafu::Backtrace },
    #[snafu(display("Parse error: {}", source))]
    ParseErr {
        kind: &'static str,
        source: serde_json::Error,
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Serialize error: {}", source))]
    SerializeErr {
        kind: &'static str,
        source: serde_json::Error,
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Resolving wasm function: {}", source))]
    ResolveErr {
        source: core_error::ResolveError,
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Region length too big. Got {}, limit {}", length, max_length))]
    // Note: this only checks length, not capacity
    RegionLengthTooBigErr {
        length: usize,
        max_length: usize,
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Region too small. Got {}, required {}", size, required))]
    RegionTooSmallErr {
        size: usize,
        required: usize,
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Runtime error: {}", msg))]
    RuntimeErr {
        msg: String,
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Uninitialized Context Data: {}", kind))]
    UninitializedContextData {
        kind: &'static str,
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Validating Wasm: {}", msg))]
    ValidationErr {
        msg: String,
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Wasmer error: {}", source))]
    WasmerErr {
        source: core_error::Error,
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Calling wasm function: {}", source))]
    WasmerRuntimeErr {
        source: core_error::RuntimeError,
        backtrace: snafu::Backtrace,
    },
}

pub type VmResult<T> = core::result::Result<T, VmError>;

pub fn make_runtime_err<T>(msg: &'static str) -> VmResult<T> {
    RuntimeErr { msg }.fail()
}

pub fn make_validation_err<T>(msg: String) -> VmResult<T> {
    ValidationErr { msg }.fail()
}
