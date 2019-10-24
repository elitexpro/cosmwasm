use snafu::{Snafu};

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub")]
pub enum Error {
    #[snafu(display("Received null pointer, refuse to use"))]
    NullPointer {
        #[cfg(feature = "backtraces")]
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("JSON error: {}", source))]
    JsonError {
        source: serde_json::error::Error,
        #[cfg(feature = "backtraces")]
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Contract error: {}", msg))]
    ContractErr {
        msg: String,
        #[cfg(feature = "backtraces")]
        backtrace: snafu::Backtrace,
    },
    #[snafu(display("Unauthorized"))]
    Unauthorized {
        #[cfg(feature = "backtraces")]
        backtrace: snafu::Backtrace,
    },
}

pub type Result<T, E = Error> = core::result::Result<T, E>;