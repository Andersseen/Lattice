# Foundation Specification

## Purpose

Define the current foundation guarantees for Lattice before product capabilities are implemented.

## Requirements

### Requirement: Lattice SHALL Keep A Desktop Shell Boundary

Lattice SHALL expose native capabilities to the Angular UI only through typed application APIs backed by Tauri IPC.

#### Scenario: Read Application Info

- **GIVEN** the desktop shell is running
- **WHEN** Angular requests application information
- **THEN** the request is handled by a Tauri command
- **AND** the command delegates to the Rust core
- **AND** the response is structured application metadata

### Requirement: Lattice SHALL Support Browser Smoke Verification

The Angular shell SHALL be testable in a browser-only CI mode without native desktop automation.

#### Scenario: Web Shell Loads

- **GIVEN** the Angular dev server is running
- **WHEN** Playwright opens the home route
- **THEN** the shell renders
- **AND** basic navigation works
- **AND** the bridge reports a typed web fallback

### Requirement: Lattice MUST Avoid Future Feature Placeholders

The foundation MUST NOT implement fake agent, model, MCP, skill, memory, task, workspace, Wisp, or Vertex behavior.

#### Scenario: Future Capability Is Proposed

- **GIVEN** a future capability is needed
- **WHEN** the implementation changes behavior or architecture
- **THEN** it is proposed through OpenSpec before implementation
