use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Component, Path};

use hyperlight_sandbox::{
    FilesystemLimits as HyperlightFilesystemLimits, HttpMethod as HyperMethod, SandboxBuilder,
};
use hyperlight_wasm_sandbox::Wasm;

use crate::{
    AdapterError, ExecuteRequest, ExecuteResponse, ExecutionOutput, HttpMethod, ResponseStatus,
    SCHEMA_VERSION,
};

pub(crate) fn execute(
    python_guest: &Path,
    request: &ExecuteRequest,
) -> Result<ExecuteResponse, AdapterError> {
    if !python_guest.is_absolute() || !python_guest.is_file() {
        return Err(AdapterError::invalid(
            "python guest module must be an existing absolute file",
        ));
    }
    let mut builder = SandboxBuilder::new()
        .module_path(python_guest.display().to_string())
        .heap_size(request.limits.heap_bytes)
        .stack_size(request.limits.stack_bytes);

    if let Some(filesystem) = &request.capabilities.filesystem {
        if let Some(input) = &filesystem.input_directory {
            builder = builder.input_dir(input);
        }
        if let (Some(_output), Some(limits)) =
            (&filesystem.output_directory, filesystem.output_limits)
        {
            let limits = HyperlightFilesystemLimits::new(
                limits.max_file_size_bytes,
                limits.max_total_size_bytes,
                limits.max_file_count,
            )
            .map_err(|error| AdapterError::Hyperlight {
                message: format!("invalid Hyperlight filesystem limits: {error:#}"),
            })?;
            builder = builder.filesystem_limits(limits).temp_output();
        }
    }

    let mut sandbox = builder
        .guest(Wasm)
        .build()
        .map_err(|error| AdapterError::Hyperlight {
            message: format!("failed to create sandbox: {error:#}"),
        })?;
    for permission in &request.capabilities.network {
        let methods = permission
            .methods
            .iter()
            .copied()
            .map(to_hyperlight_method)
            .collect::<Vec<_>>();
        sandbox
            .allow_domain(&permission.origin, methods)
            .map_err(|error| AdapterError::Hyperlight {
                message: format!(
                    "failed to grant network origin {}: {error:#}",
                    permission.origin
                ),
            })?;
    }

    let result = sandbox
        .run(&request.code)
        .map_err(|error| AdapterError::Hyperlight {
            message: format!("guest execution failed: {error:#}"),
        })?;
    let output_files = sandbox
        .get_output_files()
        .map_err(|error| AdapterError::Hyperlight {
            message: format!("failed to list output files: {error:#}"),
        })?;
    if let Some(output_directory) = request
        .capabilities
        .filesystem
        .as_ref()
        .and_then(|filesystem| filesystem.output_directory.as_deref())
    {
        let sandbox_output = sandbox
            .output_path()
            .map_err(|error| AdapterError::Hyperlight {
                message: format!("failed to resolve sandbox output path: {error:#}"),
            })?
            .ok_or_else(|| AdapterError::Hyperlight {
                message: "Hyperlight did not create the requested output directory".to_string(),
            })?;
        copy_output_entries(&sandbox_output, output_directory, &output_files)?;
    }
    let (stdout, stdout_truncated) = truncate_utf8(result.stdout, request.limits.max_stdout_bytes);
    let (stderr, stderr_truncated) = truncate_utf8(result.stderr, request.limits.max_stderr_bytes);
    let status = if result.exit_code == 0 {
        ResponseStatus::Ok
    } else {
        ResponseStatus::GuestError
    };
    Ok(ExecuteResponse {
        schema_version: SCHEMA_VERSION,
        status,
        execution: Some(ExecutionOutput {
            stdout,
            stdout_truncated,
            stderr,
            stderr_truncated,
            exit_code: result.exit_code,
            output_files,
        }),
        error: None,
    })
}

fn copy_output_entries(
    source: &Path,
    destination: &Path,
    names: &[String],
) -> Result<(), AdapterError> {
    for name in names {
        let mut components = Path::new(name).components();
        if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
            return Err(AdapterError::Hyperlight {
                message: format!("guest returned invalid output entry name {name:?}"),
            });
        }
        copy_output_entry(&source.join(name), &destination.join(name))?;
    }
    Ok(())
}

fn copy_output_entry(source: &Path, destination: &Path) -> Result<(), AdapterError> {
    let metadata = source
        .symlink_metadata()
        .map_err(|error| output_error("inspect", source, &error))?;
    if metadata.file_type().is_symlink() {
        return Err(AdapterError::Hyperlight {
            message: format!(
                "guest output contains unsupported symlink {}",
                source.display()
            ),
        });
    }
    if metadata.is_dir() {
        std::fs::create_dir(destination)
            .map_err(|error| output_error("create directory", destination, &error))?;
        let entries =
            std::fs::read_dir(source).map_err(|error| output_error("read", source, &error))?;
        for entry in entries {
            let entry = entry.map_err(|error| output_error("read", source, &error))?;
            copy_output_entry(&entry.path(), &destination.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if !metadata.is_file() {
        return Err(AdapterError::Hyperlight {
            message: format!(
                "guest output contains unsupported file type {}",
                source.display()
            ),
        });
    }
    let mut input = File::open(source).map_err(|error| output_error("open", source, &error))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| output_error("create", destination, &error))?;
    io::copy(&mut input, &mut output)
        .map_err(|error| output_error("copy to", destination, &error))?;
    Ok(())
}

fn output_error(operation: &str, path: &Path, error: &io::Error) -> AdapterError {
    AdapterError::Hyperlight {
        message: format!(
            "failed to {operation} output path {}: {error}",
            path.display()
        ),
    }
}

const fn to_hyperlight_method(method: HttpMethod) -> HyperMethod {
    match method {
        HttpMethod::Get => HyperMethod::Get,
        HttpMethod::Post => HyperMethod::Post,
        HttpMethod::Put => HyperMethod::Put,
        HttpMethod::Delete => HyperMethod::Delete,
        HttpMethod::Head => HyperMethod::Head,
        HttpMethod::Options => HyperMethod::Options,
        HttpMethod::Patch => HyperMethod::Patch,
    }
}

fn truncate_utf8(mut value: String, limit: u64) -> (String, bool) {
    let Ok(limit) = usize::try_from(limit) else {
        return (value, false);
    };
    if value.len() <= limit {
        return (value, false);
    }
    let mut end = limit;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    (value, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_at_utf8_boundary() {
        assert_eq!(truncate_utf8("aéz".to_string(), 2), ("a".to_string(), true));
    }
}
