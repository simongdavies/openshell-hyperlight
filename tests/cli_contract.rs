use assert_cmd::Command;
use pretty_assertions::assert_eq;
use serde_json::Value;

#[test]
fn invalid_json_returns_structured_error() -> anyhow::Result<()> {
    let output = Command::cargo_bin("openshell-hyperlight")?
        .args(["execute", "--python-guest", "/missing/python.aot"])
        .write_stdin("{")
        .output()?;

    assert_eq!(output.status.code(), Some(1));
    let response = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(response["status"], "error");
    assert_eq!(response["error"]["code"], "invalid_json");
    Ok(())
}

#[test]
fn missing_argument_returns_structured_error() -> anyhow::Result<()> {
    let output = Command::cargo_bin("openshell-hyperlight")?
        .args(["execute"])
        .write_stdin("{}")
        .output()?;

    assert_eq!(output.status.code(), Some(1));
    let response = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(response["error"]["code"], "invalid_request");
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn direct_worker_invocation_is_rejected() -> anyhow::Result<()> {
    let output = Command::cargo_bin("openshell-hyperlight")?
        .args(["__worker", "--python-guest", "/missing/python.aot"])
        .env("OPENSHELL_HYPERLIGHT_INTERNAL_WORKER", "1")
        .write_stdin("{}")
        .output()?;

    assert_eq!(output.status.code(), Some(1));
    let response = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(response["error"]["code"], "invalid_request");
    Ok(())
}
