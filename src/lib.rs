//! JSON contract and execution boundary for the `OpenShell` Hyperlight adapter.

mod contract;
mod process;

#[cfg(all(target_os = "linux", feature = "hyperlight"))]
mod hyperlight;

pub use contract::{
    AdapterError, Capabilities, ErrorBody, ExecuteRequest, ExecuteResponse, ExecutionLimits,
    ExecutionOutput, FilesystemCapabilities, FilesystemLimits, HttpMethod, NetworkCapability,
    ResponseStatus, SCHEMA_VERSION,
};
pub use process::{execute_with_timeout, worker_main};
