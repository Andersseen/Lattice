# Application API Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Extend Checked IPC Contracts For ModelRuntime Discovery

Lattice SHALL expose ModelRuntime discovery through the Rust-owned checked application command inventory and generated shared TypeScript bindings.

#### Scenario: ModelRuntime Commands Are Generated

- **GIVEN** Rust defines ModelRuntime discovery IPC command names
- **WHEN** TypeScript bindings are generated
- **THEN** the committed command inventory includes get, configure, and probe runtime discovery commands

#### Scenario: ModelRuntime Payload Is Decoded

- **GIVEN** Tauri returns a ModelRuntime discovery payload
- **WHEN** Angular receives it through the application API
- **THEN** Angular decodes the availability status and nested daemon/server observations before updating UI state
