# 0004 - Use pnpm, Turborepo, And Cargo Workspaces

## Status

Accepted

## Context

Lattice has both TypeScript and Rust ecosystems. They should be separated but easy to verify from the root. The project also needs a single JavaScript package manager and reproducible installs.

## Decision

Use pnpm workspaces for TypeScript packages, Turborepo for JavaScript task orchestration, and Cargo workspaces for Rust crates.

## Consequences

- `pnpm check` can be the main local quality command.
- `Cargo.lock` and `pnpm-lock.yaml` are committed because Lattice is an application.
- Cargo remains responsible for Rust compilation and tests.
- The repo avoids npm, Yarn, and Bun lockfiles.
