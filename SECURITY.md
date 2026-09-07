# Security

Lattice is early-stage software and should not be treated as a hardened security boundary yet.

## Reporting

Please report vulnerabilities privately to the maintainers. Do not open a public issue for security-sensitive findings.

## Current Security Posture

Implemented now:

- Rust-owned Tauri command boundary.
- Production desktop Content Security Policy for bundled assets and required IPC.
- Main-window-only application command permission for foundation metadata.
- Structured IPC error shape.
- No credential storage.
- No terminal, filesystem, MCP, provider, model runtime, or arbitrary command execution features.

Planned:

- OS-secure credential storage through Rust-owned APIs.
- Explicit permissions for filesystem, terminal, MCP, and external tools.
- Security review before agent execution or model runtime process management becomes user-facing.

## Sensitive Data

Do not commit secrets, API keys, model credentials, tokens, `.env` files, or private user data.

## Dependency Advisories

Dependency advisory checks run on a schedule and when dependency manifests or lockfiles change.

- JavaScript advisories use `pnpm audit --audit-level high`.
- Rust advisories use `cargo audit --deny yanked`.
- High, critical, withdrawn, or explicitly denied advisories block release qualification until fixed or documented.
- Unmaintained or unsound RustSec warnings require triage and tracking, but do not fail ordinary PRs unless explicitly escalated.
- Exceptions must name an owner, affected package, severity, reason, mitigation, and revisit date.
- Advisory service outages should be triaged and rerun; missing advisory evidence blocks a security-baseline release claim.
