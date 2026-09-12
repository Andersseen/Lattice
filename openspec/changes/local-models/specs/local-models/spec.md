# Local Models Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Report Installed And Loaded Model Inventory Separately

Lattice SHALL list models present on disk independently of whether any model is currently loaded, and SHALL report the live loaded state from a fresh probe rather than an assumption.

#### Scenario: Installed Inventory Is Listed Without Requiring A Loaded Model

- **GIVEN** the runtime is `running` and at least one model is present on disk
- **WHEN** the user opens the Models view
- **THEN** Lattice lists the installed models
- **AND** the list does not require any model to be currently loaded

#### Scenario: Runtime Unavailable Yields An Explicit Unavailable Inventory

- **GIVEN** the runtime availability is not `running`
- **WHEN** the user requests the model inventory
- **THEN** Lattice reports an explicit unavailable state naming the runtime precondition
- **AND** issues no `lms` subprocess call

### Requirement: Lattice SHALL Manage Exactly One Lattice-Owned Loaded Model Slot

Lattice SHALL support loading and unloading at most one model it manages at a time, and SHALL refuse to load a second model while any model already occupies the slot.

#### Scenario: Model Is Loaded Into An Empty Slot

- **GIVEN** a fresh loaded-inventory probe reports no model loaded
- **WHEN** the user requests loading an installed model
- **THEN** Lattice issues the load command
- **AND** confirms success only through a following loaded-inventory probe

#### Scenario: Loading A Second Model Is Refused While The Slot Is Occupied

- **GIVEN** a fresh loaded-inventory probe reports any model already loaded, owned or not
- **WHEN** the user requests loading a different model
- **THEN** Lattice issues no load command
- **AND** reports operation outcome `refused`

#### Scenario: Unload Releases Owned Model State

- **GIVEN** a persisted `owned` load record whose identifier and model key match the current loaded-inventory probe
- **WHEN** the user requests unloading the managed model
- **THEN** Lattice issues the unload command
- **AND** reports operation outcome `unloaded` after a confirming probe
- **AND** clears the owned record

### Requirement: Lattice SHALL Distinguish Owned From Externally Loaded Models

Lattice SHALL record whether a loaded model was loaded by Lattice's own load operation (`owned`) or was already present when observed (`attached`), and SHALL never infer ownership from a single observation without a matching persisted record.

#### Scenario: Externally Loaded Model Is Recorded As Attached

- **GIVEN** a fresh loaded-inventory probe reports a model loaded with no matching persisted `owned` record
- **WHEN** Lattice reads the model slot status
- **THEN** Lattice records ownership `attached`
- **AND** does not later treat that model as unloadable by Lattice

#### Scenario: Attached Model Cannot Be Unloaded By Lattice

- **GIVEN** ownership is recorded as `attached`
- **WHEN** the user requests unloading the managed model
- **THEN** Lattice issues no unload command
- **AND** reports operation outcome `refused`

### Requirement: Lattice SHALL Confirm Load And Unload Outcomes Independently Of The Mutating Command's Output

Lattice SHALL determine the success, failure and identity of a load or unload exclusively from a follow-up loaded-inventory probe, never from the load or unload command's own exit code or standard output.

#### Scenario: Load Success Is Confirmed Via A Follow-Up Probe

- **GIVEN** Lattice has issued a load command for a specific model key and identifier
- **WHEN** the following loaded-inventory probe reports that exact identifier and model key loaded
- **THEN** Lattice records ownership `owned` using the probe's observation
- **AND** reports operation outcome `loaded`

#### Scenario: Unconfirmed Load Leaves No Owned Record

- **GIVEN** Lattice has issued a load command
- **WHEN** the following loaded-inventory probe does not report the requested identifier and model key loaded
- **THEN** Lattice reports operation outcome `failed`
- **AND** writes no `owned` ownership record

### Requirement: Lattice SHALL Bound And Allow Cancellation Of Load And Unload Operations

Lattice SHALL bound every load and unload operation with a fixed deadline and SHALL let the user cancel an in-flight operation without forcing an additional mutation of model state.

#### Scenario: Load Times Out Without Assuming Success

- **GIVEN** a load operation is waiting for a confirming probe
- **WHEN** the bounded deadline elapses without confirmation
- **THEN** Lattice stops waiting
- **AND** reports operation outcome `timed_out` using one final probe's status
- **AND** writes no `owned` ownership record

#### Scenario: User Cancels An In-Flight Load Or Unload

- **GIVEN** a load or unload operation is in flight
- **WHEN** the user requests cancellation
- **THEN** Lattice stops waiting without issuing a further load or unload command
- **AND** reports operation outcome `cancelled` using one final probe's status
- **AND** a model that the probe later shows loaded anyway is recorded `attached`, not `owned`

### Requirement: Lattice SHALL Record Qualified Candidate Model Profiles With Manual Acquisition Guidance

Lattice SHALL document at least one qualified small Qwen and one qualified small Gemma candidate profile, and SHALL guide the user to acquire a missing candidate through the runtime's own documented process rather than downloading it itself.

#### Scenario: Missing Candidate Model Shows Actionable Guidance, Not A Download Action

- **GIVEN** a qualified candidate profile's model key is absent from the installed inventory
- **WHEN** the user views that candidate in the Models view
- **THEN** Lattice shows guidance naming the runtime's own acquisition step
- **AND** Lattice initiates no download of any kind
