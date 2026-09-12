# Providers Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Define A Canonical Completion Request Independent Of Any Vendor Field

Lattice SHALL express a chat completion request and its messages using only role, text, and the selected model key, with no field specific to any one provider's wire protocol.

#### Scenario: Canonical Request Contains No Vendor-Specific Field

- **GIVEN** a chat request is constructed for the local provider
- **WHEN** the request crosses the completion port
- **THEN** it contains only role, text, and model key
- **AND** no sampling or vendor-specific field is present in the canonical shape

### Requirement: Lattice SHALL Isolate Vendor Protocol Translation To One Adapter Per Provider

Lattice SHALL translate a canonical request into a specific provider's wire format, and translate that provider's response back into canonical stream events, entirely inside that provider's own adapter module.

#### Scenario: The Local OpenAI-Compatible Adapter Translates Requests And Responses

- **GIVEN** a canonical chat request targets the local provider
- **WHEN** Lattice dispatches it
- **THEN** the local adapter builds the provider-specific request body
- **AND** the local adapter is the only module that parses the provider-specific streaming response

#### Scenario: Unparseable Vendor Output Yields A Safe Failed Outcome, Never A Panic

- **GIVEN** the local provider's response contains a line that does not match its documented delta shape
- **WHEN** the adapter parses it
- **THEN** Lattice reports a normalized `failed` outcome with a safe, bounded message
- **AND** does not panic or propagate the raw vendor payload

### Requirement: Lattice SHALL Require An Explicit, Currently-Owned Model Lease Before Dispatching A Local Completion Request

Lattice SHALL refuse to dispatch a completion request against the local provider unless the requested model key matches the model currently owned and loaded in the managed slot.

#### Scenario: Request Naming The Owned Loaded Model Is Dispatched

- **GIVEN** the managed model slot's ownership is `owned` for model key `m`
- **WHEN** a chat request names model key `m`
- **THEN** Lattice dispatches the request to the local provider

#### Scenario: Request Naming Any Other Model Is Refused

- **GIVEN** the managed model slot's ownership is `owned` for model key `m`, `attached`, or `unknown`
- **WHEN** a chat request names a different model key, or the slot is not `owned`
- **THEN** Lattice refuses the request without dispatching it
- **AND** issues no implicit load of the requested model
