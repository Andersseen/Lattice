# Desktop Security Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Enforce A Restrictive Production CSP

Lattice SHALL configure a non-null Content Security Policy for bundled desktop assets that permits only trusted local application resources and the required Tauri IPC transport.

#### Scenario: Production Shell Loads With CSP

- **GIVEN** a production desktop build is running from bundled Angular assets
- **WHEN** the main window renders Home and System views
- **THEN** the WebView has a non-null CSP
- **AND** application metadata IPC still succeeds
- **AND** the UI does not require `unsafe-eval`

#### Scenario: Remote Script Is Blocked

- **GIVEN** a production desktop WebView is running
- **WHEN** content attempts to load a remote executable script
- **THEN** the load is blocked by CSP
- **AND** the blocked load is recorded as verification evidence

#### Scenario: Remote Connection Is Blocked

- **GIVEN** a production desktop WebView is running
- **WHEN** content attempts to connect to an unapproved remote origin
- **THEN** the connection is blocked by CSP
- **AND** the required IPC transport remains available

### Requirement: Lattice SHALL Separate Development And Production Allowances

Lattice SHALL keep dev-server, hot-reload, and local debugging allowances out of the production CSP and production capability baseline.

#### Scenario: Development Server Runs Separately

- **GIVEN** a developer starts the desktop app against the Angular dev server
- **WHEN** development-only allowances are needed for local iteration
- **THEN** those allowances are scoped to dev configuration or documented dev-only behavior
- **AND** the production build retains the restrictive CSP

### Requirement: Lattice SHALL Restrict Application Command And Window Authority

Lattice SHALL restrict custom application commands and native permissions so only the trusted main window can invoke the current application metadata command and necessary shell capabilities.

#### Scenario: Main Window Invokes Metadata

- **GIVEN** the trusted main window is running
- **WHEN** Angular requests application metadata
- **THEN** the permitted command succeeds through the typed application API

#### Scenario: Unauthorized Caller Is Denied

- **GIVEN** an unauthorized window, label, or origin attempts to invoke an application command
- **WHEN** it calls the metadata command or another registered custom command
- **THEN** Tauri denies the invocation
- **AND** the denial is covered by automated or recorded native verification

#### Scenario: Unneeded Native Grants Stay Unavailable

- **GIVEN** production desktop capabilities are loaded
- **WHEN** a WebView attempts to use unneeded core, window, log, filesystem, shell, process, or network authority
- **THEN** the authority is unavailable unless it is explicitly justified by the current foundation

### Requirement: Lattice SHALL Fail Closed On Fatal Desktop Startup Failure

Lattice SHALL make fatal desktop startup failures visible and exit unsuccessfully instead of silently continuing or returning a successful process status.

#### Scenario: Desktop Boot Fails

- **GIVEN** the desktop shell encounters a fatal boot error before the main window is usable
- **WHEN** startup handling runs
- **THEN** the error is reported through a safe visible channel
- **AND** the process exits with an unsuccessful status

### Requirement: Lattice SHALL Record Baseline Security And Resource Evidence

Lattice SHALL maintain reproducible 0.3 evidence for native security behavior, dependency/advisory checks, and P0 idle resource measurements.

#### Scenario: Dependency Advisory Checks Run On Schedule

- **GIVEN** dependency manifests or lockfiles change, or a scheduled check runs
- **WHEN** dependency advisory verification executes
- **THEN** JavaScript and Rust dependency advisories are checked with documented commands
- **AND** blocking severity, triage expectations, and exceptions are recorded in policy

#### Scenario: P0 Idle Baseline Is Captured

- **GIVEN** a release desktop build is running with no runtime, model, MCP, provider, workspace, or task capability active
- **WHEN** the app settles idle for five minutes and completes three launch/exit cycles
- **THEN** native/WebView aggregate physical memory, CPU, child processes, hardware, OS, build profile, command versions, and measurement uncertainty are recorded
- **AND** unexpected child processes or unexplained CPU above the v1 gate block completion
