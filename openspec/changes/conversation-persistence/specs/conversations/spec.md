# Conversations Specification Delta

## ADDED Requirements

### Requirement: Lattice SHALL Identify Conversations And Messages With Application-Generated Identity

Lattice SHALL assign every conversation and message a stable, application-generated identifier and SHALL never use a provider- or vendor-issued identifier as that identity.

#### Scenario: A New Conversation Receives An Application-Generated Identity

- **GIVEN** a chat request is sent with no existing conversation identity
- **WHEN** Lattice creates the conversation
- **THEN** Lattice assigns it an identifier it generated itself
- **AND** that identifier is returned to the caller before any message is streamed

#### Scenario: A Provider Run Identity Is Never Stored As A Message Or Conversation Identity

- **GIVEN** a chat run is dispatched under a provider-assigned run identity
- **WHEN** Lattice persists the resulting messages
- **THEN** the stored conversation and message identifiers are Lattice-generated
- **AND** the run identity does not appear as a primary key of any stored record

### Requirement: Lattice SHALL Persist Ordered Canonical Messages Per Conversation

Lattice SHALL store every message belonging to a conversation with a role, its text, a gapless per-conversation sequence number, and a generation status.

#### Scenario: A New Message Extends The Conversation In Order

- **GIVEN** a conversation already has N stored messages
- **WHEN** a new message is added to it
- **THEN** the new message is stored at sequence N
- **AND** reopening the conversation returns messages in that same order

#### Scenario: Sending On A Conversation With Fewer Locally-Known Messages Is Refused

- **GIVEN** a conversation has N messages stored
- **WHEN** a request continuing that conversation supplies fewer than N messages of history
- **THEN** Lattice refuses the request as a conflict
- **AND** stores no message from that request

### Requirement: Lattice SHALL Record One Generation Outcome Per Assistant Message

Lattice SHALL track each assistant message's generation status as exactly one of complete, streaming, cancelled, failed, or interrupted, and SHALL update it only through the message's own run lifecycle or restart reconciliation.

#### Scenario: A Streaming Reply Is Visible Before It Finishes

- **GIVEN** an assistant reply has started streaming
- **WHEN** the conversation is read before the run's terminal event
- **THEN** the assistant message is present with status streaming
- **AND** its text reflects at most the most recently written checkpoint, not necessarily the latest token

#### Scenario: A Completed Run Finalizes Its Message

- **GIVEN** a streaming assistant message's run reaches its terminal event
- **WHEN** the terminal event is completed, cancelled, or failed
- **THEN** Lattice writes the message's final text and matching status synchronously
- **AND** a failed message records a safe, bounded error message

### Requirement: Lattice SHALL Bound Checkpoint Writes During A Streaming Reply

Lattice SHALL persist a streaming assistant message's partial text at bounded intervals rather than on every incremental delta, and SHALL always persist its final state exactly once when its run terminates.

#### Scenario: Partial Text Is Checkpointed, Not Written Per Token

- **GIVEN** a run emits many incremental delta events in quick succession
- **WHEN** Lattice persists the assistant message's progress
- **THEN** the number of storage writes is bounded independently of the number of deltas emitted
- **AND** the final terminal write always reflects the complete accumulated text regardless of checkpoint timing

### Requirement: Lattice SHALL Reconcile Interrupted Generations On Restart

Lattice SHALL label any message still recorded as streaming when conversation storage is next opened as interrupted, exactly once, and SHALL NOT automatically resume or retry it.

#### Scenario: A Message Left Streaming At Shutdown Becomes Interrupted On Reopen

- **GIVEN** a message was recorded as streaming when Lattice last stopped
- **WHEN** conversation storage is opened again
- **THEN** that message's status becomes interrupted
- **AND** Lattice does not automatically resume or retry generating it

#### Scenario: A Cleanly Terminated Message Is Not Reclassified

- **GIVEN** a message already reached complete, cancelled, or failed before shutdown
- **WHEN** conversation storage is opened again
- **THEN** that message's status is unchanged

### Requirement: Lattice SHALL Support Creating, Listing, Reopening, And Deleting Conversations

Lattice SHALL let a caller create a conversation implicitly by sending its first message, list existing conversations, reopen one to read its ordered messages, and delete one along with all of its messages.

#### Scenario: Listing Returns The Most Recently Active Conversations First

- **GIVEN** multiple conversations exist with different last-updated times
- **WHEN** the caller lists conversations
- **THEN** they are returned ordered by most recently updated first

#### Scenario: Reopening Returns The Conversation's Ordered Messages

- **GIVEN** a conversation has stored messages
- **WHEN** the caller reopens it
- **THEN** Lattice returns its messages in ascending sequence order

#### Scenario: Deleting A Conversation Removes All Of Its Messages

- **GIVEN** a conversation has one or more stored messages
- **WHEN** the caller deletes that conversation
- **THEN** the conversation and every one of its messages are removed
- **AND** every other conversation and its messages are unaffected

#### Scenario: Deleting A Conversation With An Active Run Is Refused

- **GIVEN** a conversation currently has an active streaming run
- **WHEN** the caller requests deleting that conversation
- **THEN** Lattice refuses the deletion
- **AND** the conversation and its in-progress message remain intact

### Requirement: Lattice SHALL Paginate Conversation And Message Listings

Lattice SHALL bound the number of conversations or messages returned by a single request regardless of how many exist, and SHALL let the caller retrieve additional older results explicitly.

#### Scenario: A Large Conversation List Is Returned In Bounded Pages

- **GIVEN** more conversations exist than one page's configured limit
- **WHEN** the caller lists conversations without requesting further pages
- **THEN** Lattice returns at most that limit
- **AND** indicates that more conversations exist

#### Scenario: A Long Conversation's History Is Returned In Bounded Pages

- **GIVEN** a conversation has more messages than one page's configured limit
- **WHEN** the caller reopens it without requesting older messages
- **THEN** Lattice returns at most that limit of its most recent messages
- **AND** indicates that older messages exist
- **AND** a follow-up request for older messages returns the next bounded page
