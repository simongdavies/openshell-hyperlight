use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

/// Version of the request and response JSON contract.
pub const SCHEMA_VERSION: u32 = 1;

const MIN_TIMEOUT_MS: u64 = 1;
const MAX_TIMEOUT_MS: u64 = 300_000;
const MIN_HEAP_BYTES: u64 = 16 * 1024 * 1024;
// The pinned Wasm backend reserves 32 MiB input and output buffers plus 64 KiB.
const MIN_STACK_BYTES: u64 = (64 * 1024 * 1024) + (64 * 1024);
const MAX_MEMORY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_CAPTURE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CODE_BYTES: usize = 1024 * 1024;
const MAX_NETWORK_CAPABILITIES: usize = 128;

/// One Python execution request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecuteRequest {
    /// Contract version. The initial version is `1`.
    pub schema_version: u32,
    /// Python source code, limited to 1 MiB of UTF-8.
    pub code: String,
    /// Capabilities granted to this execution. Omitted capabilities are denied.
    pub capabilities: Capabilities,
    /// Required execution and output limits.
    pub limits: ExecutionLimits,
}

/// Capabilities granted to guest code.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    /// Exact HTTP origins and methods the guest may access.
    #[serde(default)]
    pub network: Vec<NetworkCapability>,
    /// Optional read-only input and writable output mappings.
    pub filesystem: Option<FilesystemCapabilities>,
    /// Reserved host tool names. Version 1 supports no host tools and requires this to be empty.
    #[serde(default)]
    pub tools: Vec<String>,
}

/// A network origin and the methods allowed for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkCapability {
    /// Exact `http` or `https` origin, with no path, query, fragment, or credentials.
    pub origin: String,
    /// Non-empty set of HTTP methods allowed for the origin.
    pub methods: Vec<HttpMethod>,
}

/// HTTP methods supported by the Hyperlight guest API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    /// Retrieve a resource.
    Get,
    /// Submit a resource.
    Post,
    /// Replace a resource.
    Put,
    /// Delete a resource.
    Delete,
    /// Retrieve response metadata.
    Head,
    /// Query supported request methods.
    Options,
    /// Partially update a resource.
    Patch,
}

/// Host directories explicitly mapped into the guest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilesystemCapabilities {
    /// Existing host directory exposed read-only at `/input`.
    pub input_directory: Option<PathBuf>,
    /// Existing, empty host directory exposed read-write at `/output`.
    pub output_directory: Option<PathBuf>,
    /// Required logical output limits when `output_directory` is present.
    pub output_limits: Option<FilesystemLimits>,
}

/// Logical writable-filesystem limits enforced by hyperlight-sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilesystemLimits {
    /// Maximum logical size of one output file.
    pub max_file_size_bytes: u64,
    /// Maximum cumulative logical size of output files.
    pub max_total_size_bytes: u64,
    /// Maximum number of output files.
    pub max_file_count: usize,
}

/// Required limits for every execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionLimits {
    /// Wall-clock deadline enforced by terminating the worker process.
    pub timeout_ms: u64,
    /// Maximum returned stdout bytes. Excess output is truncated.
    pub max_stdout_bytes: u64,
    /// Maximum returned stderr bytes. Excess output is truncated.
    pub max_stderr_bytes: u64,
    /// Hyperlight guest heap size.
    pub heap_bytes: u64,
    /// Hyperlight guest stack/scratch size.
    pub stack_bytes: u64,
}

/// Structured response to one execution request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecuteResponse {
    /// Contract version.
    pub schema_version: u32,
    /// Overall adapter status.
    pub status: ResponseStatus,
    /// Guest result when execution reached the guest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<ExecutionOutput>,
    /// Structured adapter error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

/// Adapter outcome categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    /// The guest returned exit code zero.
    Ok,
    /// The guest returned a non-zero exit code.
    GuestError,
    /// The adapter rejected or failed the request.
    Error,
    /// The worker exceeded the requested wall-clock deadline.
    Timeout,
}

/// Guest output after configured capture limits are applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionOutput {
    /// Captured standard output.
    pub stdout: String,
    /// Whether standard output was truncated.
    pub stdout_truncated: bool,
    /// Captured standard error.
    pub stderr: String,
    /// Whether standard error was truncated.
    pub stderr_truncated: bool,
    /// Guest exit code.
    pub exit_code: i32,
    /// Top-level files created in the mapped output directory.
    pub output_files: Vec<String>,
}

/// Machine-readable error details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorBody {
    /// Stable error code for callers.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
}

