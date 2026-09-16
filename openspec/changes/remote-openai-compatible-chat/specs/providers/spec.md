# Providers Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Store Remote Provider Profiles As Non-Secret References

Lattice SHALL persist each remote provider profile as a label, an HTTPS endpoint, a remote model key, an optional credential reference and a consent record, and SHALL NOT store any secret value in a profile.

#### Scenario: A Profile References A Credential Without Containing Its Secret

- **GIVEN** a credential exists and a profile is created bound to it
- **WHEN** the profile is listed
- **THEN** the profile exposes only the credential's identity
- **AND** no field of the profile carries the credential's secret value

#### Scenario: Deleting A Bound Credential Unbinds It From Every Profile

- **GIVEN** a profile is bound to a credential
- **WHEN** that credential is deleted
- **THEN** the profile remains listed with no credential bound

#### Scenario: A Stale Revision Cannot Modify A Profile

- **GIVEN** a profile has been modified since the caller last read it
- **WHEN** the caller updates it, binds a credential, or changes its consent using the old revision
- **THEN** Lattice refuses the request as a conflict
- **AND** the stored profile is unchanged

### Requirement: Lattice SHALL Accept Only Explicit HTTPS Remote Endpoints

Lattice SHALL accept a remote profile endpoint only when it is an absolute `https` URL with a host and without embedded credentials, query or fragment.

#### Scenario: A Valid HTTPS Base URL Is Accepted

- **GIVEN** the endpoint `https://api.example.com/v1/`
- **WHEN** a profile is created with it
- **THEN** Lattice stores the normalized endpoint `https://api.example.com/v1`

#### Scenario: A Non-HTTPS Or Credential-Bearing Endpoint Is Refused

- **GIVEN** an endpoint using `http`, containing user information, a query or a fragment, or lacking a host
- **WHEN** a profile is created or updated with it
- **THEN** Lattice refuses the request with a validation error
- **AND** stores nothing

### Requirement: Lattice SHALL Bind Credentials And Consent To One Destination

Lattice SHALL treat a profile's credential binding and consent as valid only for the endpoint they were given for, and SHALL clear both when the profile's endpoint changes.

#### Scenario: Changing The Endpoint Clears Credential Binding And Consent

- **GIVEN** a profile with a bound credential and granted consent
- **WHEN** its endpoint is changed to a different URL
- **THEN** the profile has no bound credential and no consent
- **AND** a subsequent remote request through it is refused until consent is granted again

#### Scenario: Changing Only Label Or Model Keeps The Destination Bindings

- **GIVEN** a profile with a bound credential and granted consent
- **WHEN** only its label or model key changes
- **THEN** its credential binding and consent remain in effect

#### Scenario: Consent Is Granted Only For The Endpoint The User Was Shown

- **GIVEN** a profile's current endpoint differs from the endpoint named in a consent request
- **WHEN** consent is requested
- **THEN** Lattice refuses the request
- **AND** records no consent

#### Scenario: Consent Can Be Revoked

- **GIVEN** a profile with granted consent
- **WHEN** the user revokes consent
- **THEN** the next remote request through that profile is refused until consent is granted again

### Requirement: Lattice SHALL Contact A Remote Provider Only For An Explicit, Consented Chat Request

Lattice SHALL send no network request to a remote endpoint as a result of creating, updating, selecting, binding or consenting to a profile, and SHALL dispatch a remote chat request only after verifying the requested model, the destination-bound consent and the bound credential.

#### Scenario: Profile Management And Selection Send No Network Traffic

- **GIVEN** a profile whose endpoint names a reachable listener
- **WHEN** the profile is created, listed, updated, bound, consented to, and selected
- **THEN** the listener receives no connection

#### Scenario: A Request Without Valid Consent Is Refused Before Any Network Call

- **GIVEN** a profile without consent for its current endpoint
- **WHEN** a chat request targets it
- **THEN** Lattice refuses the request with a consent-required error
- **AND** issues no network request and persists no message

#### Scenario: A Request Naming A Different Model Than The Profile Is Refused

