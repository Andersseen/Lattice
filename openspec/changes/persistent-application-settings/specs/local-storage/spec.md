# Local Storage Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Version Local SQLite Schema

Lattice SHALL version its local SQLite schema and migrate settings storage transactionally.

#### Scenario: Migration Succeeds

- **GIVEN** an older supported settings schema exists
- **WHEN** Lattice opens local storage
- **THEN** it creates a pre-migration backup for an existing non-empty database
- **AND** applies the migration in a transaction
- **AND** records the current schema version

#### Scenario: Migration Fails

- **GIVEN** an older supported settings schema exists
- **WHEN** migration fails
- **THEN** the transaction is rolled back
- **AND** existing data is preserved
- **AND** any created backup remains available

#### Scenario: Newer Schema Is Refused

- **GIVEN** local storage has a schema version newer than the running binary supports
- **WHEN** Lattice opens local storage
- **THEN** it refuses to open the database
- **AND** it does not overwrite the schema or settings data

### Requirement: Lattice SHALL Keep Storage Authority In Rust

Lattice SHALL prevent frontend callers from choosing SQL statements, database paths, or storage adapters.

#### Scenario: Frontend Updates Settings

- **GIVEN** Angular submits a settings update
- **WHEN** the update crosses IPC
- **THEN** the request contains only typed settings fields and expected revision
- **AND** Rust chooses the database path and SQL statements
