# Application Settings Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Persist Non-Secret Application Settings

Lattice SHALL store validated non-secret application settings in Rust-owned local storage and return them through typed application IPC.

#### Scenario: Settings Are Read

- **GIVEN** the desktop app starts with no settings database
- **WHEN** Angular requests application settings
- **THEN** Rust creates the current schema under the OS app-data directory
- **AND** returns default settings with schema version and revision

#### Scenario: Settings Survive Restart

- **GIVEN** a user saves valid application settings
- **WHEN** the desktop app is restarted
- **THEN** reading settings returns the saved values

### Requirement: Lattice SHALL Validate Settings Before Persistence

Lattice SHALL reject invalid settings updates before writing them and preserve the previous revision.

#### Scenario: Invalid Setting Is Rejected

- **GIVEN** current settings exist
- **WHEN** an update contains an unsupported appearance value or idle-unload value outside the accepted range
- **THEN** the update fails with a safe settings error
- **AND** the stored settings and revision remain unchanged

#### Scenario: Revision Conflict Is Rejected

- **GIVEN** current settings are at revision `N`
- **WHEN** an update or reset is submitted with a different expected revision
- **THEN** the write fails with a recoverable conflict error
- **AND** revision `N` remains stored

### Requirement: Lattice SHALL Expose A Minimal Settings UI

Lattice SHALL provide a settings view that can read, edit, save, reset, retry after error, and display application settings without exposing storage internals.

#### Scenario: User Saves Settings

- **GIVEN** settings are loaded in the Settings view
- **WHEN** the user changes appearance or idle-unload preferences and saves
- **THEN** Angular sends a typed settings update request
- **AND** the view displays the saved settings revision

#### Scenario: Storage Failure Is Recoverable In UI

- **GIVEN** settings cannot be read or saved
- **WHEN** the Settings view receives a safe application error
- **THEN** it displays the safe error message
- **AND** offers a retry path without exposing SQL, filesystem paths, or raw internal details
