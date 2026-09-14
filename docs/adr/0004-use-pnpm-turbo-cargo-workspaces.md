# 0004 - Use Bun, Turborepo, And Cargo Workspaces

## Status

Accepted

## Context

Lattice has both TypeScript and Rust ecosystems. They should be separated but easy to verify from the root. The project also needs a single JavaScript package manager and reproducible installs.

## Decision

Use Bun workspaces for TypeScript packages and JavaScript command running, Turborepo for JavaScript task orchestration, and Cargo workspaces for Rust crates.

Biome handles generic TypeScript, JavaScript, JSON and CSS formatting/linting. Angular ESLint remains only for Angular component conventions and template validation that Biome does not model correctly for this repo.

Rspack and Nx are not adopted in this tooling pass. Lattice keeps the official Angular builder because replacing the bundler/task graph would add integration overhead without demonstrated project need.

## Consequences

- `bun run check` can be the main local quality command.
- `Cargo.lock` and `bun.lock` are committed because Lattice is an application.
- Cargo remains responsible for Rust compilation and tests.
- The repo avoids npm, pnpm and Yarn lockfiles.
