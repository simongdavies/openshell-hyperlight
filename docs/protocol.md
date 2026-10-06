# JSON protocol version 1

The `execute` command accepts one UTF-8 JSON object on stdin. Unknown fields are
errors. Sizes are bytes.

```json
{
  "schema_version": 1,
  "code": "print('hello')",
  "capabilities": {
    "network": [
      {
        "origin": "https://example.com",
        "methods": ["GET"]
      }
    ],
    "filesystem": {
      "input_directory": "/workspace/input",
      "output_directory": "/workspace/output",
      "output_limits": {
        "max_file_size_bytes": 1048576,
        "max_total_size_bytes": 4194304,
        "max_file_count": 16
      }
    },
    "tools": []
  },
  "limits": {
    "timeout_ms": 30000,
    "max_stdout_bytes": 65536,
    "max_stderr_bytes": 65536,
    "heap_bytes": 67108864,
    "stack_bytes": 134217728
  }
}
```

## Capability rules

- Missing `network` and an empty list both deny network access.
- An origin must contain only `http` or `https`, host, and optional port.
  Paths, query strings, fragments, credentials, wildcards, `CONNECT`, and
  `TRACE` are unsupported.
- Methods are explicit and non-empty.
- Missing `filesystem` denies filesystem access.
- Input and output directories must exist, be absolute, and not overlap.
- Output directories must be empty. Hyperlight writes to an adapter-owned
  temporary directory; the adapter then copies new files into the requested
  directory without recursively deleting caller-selected paths.
- Output limits are mandatory whenever output is mapped.
- `tools` must be empty. This makes unsupported tool grants fail closed.

## Execution limits

- `timeout_ms`: 1 through 300,000. The parent kills the complete worker
  process after this wall-clock duration.
- stdout/stderr capture: 0 through 16 MiB each. A zero limit returns an empty,
  truncated string when guest output is present.
- heap: 16 MiB through 4 GiB.
- stack/scratch: 64 MiB plus 64 KiB through 4 GiB. The lower bound covers
  fixed input/output buffers in the pinned Wasm backend.
- Python source: non-empty and at most 1 MiB.

Capture limits apply when Hyperlight returns its output strings. They bound the
JSON response but do not prevent the guest/runtime from allocating output
before return.

## Error codes

| Code | Meaning |
|---|---|
| `invalid_json` | Stdin was not valid version-1 JSON. |
| `invalid_request` | A field or capability violated the contract. |
| `worker_failed` | The isolated worker could not start, communicate, or exit cleanly. |
| `hyperlight_failed` | Hyperlight construction or guest execution failed. |
| `unsupported` | The binary lacks Linux/Hyperlight execution support. |
| `execution_timeout` | The worker exceeded `timeout_ms` and was terminated. |
