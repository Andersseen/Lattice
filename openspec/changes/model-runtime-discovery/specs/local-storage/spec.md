# Local Storage Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Persist ModelRuntime Discovery Metadata

Lattice SHALL persist configured runtime executable path and approved discovery metadata in Rust-owned SQLite storage.

#### Scenario: Runtime Metadata Survives Restart

- **GIVEN** a runtime executable path and successful probe metadata are stored
- **WHEN** Lattice reopens local storage
- **THEN** reading runtime discovery status returns the stored path, approval metadata, and last observation

#### Scenario: Changed Executable Requires New Approval

- **GIVEN** a runtime executable path has approved metadata
- **WHEN** the executable identity changes before a later status read
- **THEN** Lattice reports that a new probe approval is required
- **AND** does not execute the changed executable during the read

### Requirement: Lattice SHALL Migrate Local Storage For ModelRuntime Discovery

Lattice SHALL migrate local SQLite storage for runtime discovery transactionally without weakening existing settings migration guarantees.

#### Scenario: Settings Storage Migrates To Runtime Discovery Schema

- **GIVEN** a supported settings schema exists without runtime discovery metadata
- **WHEN** Lattice opens local storage
- **THEN** it creates a pre-migration backup for an existing non-empty database
- **AND** adds runtime discovery storage in a transaction
- **AND** preserves existing application settings
