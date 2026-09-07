# Model Runtime Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Configure A Runtime Executable Without Probing

Lattice SHALL allow a user-selected absolute runtime executable path to be stored without executing it.

#### Scenario: Absolute Executable Path Is Configured

- **GIVEN** no runtime executable path is configured
- **WHEN** the user saves an absolute executable path
- **THEN** Lattice stores the path
- **AND** does not execute the path during configuration

#### Scenario: Relative Executable Path Is Rejected

- **GIVEN** a runtime executable path is configured through IPC
- **WHEN** the path is relative or empty
- **THEN** Lattice rejects it with a safe recoverable error
- **AND** preserves the previous runtime discovery revision

### Requirement: Lattice SHALL Probe Runtime Status Only After Explicit Approval

Lattice SHALL execute only the configured absolute runtime executable through fixed read-only probe commands after the user explicitly requests a probe.

#### Scenario: Runtime Probe Is Approved

- **GIVEN** an absolute runtime executable path is configured
- **WHEN** the user requests a runtime probe
- **THEN** Lattice executes bounded read-only version, daemon-status, and server-status probes
- **AND** stores the observed executable identity and runtime metadata

#### Scenario: Runtime Probe Times Out

- **GIVEN** the configured executable does not finish a probe within the allowed timeout
- **WHEN** the user requests a runtime probe
- **THEN** Lattice terminates the probe attempt
- **AND** reports status `unknown`
- **AND** does not start or stop runtime resources

### Requirement: Lattice SHALL Report Distinct Runtime Availability States

Lattice SHALL report top-level runtime availability as `missing`, `unsupported`, `stopped`, `running`, `unreachable`, or `unknown`, with separate daemon and server observations.

#### Scenario: Supported Runtime Is Running

- **GIVEN** the approved executable reports a supported CLI version, running daemon, and running local server
- **WHEN** the loopback endpoint is reachable
- **THEN** Lattice reports runtime status `running`
- **AND** includes daemon and server observations separately

#### Scenario: Server Is Unreachable

- **GIVEN** the approved executable reports a running server port
- **WHEN** Lattice cannot connect to the loopback endpoint
- **THEN** Lattice reports runtime status `unreachable`
- **AND** does not follow redirects or use a remote host

#### Scenario: Runtime Is Unsupported

- **GIVEN** the approved executable reports a CLI version below the supported range
- **WHEN** Lattice completes the probe
- **THEN** Lattice reports runtime status `unsupported`

### Requirement: Lattice SHALL Expose A Models Setup UI For Discovery

Lattice SHALL provide a Models setup/status view that consumes ModelRuntime discovery DTOs without coupling Angular to llmster commands.

#### Scenario: User Probes Configured Runtime

- **GIVEN** the Models view has a configured executable path
- **WHEN** the user requests a probe
- **THEN** Angular calls the typed ModelRuntime discovery API
- **AND** renders the returned availability state, daemon observation, server observation, endpoint, and revision
