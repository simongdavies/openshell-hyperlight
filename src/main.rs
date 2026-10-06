use std::io::{Read, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand, error::ErrorKind};
use openshell_hyperlight::{
    AdapterError, ExecuteRequest, ExecuteResponse, ResponseStatus, execute_with_timeout,
    worker_main,
};

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            let _ = error.print();
            return;
        }
        Err(error) => {
            let response = ExecuteResponse::from_error(&AdapterError::InvalidRequest {
                message: error.to_string(),
            });
            let _ = write_response(&response);
            std::process::exit(1);
        }
    };
    let (response, exit_code) = match run(cli) {
        Ok(response) => {
            let exit_code = i32::from(response.status == ResponseStatus::Error);
            (response, exit_code)
        }
        Err(error) => {
            let response = ExecuteResponse::from_error(&error);
            (response, 1)
        }
    };
    if write_response(&response).is_err() {
        std::process::exit(2);
    }
    std::process::exit(exit_code);
}

fn run(cli: Cli) -> Result<ExecuteResponse, AdapterError> {
    let request_json = read_stdin()?;
    let response = match cli.command {
        Command::Execute { python_guest } => {
            let request = serde_json::from_slice::<ExecuteRequest>(&request_json)
                .map_err(|source| AdapterError::InvalidJson { source })?;
            let current_executable =
                std::env::current_exe().map_err(|error| AdapterError::Worker {
                    message: format!("failed to resolve current executable: {error}"),
                })?;
            execute_with_timeout(&current_executable, &python_guest, &request)?
        }
        Command::Worker { python_guest } => {
            verify_worker_invocation()?;
            worker_main(&python_guest, &request_json)?
        }
    };
    Ok(response)
}

#[cfg(target_os = "linux")]
fn verify_worker_invocation() -> Result<(), AdapterError> {
    if std::env::var_os("OPENSHELL_HYPERLIGHT_INTERNAL_WORKER").is_none() {
        return Err(AdapterError::InvalidRequest {
            message: "internal worker invocation rejected".to_string(),
        });
    }
    let status =
        std::fs::read_to_string("/proc/self/status").map_err(|error| AdapterError::Worker {
            message: format!("failed to read worker process metadata: {error}"),
        })?;
    let parent_id = status
        .lines()
        .find_map(|line| line.strip_prefix("PPid:"))
        .and_then(|value| value.trim().parse::<u32>().ok())
        .ok_or_else(|| AdapterError::Worker {
            message: "failed to determine worker parent process".to_string(),
        })?;
    let parent_executable =
        std::fs::read_link(format!("/proc/{parent_id}/exe")).map_err(|error| {
            AdapterError::Worker {
                message: format!("failed to identify worker parent executable: {error}"),
            }
        })?;
    let current_executable = std::env::current_exe().map_err(|error| AdapterError::Worker {
        message: format!("failed to identify worker executable: {error}"),
    })?;
    if parent_executable != current_executable {
        return Err(AdapterError::InvalidRequest {
            message: "internal worker invocation rejected".to_string(),
        });
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn verify_worker_invocation() -> Result<(), AdapterError> {
    Err(AdapterError::Unsupported {
        message: "internal worker execution is supported only on Linux".to_string(),
    })
}

#[derive(Debug, Parser)]
#[command(
    name = "openshell-hyperlight",
    version,
    about = "Execute one JSON-described Python workload in Hyperlight on Linux/KVM"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Read one execution request from stdin and write one response to stdout.
    Execute {
        /// Absolute path to the trusted Hyperlight Python AOT guest module.
        #[arg(long)]
        python_guest: PathBuf,
    },
    #[command(name = "__worker", hide = true)]
    Worker {
        #[arg(long)]
        python_guest: PathBuf,
    },
}

fn read_stdin() -> Result<Vec<u8>, AdapterError> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .read_to_end(&mut bytes)
        .map_err(|error| AdapterError::Worker {
            message: format!("failed to read stdin: {error}"),
        })?;
    Ok(bytes)
}

fn write_response(response: &ExecuteResponse) -> Result<(), AdapterError> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, response).map_err(|error| AdapterError::Worker {
        message: format!("failed to serialize response: {error}"),
    })?;
    stdout
        .write_all(b"\n")
        .map_err(|error| AdapterError::Worker {
            message: format!("failed to write response: {error}"),
        })
}
