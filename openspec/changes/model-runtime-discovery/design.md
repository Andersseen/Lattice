# Model Runtime Discovery Design

## Ownership

`lattice-core` owns the ModelRuntime discovery contract, executable path validation, probe execution limits, CLI output parsing, endpoint loopback health, storage schema, and safe errors. `lattice-desktop` exposes typed commands and no direct process or network authority to Angular. Angular owns only setup/status presentation and calls the application API.

## Contracts

Commands:

- `get_model_runtime_status` returns the persisted runtime discovery status without executing the configured executable.
- `configure_model_runtime` accepts an expected runtime revision and one absolute executable path. It validates the path shape, persists it, clears prior approval metadata, and does not execute it.
- `probe_model_runtime` accepts an expected runtime revision and executes only the configured absolute path with fixed read-only argument arrays.

The top-level status values are `missing`, `unsupported`, `stopped`, `running`, `unreachable`, and `unknown`. Daemon and server observations are separate so a daemon can be present while the API server is stopped.

## Probes

The llmster adapter invokes:

- `<executable> --version`
- `<executable> daemon status --json`
- `<executable> server status --json --quiet`

Each probe uses a fixed argv list, no shell, a 2 second timeout, and bounded captured output. The implementation parses only the small documented JSON shapes from current LM Studio docs. It does not call `lms daemon up`, `lms server start`, `lms get`, `lms load`, or any other mutating command.

The minimum recognized CLI version for 0.5 is `0.0.47`, matching the current LM Studio CLI documentation example checked on 2026-09-07. Older parsed versions produce `unsupported`; unparseable or malformed outputs produce `unknown`.

## Endpoint Health

The adapter accepts only loopback server observations. Server status JSON provides a port; Lattice constructs `http://127.0.0.1:<port>` and attempts a bounded TCP connection to `127.0.0.1:<port>`. It follows no redirects and accepts no remote host from the frontend.

## Storage

The existing SQLite database migrates from schema version 1 to version 2 by adding one `model_runtime_discovery` row. The row stores runtime revision, configured path, approved executable fingerprint, approved CLI version, daemon/server observations, endpoint, last checked timestamp, and safe message.

Executable identity is represented by canonical path, byte length, and modified timestamp. If the file disappears or changes after approval, non-executing status reads mark the runtime as requiring a new explicit probe.

## UI

The Models route shows the configured executable path, top-level status, daemon status, server status, endpoint, CLI version, last checked time, and safe error messages. Configure saves an absolute path only. Probe is a distinct button and is the user's explicit approval to execute the configured path once for read-only status.

Browser smoke mode uses an in-memory fallback and does not claim native discovery evidence.

## Documentation Sources

LM Studio docs checked on 2026-09-07:

- `lms` CLI overview: `https://lmstudio.ai/docs/cli`
- daemon status JSON: `https://lmstudio.ai/docs/cli/daemon/daemon-status`
- server status JSON: `https://lmstudio.ai/docs/cli/serve/server-status`
- REST/server overview: `https://lmstudio.ai/docs/developer/rest/endpoints`

## Verification

Fixture executables cover missing paths, relative paths, unsupported versions, malformed JSON, timeout, stopped daemon/server, and unreachable/running loopback endpoint states. UI verification covers configure/probe flows through browser fallback. Real llmster smoke is opt-in evidence and not required for ordinary PR checks.
