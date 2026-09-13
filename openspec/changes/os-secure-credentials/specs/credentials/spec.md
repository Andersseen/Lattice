# Credentials Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Store Only A Credential Reference, Never The Secret Value, In Application Storage

Lattice SHALL persist a credential's label, provider key, and timestamps in its own SQLite storage, and SHALL NOT persist the secret value anywhere outside the OS keychain.

#### Scenario: Application Storage Contains No Secret Bytes

- **GIVEN** a credential has been created with a secret value
- **WHEN** Lattice's local SQLite storage is inspected directly
- **THEN** the entered secret value does not appear anywhere in it

### Requirement: Lattice SHALL Acquire A New Or Replacement Secret Through Native Entry Only

Lattice SHALL collect a credential's secret value only through a native, non-WebView prompt, and SHALL NOT expose any interface for the frontend to supply, read, or display that value.

#### Scenario: No Command Returns A Secret Value

- **GIVEN** any credential command available to the frontend
- **WHEN** its response shape is inspected
- **THEN** no field of that response carries the secret value

#### Scenario: Cancelling Native Entry Leaves No Trace

- **GIVEN** a user is prompted to enter a secret for a new or replaced credential
- **WHEN** the user cancels the prompt
- **THEN** Lattice creates or modifies no stored reference and no keychain entry

### Requirement: Lattice SHALL Report One Of Four Honest Availability States Per Credential

Lattice SHALL report each credential's availability as exactly one of available, locked, missing, or unsupported, reflecting the real state of the OS keychain rather than an assumed or cached value.

#### Scenario: An Unlocked, Present Secret Is Available

- **GIVEN** a credential's keychain entry exists and the keychain is unlocked
- **WHEN** the credential is listed
- **THEN** its availability is reported as available

#### Scenario: A Locked Keychain Is Reported, Not Silently Treated As Missing

- **GIVEN** the OS keychain is locked
- **WHEN** credentials are listed
- **THEN** every credential's availability is reported as locked
- **AND** Lattice does not report locked entries as missing or attempt a plaintext fallback

#### Scenario: A Keychain Entry Removed Outside Lattice Is Reported As Missing

- **GIVEN** a credential's stored reference exists but its keychain entry has been removed outside Lattice
- **WHEN** the credential is listed
- **THEN** its availability is reported as missing

#### Scenario: An Unsupported Platform Reports Every Credential As Unsupported And Never Falls Back

- **GIVEN** Lattice is running on a platform without a supported secure credential backend
- **WHEN** a credential is created, listed, or replaced
- **THEN** the operation reports unsupported
- **AND** no secret value is written in plaintext anywhere

### Requirement: Lattice SHALL Roll Back Partial Writes Between The Keychain And Application Storage

Lattice SHALL ensure that a credential reference exists in application storage if and only if its secret was successfully written to the keychain.

#### Scenario: A Keychain Write Failure Leaves No Orphaned Reference

- **GIVEN** writing a newly entered secret to the keychain fails
- **WHEN** Lattice would otherwise record the credential's reference
- **THEN** no reference row is created

#### Scenario: Deleting A Credential Removes Both Its Reference And Its Keychain Entry

- **GIVEN** a credential with both a stored reference and a keychain entry exists
- **WHEN** the credential is deleted
- **THEN** the reference no longer appears in listings
- **AND** its keychain entry no longer exists

### Requirement: Lattice SHALL Resolve A Credential's Secret For Rust-Internal Use Only

Lattice SHALL provide a way for Rust to resolve an authorized credential reference to its secret value for its own internal use, and SHALL NOT expose that resolution through any interface reachable by the frontend.

#### Scenario: The Frontend Has No Path To The Resolved Secret

- **GIVEN** a credential reference is resolvable by Rust
- **WHEN** the full set of frontend-reachable commands is inspected
- **THEN** none of them return the resolved secret value