/// Adapter failures before a response is serialized.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    /// Input was not valid JSON.
    #[error("invalid request JSON")]
    InvalidJson {
        /// Parser error.
        #[source]
        source: serde_json::Error,
    },
    /// A request field violated the contract.
    #[error("invalid request: {message}")]
    InvalidRequest {
        /// Validation failure.
        message: String,
    },
    /// The worker process could not be managed.
    #[error("worker process failed: {message}")]
    Worker {
        /// Process failure.
        message: String,
    },
    /// The Hyperlight runtime failed.
    #[error("hyperlight execution failed: {message}")]
    Hyperlight {
        /// Runtime failure.
        message: String,
    },
    /// This build or platform cannot execute Hyperlight guests.
    #[error("Hyperlight execution is unavailable: {message}")]
    Unsupported {
        /// Unsupported configuration.
        message: String,
    },
}

impl ExecuteRequest {
    /// Validate the complete request before starting the worker deadline.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::InvalidRequest`] when any field or capability
    /// violates protocol version 1.
    pub fn validate(&self) -> Result<(), AdapterError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(AdapterError::invalid(format!(
                "schema_version must be {SCHEMA_VERSION}"
            )));
        }
        if self.code.is_empty() {
            return Err(AdapterError::invalid("code must not be empty"));
        }
        if self.code.len() > MAX_CODE_BYTES {
            return Err(AdapterError::invalid(format!(
                "code exceeds the {MAX_CODE_BYTES}-byte limit"
            )));
        }
        if !self.capabilities.tools.is_empty() {
            return Err(AdapterError::invalid(
                "host tools are not supported in schema version 1",
            ));
        }
        self.limits.validate()?;
        self.capabilities.validate()
    }
}

impl Capabilities {
    fn validate(&self) -> Result<(), AdapterError> {
        if self.network.len() > MAX_NETWORK_CAPABILITIES {
            return Err(AdapterError::invalid(format!(
                "network capability count exceeds {MAX_NETWORK_CAPABILITIES}"
            )));
        }
        self.network
            .iter()
            .try_for_each(NetworkCapability::validate)?;
        if let Some(filesystem) = &self.filesystem {
            filesystem.validate()?;
        }
        Ok(())
    }
}

impl NetworkCapability {
    fn validate(&self) -> Result<(), AdapterError> {
        let url = Url::parse(&self.origin)
            .map_err(|error| AdapterError::invalid(format!("invalid network origin: {error}")))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AdapterError::invalid(
                "network origins must use http or https",
            ));
        }
        if url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(AdapterError::invalid(
                "network origin must contain only scheme, host, and optional port",
            ));
        }
        if self.methods.is_empty() {
            return Err(AdapterError::invalid(
                "network capability methods must not be empty",
            ));
        }
        let unique = self.methods.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != self.methods.len() {
            return Err(AdapterError::invalid(
                "network capability methods must not contain duplicates",
            ));
        }
        Ok(())
    }
}

impl FilesystemCapabilities {
    fn validate(&self) -> Result<(), AdapterError> {
        if self.input_directory.is_none() && self.output_directory.is_none() {
            return Err(AdapterError::invalid(
                "filesystem capability must map input_directory or output_directory",
            ));
        }
        if let Some(input) = &self.input_directory {
            validate_directory(input, "input_directory")?;
        }
        match (&self.output_directory, &self.output_limits) {
            (Some(output), Some(limits)) => {
                validate_directory(output, "output_directory")?;
                if output
                    .read_dir()
                    .map_err(|error| {
                        AdapterError::invalid(format!(
                            "cannot inspect output_directory {}: {error}",
                            output.display()
                        ))
                    })?
                    .next()
                    .is_some()
                {
                    return Err(AdapterError::invalid(
                        "output_directory must be empty to receive guest output without collisions",
                    ));
                }
                limits.validate()?;
            }
            (Some(_), None) => {
                return Err(AdapterError::invalid(
                    "output_limits are required with output_directory",
                ));
            }
            (None, Some(_)) => {
                return Err(AdapterError::invalid(
                    "output_limits require output_directory",
                ));
            }
            (None, None) => {}
        }
        if let (Some(input), Some(output)) = (&self.input_directory, &self.output_directory) {
            let input = canonicalize(input, "input_directory")?;
            let output = canonicalize(output, "output_directory")?;
            if input == output || input.starts_with(&output) || output.starts_with(&input) {
                return Err(AdapterError::invalid(
                    "input_directory and output_directory must not overlap",
                ));
            }
        }
        Ok(())
    }
}

impl FilesystemLimits {
    fn validate(self) -> Result<(), AdapterError> {
        if self.max_file_size_bytes == 0
            || self.max_total_size_bytes == 0
            || self.max_file_count == 0
        {
            return Err(AdapterError::invalid(
                "filesystem limits must all be greater than zero",
            ));
        }
        if self.max_file_size_bytes > self.max_total_size_bytes {
            return Err(AdapterError::invalid(
                "max_file_size_bytes must not exceed max_total_size_bytes",
            ));
        }
        Ok(())
    }
}

impl ExecutionLimits {
    /// Return the requested worker deadline.
    #[must_use]
    pub const fn timeout(self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }

