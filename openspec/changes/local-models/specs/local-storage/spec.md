# Local Storage Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Persist Model Load Ownership

Lattice SHALL persist the recorded model load ownership state, owned identifier, owned model key, and owned-since timestamp so ownership can be reconciled against a fresh loaded-inventory probe after restart instead of being reassumed.

#### Scenario: Load Ownership Survives Restart When Verified

- **GIVEN** a persisted `owned` load ownership record
- **WHEN** the application restarts and a fresh loaded-inventory probe reports a matching identifier and model key
- **THEN** Lattice restores ownership `owned`

#### Scenario: Unverifiable Load Ownership Downgrades To Unknown

- **GIVEN** a persisted `owned` load ownership record
- **WHEN** the application restarts and a fresh loaded-inventory probe reports a different identifier, a different model key, or no loaded model
- **THEN** Lattice stores ownership `unknown`
- **AND** does not treat any currently loaded model as unloadable by Lattice on the strength of the stale record

### Requirement: Lattice SHALL Migrate Local Storage For Model Load Ownership

Lattice SHALL migrate the settings schema to add a model load state row using the existing transactional, backed-up migration path, without altering unrelated stored settings.

#### Scenario: Settings Storage Migrates To Model-Load Schema

- **GIVEN** a database at schema version 3
- **WHEN** Lattice starts and applies the pending migration
- **THEN** Lattice creates a pre-migration backup
- **AND** applies the schema change to version 4 in one transaction
- **AND** preserves existing settings, runtime discovery, and runtime ownership metadata unchanged
