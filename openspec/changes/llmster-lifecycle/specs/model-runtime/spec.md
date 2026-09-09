# Model Runtime Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Start An Approved Runtime Only When Not Already Present

Lattice SHALL start the daemon and local server of the already-approved runtime executable only when a fresh probe shows them not already running, and SHALL treat an already-running resource as a no-op rather than spawning a second process.

#### Scenario: Runtime Is Started From Stopped

- **GIVEN** an approved runtime executable and a fresh probe reporting the daemon and server stopped
- **WHEN** the user requests a runtime start
- **THEN** Lattice spawns the daemon and server through fixed argv commands
- **AND** waits for a bounded health confirmation before reporting status `running`

#### Scenario: Already Running Runtime Is Not Restarted

- **GIVEN** a fresh probe reports the daemon already running
- **WHEN** the user requests a runtime start
- **THEN** Lattice spawns no new process
- **AND** reports operation outcome `already_running` using the freshly observed state

### Requirement: Lattice SHALL Distinguish Owned From Attached Runtime Resources

Lattice SHALL record whether a running daemon was started by the current start operation (`owned`) or was already present (`attached`), and SHALL never infer ownership from a single observation without a matching persisted record.

#### Scenario: Lattice Records Ownership Of A Started Daemon

- **GIVEN** Lattice spawns the daemon because none was running
- **WHEN** the daemon reports its own process ID in its start output
- **THEN** Lattice records that reported process ID, not the ID of the process used to invoke the command, as the owned daemon identity

#### Scenario: Pre-Existing Runtime Is Recorded As Attached

- **GIVEN** a fresh probe reports the daemon already running before any start is requested
- **WHEN** Lattice completes the start operation
- **THEN** Lattice records ownership `attached`
- **AND** does not later treat that daemon as stoppable by Lattice

### Requirement: Lattice SHALL Stop Only Provably Owned Runtime Resources

Lattice SHALL refuse to stop a runtime resource unless a fresh probe's observed process identity and executable fingerprint match a persisted `owned` ownership record.

#### Scenario: Owned Runtime Is Stopped

- **GIVEN** a persisted `owned` ownership record whose process identity and executable fingerprint match the current probe
- **WHEN** the user requests a runtime stop
- **THEN** Lattice executes the stop commands
- **AND** reports operation outcome `stopped` after a confirming probe

#### Scenario: Attached Runtime Stop Is Refused

- **GIVEN** ownership is recorded as `attached`
- **WHEN** the user requests a runtime stop
- **THEN** Lattice executes no stop command
- **AND** reports operation outcome `refused`

#### Scenario: Ownership Cannot Be Verified

- **GIVEN** a persisted `owned` record whose process identity or executable fingerprint no longer matches the current probe
- **WHEN** the user requests a runtime stop
- **THEN** Lattice downgrades ownership to `unknown`
- **AND** executes no stop command
- **AND** reports operation outcome `refused`

### Requirement: Lattice SHALL Bound And Allow Cancellation Of Lifecycle Operations

Lattice SHALL bound every start and stop operation with a fixed deadline and SHALL let the user cancel an in-flight operation without forcing an additional mutation of runtime state.

#### Scenario: Health Wait Times Out

- **GIVEN** a start operation is waiting for health confirmation
- **WHEN** the bounded deadline elapses without confirmation
- **THEN** Lattice stops waiting
- **AND** reports operation outcome `timed_out` using one final probe's status

#### Scenario: User Cancels An In-Flight Operation

- **GIVEN** a start or stop operation is in flight
- **WHEN** the user requests cancellation
- **THEN** Lattice stops waiting without issuing a further stop or start command
- **AND** reports operation outcome `cancelled` using one final probe's status

### Requirement: Lattice SHALL Stop Owned Runtime Resources On Application Exit

Lattice SHALL attempt to stop only currently `owned` runtime resources when the application exits, within a short bounded deadline, and SHALL never block application exit on that attempt succeeding.

#### Scenario: Owned Runtime Is Stopped At Shutdown

- **GIVEN** ownership is recorded as `owned` when the application begins exiting
- **WHEN** shutdown proceeds
- **THEN** Lattice attempts a bounded stop of the owned resources
- **AND** allows application exit to proceed regardless of the stop outcome

#### Scenario: Attached Runtime Is Left Running At Shutdown

- **GIVEN** ownership is recorded as `attached` or `unknown` when the application begins exiting
- **WHEN** shutdown proceeds
- **THEN** Lattice issues no stop command for that resource
