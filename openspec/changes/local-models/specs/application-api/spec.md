# Application API Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Extend Checked IPC Contracts For Local Model Management

Lattice SHALL register `get_model_slot_status`, `load_model`, `unload_model`, and `cancel_model_operation` in the same checked command inventory used for existing application commands, and SHALL generate matching shared TypeScript bindings.

#### Scenario: Model Commands Are Generated

- **GIVEN** the Rust command inventory declares the four model-management command constants
- **WHEN** the checked command inventory test runs
- **THEN** the Tauri handler registration and the declared constants match exactly
- **AND** generated TypeScript bindings expose the same command names and payload shapes

#### Scenario: Model Inventory And Outcome Are Decoded

- **GIVEN** a model command returns an updated model slot status payload
- **WHEN** Angular decodes the response
- **THEN** unknown or malformed ownership or operation-outcome values decode to a safe bridge error
- **AND** no unhandled variant reaches the UI layer
