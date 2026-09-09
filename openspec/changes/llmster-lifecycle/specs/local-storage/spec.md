# Local Storage Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Persist ModelRuntime Ownership

Lattice SHALL persist the recorded runtime ownership state, owned daemon process identity, and owned-since timestamp so ownership can be reconciled against a fresh probe after restart instead of being reassumed.

#### Scenario: Ownership Survives Restart When Verified

- **GIVEN** a persisted `owned` ownership record
- **WHEN** the application restarts and a fresh probe reports a matching process identity and executable fingerprint
- **THEN** Lattice restores ownership `owned`

#### Scenario: Unverifiable Ownership Downgrades To Unknown

- **GIVEN** a persisted `owned` ownership record
- **WHEN** the application restarts and a fresh probe reports a different process identity, a different executable fingerprint, or no running daemon
- **THEN** Lattice stores ownership `unknown`
- **AND** does not treat the observed daemon as stoppable by Lattice

### Requirement: Lattice SHALL Migrate Local Storage For ModelRuntime Ownership

Lattice SHALL migrate the settings schema to add ownership columns using the existing transactional, backed-up migration path, without altering unrelated stored settings.

#### Scenario: Settings Storage Migrates To Ownership Schema

- **GIVEN** a database at the prior schema version
- **WHEN** Lattice starts and applies the pending migration
- **THEN** Lattice creates a pre-migration backup
- **AND** applies the schema change in one transaction
- **AND** preserves existing settings and runtime discovery metadata unchanged
