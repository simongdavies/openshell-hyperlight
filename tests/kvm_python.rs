#![cfg(all(target_os = "linux", feature = "hyperlight"))]

use std::path::PathBuf;

use assert_cmd::Command;
use openshell_hyperlight::{
    Capabilities, ExecuteRequest, ExecutionLimits, FilesystemCapabilities, FilesystemLimits,
    HttpMethod, NetworkCapability, ResponseStatus, SCHEMA_VERSION, worker_main,
};
use tempfile::TempDir;

#[test]
#[ignore = "requires /dev/kvm and HYPERLIGHT_PYTHON_GUEST"]
fn executes_python_through_hyperlight() -> anyhow::Result<()> {
    let guest = PathBuf::from(std::env::var("HYPERLIGHT_PYTHON_GUEST")?);
    let request = ExecuteRequest {
        schema_version: SCHEMA_VERSION,
        code: "print(sum(i * i for i in range(5)))".to_string(),
        capabilities: Capabilities::default(),
        limits: limits(30_000),
    };

    let response = worker_main(&guest, &serde_json::to_vec(&request)?)?;

    assert_eq!(
        (
            response.status,
            response.execution.map(|output| output.stdout)
        ),
        (ResponseStatus::Ok, Some("30\n".to_string()))
    );
    Ok(())
}

#[test]
#[ignore = "requires /dev/kvm and HYPERLIGHT_PYTHON_GUEST"]
fn terminates_python_at_wall_clock_deadline() -> anyhow::Result<()> {
    let guest = std::env::var("HYPERLIGHT_PYTHON_GUEST")?;
    let request = ExecuteRequest {
        schema_version: SCHEMA_VERSION,
        code: "while True:\n    pass".to_string(),
        capabilities: Capabilities::default(),
        limits: limits(5_000),
    };

    let output = Command::cargo_bin("openshell-hyperlight")?
        .args(["execute", "--python-guest", &guest])
        .write_stdin(serde_json::to_vec(&request)?)
        .output()?;
    let response = serde_json::from_slice::<serde_json::Value>(&output.stdout)?;

    assert_eq!(response["status"], "timeout");
    Ok(())
}

#[test]
#[ignore = "requires /dev/kvm and HYPERLIGHT_PYTHON_GUEST"]
fn maps_explicit_filesystem_capabilities() -> anyhow::Result<()> {
    let guest = PathBuf::from(std::env::var("HYPERLIGHT_PYTHON_GUEST")?);
    let input = TempDir::new()?;
    let output = TempDir::new()?;
    std::fs::write(input.path().join("message.txt"), "hello")?;
    let request = ExecuteRequest {
        schema_version: SCHEMA_VERSION,
        code: concat!(
            "with open('/input/message.txt') as source:\n",
            "    value = source.read()\n",
            "with open('/output/result.txt', 'w') as target:\n",
            "    target.write(value.upper())\n"
        )
        .to_string(),
        capabilities: Capabilities {
            network: Vec::new(),
            filesystem: Some(FilesystemCapabilities {
                input_directory: Some(input.path().to_path_buf()),
                output_directory: Some(output.path().to_path_buf()),
                output_limits: Some(FilesystemLimits {
                    max_file_size_bytes: 1_024,
                    max_total_size_bytes: 1_024,
                    max_file_count: 1,
                }),
            }),
            tools: Vec::new(),
        },
        limits: limits(30_000),
    };

    let response = worker_main(&guest, &serde_json::to_vec(&request)?)?;

    assert_eq!(
        (
            response.status,
            std::fs::read_to_string(output.path().join("result.txt"))?
        ),
        (ResponseStatus::Ok, "HELLO".to_string())
    );
    Ok(())
}

#[test]
#[ignore = "requires /dev/kvm, network access, and HYPERLIGHT_PYTHON_GUEST"]
fn permits_only_explicit_network_origin_and_method() -> anyhow::Result<()> {
    let guest = PathBuf::from(std::env::var("HYPERLIGHT_PYTHON_GUEST")?);
    let request = ExecuteRequest {
        schema_version: SCHEMA_VERSION,
        code: concat!(
            "response = http_get('https://example.com/')\n",
            "print(response['status'])\n"
        )
        .to_string(),
        capabilities: Capabilities {
            network: vec![NetworkCapability {
                origin: "https://example.com".to_string(),
                methods: vec![HttpMethod::Get],
            }],
            filesystem: None,
            tools: Vec::new(),
        },
        limits: limits(30_000),
    };

    let response = worker_main(&guest, &serde_json::to_vec(&request)?)?;

    assert_eq!(
        response.execution.map(|output| output.stdout),
        Some("200\n".to_string())
    );
    Ok(())
}

const fn limits(timeout_ms: u64) -> ExecutionLimits {
    ExecutionLimits {
        timeout_ms,
        max_stdout_bytes: 1_024,
        max_stderr_bytes: 1_024,
        heap_bytes: 64 * 1024 * 1024,
        stack_bytes: 128 * 1024 * 1024,
    }
}
