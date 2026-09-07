# application-api Specification

## Purpose

Define the checked Angular-to-Tauri application boundary for current foundation commands, including Rust-owned command names, generated shared TypeScript bindings, runtime decoding, safe error normalization, and metadata refresh ordering.

## Requirements

### Requirement: Lattice SHALL Check Application IPC Contracts

Lattice SHALL keep application command names and native response DTOs in a Rust-owned contract that is committed to shared TypeScript bindings and checked for drift in automated tests.

#### Scenario: Generated Contract Drift Is Detected

- **GIVEN** a Rust-owned application wire contract changes
- **WHEN** the committed TypeScript bindings are not regenerated
- **THEN** contract verification fails

#### Scenario: Angular Uses Checked Command Names

- **GIVEN** Angular requests application metadata
- **WHEN** it invokes the native command
- **THEN** the command name comes from the generated application command inventory

### Requirement: Lattice SHALL Decode Native Application Metadata

Lattice SHALL treat Tauri IPC payloads as untrusted data and decode native application metadata before application state consumes it.

#### Scenario: Native Metadata Payload Is Valid

- **GIVEN** Tauri returns native application metadata with `runtime` set to `tauri`
- **WHEN** Angular decodes the payload
- **THEN** the typed application API returns that metadata

#### Scenario: Native Metadata Payload Is Malformed

- **GIVEN** Tauri returns malformed application metadata
- **WHEN** Angular decodes the payload
- **THEN** the typed application API raises one safe bridge error

### Requirement: Lattice SHALL Normalize Application Boundary Errors Once

Lattice SHALL normalize unknown bridge failures and structured app errors through one safe application boundary normalizer before stores consume them.

#### Scenario: Unknown Error Is Raised

- **GIVEN** native metadata cannot be read and the failure is not a structured app error
- **WHEN** Angular handles the failure
- **THEN** the visible error has code `bridge.unknown`
- **AND** the message does not expose raw internal error text

### Requirement: Lattice SHALL Preserve Latest Metadata Refresh

Lattice SHALL prevent earlier metadata refreshes from overwriting a newer refresh result.

#### Scenario: Earlier Refresh Finishes Last

- **GIVEN** two metadata refreshes are in flight
- **WHEN** the newer refresh succeeds before the older refresh
- **THEN** the store keeps the newer application metadata
- **AND** the older refresh does not change loading or error state