- **GIVEN** a consented profile configured for model `m`
- **WHEN** a chat request targeting it names a model other than `m`
- **THEN** Lattice refuses the request without dispatching it

#### Scenario: An Unavailable Credential Is Refused Before Any Network Call

- **GIVEN** a consented profile whose bound credential is missing, locked or unsupported
- **WHEN** a chat request targets it
- **THEN** Lattice refuses the request with the credential's safe availability error
- **AND** issues no network request and persists no message

### Requirement: Lattice SHALL Keep Remote Credentials Out Of Every Non-Request Surface

Lattice SHALL resolve a bound credential only when dispatching the request that uses it, SHALL place it only in that request's authorization header, and SHALL NOT expose it through IPC, logs, conversation storage, error messages, URLs or a long-lived cache.

#### Scenario: The Resolved Secret Reaches Only The Authorization Header

- **GIVEN** a consented profile bound to a credential with secret `s`
- **WHEN** a remote chat request is dispatched and fails
- **THEN** the provider receives `s` only as a bearer authorization header
- **AND** `s` appears in no stream event, error message, persisted message or request URL

### Requirement: Lattice SHALL Reach Only The Configured Remote Destination

Lattice SHALL send a remote request directly to the profile's configured endpoint, SHALL NOT follow redirects, and SHALL NOT route the request through an environment-configured proxy.

#### Scenario: A Redirect Response Is Refused, Not Followed

- **GIVEN** the remote endpoint answers with a redirect to another location
- **WHEN** a remote chat request is dispatched
- **THEN** Lattice terminates the run with a normalized rejection failure
- **AND** sends no request to the redirect location

### Requirement: Lattice SHALL Normalize Remote Provider Failures

Lattice SHALL report remote failures as stable, safe error codes and SHALL NOT forward a remote provider's raw error body.

#### Scenario: Authentication, Rate Limit, Server And Timeout Failures Are Distinguished

- **GIVEN** a remote endpoint answers with `401` or `403`, `429`, a `5xx` status, or does not answer within the deadline
- **WHEN** a remote chat request is dispatched
- **THEN** Lattice terminates the run with `provider.auth_failed`, `provider.rate_limited`, `provider.unavailable` or `provider.timeout` respectively
- **AND** the failure message contains no text from the provider's response body
- **AND** Lattice does not retry the request automatically

#### Scenario: An Error Payload Inside A Stream Fails The Run

- **GIVEN** a remote stream delivers a data payload carrying an `error` object
- **WHEN** Lattice parses it
- **THEN** Lattice terminates the run with exactly one failed outcome

#### Scenario: An Oversized Stream Line Fails The Run

- **GIVEN** a stream line exceeds the configured maximum line length
- **WHEN** Lattice reads it
- **THEN** Lattice terminates the run with exactly one failed outcome without buffering the line further

### Requirement: Lattice SHALL Translate The Pinned OpenAI-Compatible Subset For Both Local And Remote Destinations

Lattice SHALL translate canonical chat requests and streamed responses for local and remote OpenAI-compatible destinations inside one adapter module, differing only in destination URL, authentication and the output-limit field.

#### Scenario: The Remote Destination Uses The Current Output-Limit Field And Bearer Authentication

- **GIVEN** a remote chat request with a bound credential
- **WHEN** the adapter builds the request
- **THEN** it posts to the profile endpoint's `/chat/completions` path with `stream` enabled
- **AND** declares the output bound as `max_completion_tokens` with a bearer authorization header

#### Scenario: The Local Destination Keeps Its Existing Wire Shape

- **GIVEN** a local chat request
- **WHEN** the adapter builds the request
- **THEN** it posts to the local runtime's `/v1/chat/completions` path with `max_tokens` and no authorization header

#### Scenario: Empty Assistant Turns Are Omitted From The Wire Request

- **GIVEN** canonical history containing an assistant message with empty text, such as a reply that failed before its first token
- **WHEN** the adapter builds the request
- **THEN** that empty assistant turn is not sent
- **AND** the canonical stored history is unchanged
