# Application Runtime Foundation Design

## Contract Generation

Rust remains authoritative for the native wire contract. `lattice-core` exposes a small generator used by `cargo run -p lattice-core --bin generate_types`, and Cargo tests compare the generated output with `packages/types/src/generated.ts`.

This deliberately avoids adding `ts-rs` in 0.2. The current application has one metadata DTO and one error DTO, so a hand-maintained Rust generator has lower dependency cost while still failing drift in ordinary Rust tests. Reconsider `ts-rs` when the contract surface grows beyond the metadata command or when serde rename behavior becomes difficult to audit manually.

## Angular Boundary

Angular still owns presentation fallback state. Native metadata is decoded through `decodeAppInfo()` after Tauri returns an unknown payload. The public `AppInfo` union allows the browser fallback runtime, while generated `NativeAppInfo` remains native-only.

Errors are normalized only in `app-wire.ts`. Stores receive `AppError` from that boundary and do not duplicate shape checks.

## Refresh Ordering

`AppInfoStore` assigns a monotonically increasing sequence to every refresh. Only the latest sequence may update info, error, or loading state. This keeps late failures and late successes from replacing newer results.

## Tooling

OpenSpec validation is pinned to `@fission-ai/openspec@1.12.0`, current npm latest as checked on 2026-09-06. If the version changes upstream, update the pin through a normal dependency/tooling review.
