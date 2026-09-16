# Application API Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Extend Checked IPC Contracts For Remote Provider Profiles

Lattice SHALL register the remote provider profile commands in the same checked command inventory and permission set used for existing application commands, and SHALL generate matching TypeScript bindings for the profile, its requests, and the chat target.

#### Scenario: Provider Profile Commands Are Generated And Permitted

- **GIVEN** the Rust command inventory declares the provider profile command constants
- **WHEN** the checked command inventory and capability tests run
- **THEN** the Tauri handler registration, the declared constants and the main window's permissions match exactly
- **AND** generated TypeScript bindings expose the same command names, `ProviderProfile`, its request shapes and the `ChatTarget` union

#### Scenario: Provider Profiles Are Decoded Defensively

- **GIVEN** a provider profile payload reaches Angular
- **WHEN** it is malformed or carries an unknown shape
- **THEN** Angular reports one safe bridge error instead of passing the payload to the UI
