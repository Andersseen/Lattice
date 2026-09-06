# ADR 0006: Rust-owned Application Contracts

- Status: Accepted
- Date: 2026-09-06

## Context

Lattice's first native command returns application metadata from Rust through Tauri IPC. Before 0.2, the TypeScript shape was duplicated by hand and Angular trusted `invoke<T>()` without decoding the runtime payload. That is acceptable for a smoke shell, but it is a weak foundation for later native commands.

## Decision

Rust owns native application command names and wire DTOs. The repo commits generated TypeScript bindings at `packages/types/src/generated.ts`, and `cargo test -p lattice-core committed_typescript_bindings_match_rust_contract` fails when regenerated output differs.

Angular treats Tauri responses as `unknown` and decodes them before returning data from the application API. Browser-only fallback metadata remains a presentation-side extension around the generated native metadata type.

0.2 does not add `ts-rs`. With one native DTO and one error DTO, a local Rust generator provides contract drift detection without introducing dependency, serde compatibility, maintenance, and license surface. Revisit `ts-rs` once there are enough Rust DTOs that manual generator maintenance becomes more expensive than admitting the dependency.

## Consequences

- TypeScript contracts are not edited by hand when native DTOs change.
- Command spelling is centralized instead of repeated in Angular string literals.
- Runtime IPC payloads are decoded instead of trusted through compile-time-only assertions.
- The generator is intentionally small and must not become an independent schema framework.
