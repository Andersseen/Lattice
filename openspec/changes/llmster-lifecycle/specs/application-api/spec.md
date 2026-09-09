# Application API Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Extend Checked IPC Contracts For ModelRuntime Lifecycle

Lattice SHALL register `start_model_runtime`, `stop_model_runtime`, and `cancel_model_runtime_operation` in the same checked command inventory used for existing application commands, and SHALL generate matching shared TypeScript bindings.

#### Scenario: Lifecycle Commands Are Generated

- **GIVEN** the Rust command inventory declares the three lifecycle command constants
- **WHEN** the checked command inventory test runs
- **THEN** the Tauri handler registration and the declared constants match exactly
- **AND** generated TypeScript bindings expose the same command names and payload shapes

#### Scenario: Lifecycle Operation Outcome Is Decoded

- **GIVEN** a lifecycle command returns an updated runtime status payload
- **WHEN** Angular decodes the response
- **THEN** unknown or malformed ownership or operation-outcome values decode to a safe bridge error
- **AND** no unhandled variant reaches the UI layer
