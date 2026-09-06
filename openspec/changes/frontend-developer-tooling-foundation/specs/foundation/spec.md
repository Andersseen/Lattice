# Foundation Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Admit Frontend Libraries With Current Consumers

Frontend libraries added to the desktop app SHALL have an actual application consumer in the same change and SHALL pass the existing frontend and desktop smoke checks.

#### Scenario: Frontend Library Is Added

- **GIVEN** a frontend package is added to `apps/desktop`
- **WHEN** the change is verified
- **THEN** at least one reachable Angular component or application provider consumes it
- **AND** frontend typecheck, lint, tests, build and browser smoke checks pass

### Requirement: Lattice SHALL Keep Agent Development Tooling Out Of Product Runtime

Agent-development tooling SHALL be project-local contributor tooling unless a later accepted product capability introduces runtime skills or MCP behavior.

#### Scenario: Agentyx Is Configured

- **GIVEN** Agentyx is configured for the repository
- **WHEN** a contributor previews installation
- **THEN** the command can run as a dry-run without product runtime behavior
- **AND** the desktop application does not depend on Agentyx packages
