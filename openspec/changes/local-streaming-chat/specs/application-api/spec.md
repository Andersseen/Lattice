# Application API Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Extend Checked IPC Contracts For Streaming Chat

Lattice SHALL register `start_chat_stream` and `cancel_chat_stream` in the same checked command inventory used for existing application commands, and SHALL generate matching shared TypeScript bindings for the request, run handle, and every stream event variant.

#### Scenario: Chat Commands Are Generated

- **GIVEN** the Rust command inventory declares the two chat-streaming command constants
- **WHEN** the checked command inventory test runs
- **THEN** the Tauri handler registration and the declared constants match exactly
- **AND** generated TypeScript bindings expose the same command names, the run handle shape, and every `ChatStreamEvent` variant

#### Scenario: Stream Events Are Decoded Defensively

- **GIVEN** a channel delivers a stream event payload to Angular
- **WHEN** Angular decodes it
- **THEN** an unknown or malformed event `kind` decodes to a safe bridge error rather than reaching the UI layer
- **AND** decoding one malformed event does not stop delivery of subsequent valid events for the same run
