# Chat Streaming Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Stream Incremental Text Under One Identified Run

Lattice SHALL assign a stable run identity before dispatching any completion request, register the event channel for that run before dispatch, and label every subsequent event for that run with the same identity and a monotonically increasing sequence number.

#### Scenario: A New Run Receives A Stable Identity Before Any Event Is Sent

- **GIVEN** the user submits a chat request while no run is active
- **WHEN** Lattice accepts the request
- **THEN** Lattice returns a run identity to the caller before the first `started` or `delta` event can be observed
- **AND** every event for that request carries the same run identity

#### Scenario: Sequence Numbers Are Monotonically Increasing And Gapless From Zero

- **GIVEN** a run has emitted N events
- **WHEN** the next event for that run is emitted
- **THEN** its sequence number is exactly N

### Requirement: Lattice SHALL Produce Exactly One Terminal Outcome Per Run

Lattice SHALL terminate every run with exactly one of `completed`, `cancelled`, or `failed`, and SHALL emit no further event for that run afterward.

#### Scenario: Natural Completion Emits Exactly One Completed Event

- **GIVEN** the local model runtime sends a complete response ending in `[DONE]`
- **WHEN** Lattice finishes reading the stream
- **THEN** Lattice emits exactly one `completed` event naming the finish reason
- **AND** emits no `cancelled` or `failed` event for that run

#### Scenario: Malformed Stream Content Emits Exactly One Failed Event

- **GIVEN** the local model runtime sends a line that does not parse as a valid delta
- **WHEN** Lattice encounters that line
- **THEN** Lattice stops reading further stream content
- **AND** emits exactly one `failed` event with a safe, bounded message
- **AND** does not silently skip the malformed line and continue

#### Scenario: A Duplicate Terminal-Shaped Payload Is Never Forwarded Twice

- **GIVEN** a run has already emitted its one terminal event
- **WHEN** the underlying connection subsequently delivers additional terminal-shaped content before it is closed
- **THEN** Lattice forwards no further event for that run

### Requirement: Lattice SHALL Allow Cancellation Of An In-Flight Run At Any Point

Lattice SHALL accept a cancellation request naming the active run's identity at any time between run start and its terminal event, and SHALL terminate that run with `cancelled` rather than any other outcome once cancellation is observed.

#### Scenario: Cancel Before Any Delta Is Received

- **GIVEN** a run has started but no `delta` event has been emitted yet
- **WHEN** the user cancels
- **THEN** Lattice emits `cancelled` as the run's only terminal event
- **AND** emits no `delta` event afterward

#### Scenario: Cancel Mid-Stream

- **GIVEN** a run has emitted one or more `delta` events
- **WHEN** the user cancels
- **THEN** Lattice stops reading further content from the local model runtime
- **AND** emits `cancelled` as the run's only terminal event

#### Scenario: Cancelling An Unknown Or Already-Terminal Run Is A No-Op

- **GIVEN** the named run identity is not the currently active run
- **WHEN** a cancellation request names it
- **THEN** Lattice performs no action
- **AND** returns successfully without error

### Requirement: Lattice SHALL Enforce Exactly One Globally Active Run

Lattice SHALL refuse to start a new run while another run is active, and SHALL guarantee that a crashed, errored, or timed-out run always releases the active-run slot.

#### Scenario: A Second Run Is Refused While One Is Active

- **GIVEN** a run is currently active
- **WHEN** the user submits another chat request
- **THEN** Lattice refuses the new request without contacting the local model runtime
- **AND** the active run continues unaffected

#### Scenario: A Crashed Or Errored Run Frees The Active Slot

- **GIVEN** a run terminates via `failed`, including a worker-thread panic recovered by a guard
- **WHEN** its terminal event is emitted
- **THEN** the active-run slot becomes free
- **AND** the next chat request is accepted

### Requirement: Lattice SHALL Enforce Bounded Prompt, Output, And Time Limits

Lattice SHALL refuse an oversized prompt before contacting the local model runtime, SHALL bound generated output length at the source, and SHALL terminate a stalled or overlong stream by deadline.

#### Scenario: Oversized Prompt Is Refused Before Any Request Is Sent

- **GIVEN** the combined message text exceeds the configured prompt bound
- **WHEN** the user submits the request
- **THEN** Lattice refuses the request
- **AND** issues no HTTP request to the local model runtime

#### Scenario: Output Is Bounded By A Maximum Token Count

- **GIVEN** a run is dispatched
- **WHEN** Lattice builds the outgoing request
- **THEN** the request declares the configured maximum output token count

#### Scenario: A Stalled Stream Is Terminated By Deadline

- **GIVEN** a run is active and no further bytes arrive from the local model runtime within the configured idle timeout
- **WHEN** that timeout elapses
- **THEN** Lattice terminates the run with `failed`
- **AND** frees the active-run slot

### Requirement: Lattice SHALL Prevent The Managed Model From Being Unloaded While A Chat Run Holds Its Lease

Lattice SHALL refuse a model-unload request for as long as a chat run is active against that model, leaving the loaded model untouched.

#### Scenario: Unload Is Refused While A Chat Run Holds The Model Lease

- **GIVEN** a chat run is currently active against the managed model
- **WHEN** the user requests unloading that model
- **THEN** Lattice issues no unload command
- **AND** returns a conflict error naming the active chat run as the reason
- **AND** the loaded model remains untouched
