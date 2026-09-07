# Application API Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Extend Checked IPC Contracts For Settings

Lattice SHALL expose application settings through the existing checked application command inventory and generated shared TypeScript bindings.

#### Scenario: Settings Commands Are Generated

- **GIVEN** Rust defines application settings IPC command names
- **WHEN** TypeScript bindings are generated
- **THEN** the committed command inventory includes read, update, and reset settings commands

#### Scenario: Settings Payload Is Decoded

- **GIVEN** Tauri returns an application settings payload
- **WHEN** Angular receives it through the application API
- **THEN** Angular decodes schema version, revision, appearance, and idle-unload fields before updating UI state
