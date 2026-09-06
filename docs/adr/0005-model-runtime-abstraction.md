# 0005 - Keep Model Runtime Replaceable

## Status

Accepted

## Context

Lattice expects llmster to be the initial local model runtime. Directly coupling UI or core product behavior to llmster would make later replacement or additional runtimes expensive.

## Decision

Treat model execution as a future `ModelRuntime` boundary. llmster should enter through an adapter when that capability is specified.

## Consequences

- No llmster API is introduced in this foundation.
- The UI cannot depend on model runtime details.
- Future runtime work needs an OpenSpec proposal and design.
- The abstraction is documented now but not implemented until there is a real consumer.
