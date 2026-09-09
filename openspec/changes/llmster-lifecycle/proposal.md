# llmster Lifecycle

## Objective

Start and stop an approved llmster daemon and local HTTP server without disrupting a GUI-owned or otherwise externally attached runtime, and record which resources Lattice itself owns.

## Rationale

0.5 added read-only discovery and explicitly excludes any mutating call. Installed model management (0.7) and every later inference consumer need a reachable running runtime, but llmster's daemon may already be running as an attached GUI process, as a resource a prior Lattice session started, or as an unrelated process Lattice cannot identify. Lifecycle control must distinguish these cases before anything is started or stopped automatically, and must never assume the process ID of the command Lattice runs to invoke the CLI is the daemon's own identity.

## Dependencies

- 0.5 ModelRuntime discovery (`openspec/changes/model-runtime-discovery/`): executable approval, probe adapter, `ModelRuntimeStatus`, loopback health check.
- 0.3 desktop security baseline: native command/window permission model that lifecycle commands must register into.

## Scope

- Add `start_model_runtime` and `stop_model_runtime` checked IPC commands operating only on the executable already approved by 0.5.
- Add a `cancel_model_runtime_operation` command that stops waiting on an in-flight start/stop without assuming the underlying resource can be rolled back.
- Introduce `RuntimeOwnership` (`owned`, `attached`, `unknown`) recorded per resource and persisted so it can be reconciled, not assumed, across restarts.
- Perform a bounded health wait after start, reusing the 0.5 loopback probe, before reporting a resource `running`.
- Make start and stop idempotent: starting an already-running resource never spawns a second process or reclaims attached ownership; stopping an attached or unknown resource is refused outright, never attempted.
- Reconcile daemonization: trust only the PID llmster's own `--json` output reports for a started daemon, never the PID of the short-lived process Lattice spawns to invoke `lms daemon up`.
- Add a best-effort shutdown hook that stops only currently-owned resources when Lattice exits.
- Produce the P1 resource/lifecycle profile: ten start/stop cycles comparing no-runtime, attached-runtime, and owned-idle-runtime states, confirming no orphaned or duplicate daemon remains.

## Non-goals

- No model list, load, unload, or inference (0.7 and later).
- No system/login service registration, "start on boot," or any unattended start outside an explicit user action.
- No arbitrary terminal execution or general process-supervisor platform; 0.17 foreground command execution is a distinct, narrower authorization surface introduced later.
- No automatic reconnection/retry loop, background polling service, or eager runtime start on application launch.
- No stopping a process by name or port guess; only a resource this feature can still verify it started.
- No non-loopback bind address or CORS enablement for the local server.

## Impacted Capabilities

- `model-runtime`
- `application-api`
- `local-storage`
