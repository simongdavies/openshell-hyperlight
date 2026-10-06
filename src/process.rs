use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

use wait_timeout::ChildExt;

use crate::{AdapterError, ExecuteRequest, ExecuteResponse};

const MAX_WORKER_DIAGNOSTIC_BYTES: usize = 64 * 1024;

/// Execute a validated request in a dedicated worker process.
///
/// # Errors
///
/// Returns an error when validation fails, the worker cannot be managed, or
/// the worker returns an invalid response.
pub fn execute_with_timeout(
    current_executable: &Path,
    python_guest: &Path,
    request: &ExecuteRequest,
) -> Result<ExecuteResponse, AdapterError> {
    request.validate()?;
    let mut child = Command::new(current_executable)
        .arg("__worker")
        .arg("--python-guest")
        .arg(python_guest)
        .env("OPENSHELL_HYPERLIGHT_INTERNAL_WORKER", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| AdapterError::Worker {
            message: format!("failed to start worker: {error}"),
        })?;

    let request_json = serde_json::to_vec(request).map_err(|error| AdapterError::Worker {
        message: format!("failed to serialize validated request: {error}"),
    })?;
    let mut stdin = child.stdin.take().ok_or_else(|| AdapterError::Worker {
        message: "worker stdin was unavailable".to_string(),
    })?;
    stdin
        .write_all(&request_json)
        .map_err(|error| AdapterError::Worker {
            message: format!("failed to send request to worker: {error}"),
        })?;
    drop(stdin);

    let stdout = child.stdout.take().ok_or_else(|| AdapterError::Worker {
        message: "worker stdout was unavailable".to_string(),
    })?;
    let stderr = child.stderr.take().ok_or_else(|| AdapterError::Worker {
        message: "worker stderr was unavailable".to_string(),
    })?;
    let stdout_reader = thread::spawn(move || read_to_end(stdout));
    let stderr_reader = thread::spawn(move || read_to_end(stderr));

    let status = child
        .wait_timeout(request.limits.timeout())
        .map_err(|error| AdapterError::Worker {
            message: format!("failed while waiting for worker: {error}"),
        })?;
    let timed_out = status.is_none();
    if timed_out {
        child.kill().map_err(|error| AdapterError::Worker {
            message: format!("failed to terminate timed-out worker: {error}"),
        })?;
        child.wait().map_err(|error| AdapterError::Worker {
            message: format!("failed to reap timed-out worker: {error}"),
        })?;
    }

    let stdout = join_reader(stdout_reader, "stdout")?;
    let stderr = join_reader(stderr_reader, "stderr")?;
    if timed_out {
        return Ok(ExecuteResponse::timeout(request.limits.timeout_ms));
    }
    let status = status.ok_or_else(|| AdapterError::Worker {
        message: "worker status disappeared".to_string(),
    })?;
    if let Ok(response) = serde_json::from_slice::<ExecuteResponse>(&stdout) {
        return Ok(response);
    }
    if !status.success() {
        return Err(AdapterError::Worker {
            message: format!(
                "worker exited with {status}; stderr: {}",
                bounded_diagnostic(&stderr)
            ),
        });
    }
    serde_json::from_slice(&stdout).map_err(|error| AdapterError::Worker {
        message: format!(
            "worker returned invalid JSON: {error}; stderr: {}",
            bounded_diagnostic(&stderr)
        ),
    })
}

/// Run the in-process Hyperlight worker and return its structured response.
///
/// # Errors
///
/// Returns an error when the JSON contract is invalid, the build does not
/// support Hyperlight execution, or Hyperlight fails.
pub fn worker_main(
    python_guest: &Path,
    request_json: &[u8],
) -> Result<ExecuteResponse, AdapterError> {
    let request = serde_json::from_slice::<ExecuteRequest>(request_json)
        .map_err(|source| AdapterError::InvalidJson { source })?;
    request.validate()?;
    execute_hyperlight(python_guest, &request)
}

#[cfg(all(target_os = "linux", feature = "hyperlight"))]
fn execute_hyperlight(
    python_guest: &Path,
    request: &ExecuteRequest,
) -> Result<ExecuteResponse, AdapterError> {
    crate::hyperlight::execute(python_guest, request)
}

#[cfg(not(all(target_os = "linux", feature = "hyperlight")))]
fn execute_hyperlight(
    _python_guest: &Path,
    _request: &ExecuteRequest,
) -> Result<ExecuteResponse, AdapterError> {
    Err(AdapterError::Unsupported {
        message: "build on Linux with --features hyperlight".to_string(),
    })
}

fn read_to_end(mut reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn join_reader(
    reader: thread::JoinHandle<std::io::Result<Vec<u8>>>,
    stream: &str,
) -> Result<Vec<u8>, AdapterError> {
    reader
        .join()
        .map_err(|_| AdapterError::Worker {
            message: format!("worker {stream} reader panicked"),
        })?
        .map_err(|error| AdapterError::Worker {
            message: format!("failed to read worker {stream}: {error}"),
        })
}

fn bounded_diagnostic(bytes: &[u8]) -> String {
    let end = bytes.len().min(MAX_WORKER_DIAGNOSTIC_BYTES);
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}