    fn validate(self) -> Result<(), AdapterError> {
        if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&self.timeout_ms) {
            return Err(AdapterError::invalid(format!(
                "timeout_ms must be between {MIN_TIMEOUT_MS} and {MAX_TIMEOUT_MS}"
            )));
        }
        for (name, value) in [
            ("max_stdout_bytes", self.max_stdout_bytes),
            ("max_stderr_bytes", self.max_stderr_bytes),
        ] {
            if value > MAX_CAPTURE_BYTES {
                return Err(AdapterError::invalid(format!(
                    "{name} must not exceed {MAX_CAPTURE_BYTES}"
                )));
            }
        }
        if !(MIN_HEAP_BYTES..=MAX_MEMORY_BYTES).contains(&self.heap_bytes) {
            return Err(AdapterError::invalid(format!(
                "heap_bytes must be between {MIN_HEAP_BYTES} and {MAX_MEMORY_BYTES}"
            )));
        }
        if !(MIN_STACK_BYTES..=MAX_MEMORY_BYTES).contains(&self.stack_bytes) {
            return Err(AdapterError::invalid(format!(
                "stack_bytes must be between {MIN_STACK_BYTES} and {MAX_MEMORY_BYTES}"
            )));
        }
        Ok(())
    }
}

impl ExecuteResponse {
    /// Build a structured adapter error response.
    #[must_use]
    pub fn from_error(error: &AdapterError) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            status: ResponseStatus::Error,
            execution: None,
            error: Some(ErrorBody {
                code: error.code().to_string(),
                message: error.to_string(),
            }),
        }
    }

    /// Build a timeout response.
    #[must_use]
    pub fn timeout(timeout_ms: u64) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            status: ResponseStatus::Timeout,
            execution: None,
            error: Some(ErrorBody {
                code: "execution_timeout".to_string(),
                message: format!("execution exceeded timeout_ms={timeout_ms}"),
            }),
        }
    }
}

impl AdapterError {
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            message: message.into(),
        }
    }

    const fn code(&self) -> &'static str {
        match self {
            Self::InvalidJson { .. } => "invalid_json",
            Self::InvalidRequest { .. } => "invalid_request",
            Self::Worker { .. } => "worker_failed",
            Self::Hyperlight { .. } => "hyperlight_failed",
            Self::Unsupported { .. } => "unsupported",
        }
    }
}

fn validate_directory(path: &Path, field: &str) -> Result<(), AdapterError> {
    if !path.is_absolute() {
        return Err(AdapterError::invalid(format!("{field} must be absolute")));
    }
    let metadata = path.metadata().map_err(|error| {
        AdapterError::invalid(format!(
            "cannot inspect {field} {}: {error}",
            path.display()
        ))
    })?;
    if !metadata.is_dir() {
        return Err(AdapterError::invalid(format!(
            "{field} must be a directory"
        )));
    }
    Ok(())
}

fn canonicalize(path: &Path, field: &str) -> Result<PathBuf, AdapterError> {
    path.canonicalize().map_err(|error| {
        AdapterError::invalid(format!(
            "cannot canonicalize {field} {}: {error}",
            path.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ExecuteRequest {
        ExecuteRequest {
            schema_version: SCHEMA_VERSION,
            code: "print('ok')".to_string(),
            capabilities: Capabilities::default(),
            limits: ExecutionLimits {
                timeout_ms: 1_000,
                max_stdout_bytes: 1_024,
                max_stderr_bytes: 1_024,
                heap_bytes: 32 * 1024 * 1024,
                stack_bytes: 65 * 1024 * 1024,
            },
        }
    }

    #[test]
    fn accepts_default_deny_request() {
        assert_eq!(
            request().validate().map_err(|error| error.to_string()),
            Ok(())
        );
    }

    #[test]
    fn rejects_unknown_json_fields() {
        let result = serde_json::from_str::<ExecuteRequest>(
            r#"{"schema_version":1,"code":"pass","capabilities":{},"limits":{"timeout_ms":1,"max_stdout_bytes":0,"max_stderr_bytes":0,"heap_bytes":16777216,"stack_bytes":16777216},"extra":true}"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn rejects_origin_path() {
        let mut request = request();
        request.capabilities.network.push(NetworkCapability {
            origin: "https://example.com/api".to_string(),
            methods: vec![HttpMethod::Get],
        });

        assert_eq!(
            request.validate().map_err(|error| error.to_string()),
            Err(
                "invalid request: network origin must contain only scheme, host, and optional port"
                    .to_string()
            )
        );
    }

    #[test]
    fn rejects_tools() {
        let mut request = request();
        request.capabilities.tools.push("read_secret".to_string());

        assert_eq!(
            request.validate().map_err(|error| error.to_string()),
            Err("invalid request: host tools are not supported in schema version 1".to_string())
        );
    }
}
