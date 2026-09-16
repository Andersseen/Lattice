# Chat Streaming Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Resolve The Chat Target Per Request

Lattice SHALL accept an explicit local or remote target with every chat request, SHALL apply that target only to that request, and SHALL use the same run identity, sequencing, terminal-outcome and single-active-run rules for both.

#### Scenario: Selecting A Target Affects Only The Next Request

- **GIVEN** a conversation whose previous reply came from the local target
- **WHEN** the next request names a remote profile target
- **THEN** that request is dispatched to the remote profile
- **AND** earlier stored messages and their provenance are unchanged

#### Scenario: Local And Remote Runs Share The Single Active Run

- **GIVEN** a remote run is active
- **WHEN** a local chat request is submitted
- **THEN** Lattice refuses it as a conflict without contacting either destination

### Requirement: Lattice SHALL Hold The Local Model Lease Only For Local Runs

Lattice SHALL NOT require a loaded local model for a remote run, and SHALL refuse unloading the managed model only while a local run is active.

#### Scenario: A Remote Run Needs No Loaded Local Model

- **GIVEN** no local model is loaded and a consented remote profile exists
- **WHEN** a chat request targets that profile
- **THEN** Lattice dispatches it without consulting the local model slot

#### Scenario: Unload Is Allowed While Only A Remote Run Is Active

- **GIVEN** a remote run is active and a local model is loaded and owned
- **WHEN** the user unloads the local model
- **THEN** Lattice does not refuse the unload because of the remote run

### Requirement: Lattice SHALL Cancel Remote Runs Promptly And Close The Connection On A Best-Effort Basis

Lattice SHALL report cancellation of any run within the orchestrator poll interval, SHALL skip dispatch when cancellation is observed before the request is sent, and SHALL close an in-flight connection no later than the next chunk it receives after cancellation.

#### Scenario: Cancellation Before Dispatch Sends No Request

- **GIVEN** a run is cancelled before its request is sent
- **WHEN** the reader observes the cancellation
- **THEN** no request reaches the destination
- **AND** the run's only terminal event is `cancelled`

#### Scenario: Cancellation Mid-Stream Stops Reading At The Next Chunk

- **GIVEN** a remote run has emitted one or more deltas
- **WHEN** the user cancels
- **THEN** Lattice emits `cancelled` as the run's only terminal event
- **AND** stops reading and closes the connection once the next chunk arrives or the deadline elapses
