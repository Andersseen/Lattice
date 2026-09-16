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
- Credential references whose secret values live only in the macOS Keychain, entered through a native prompt; no command returns a secret, and unsupported platforms fail closed.
- Remote OpenAI-compatible providers over HTTPS only, with certificate verification against bundled Mozilla roots, no redirect following, no environment proxies, and per-endpoint consent that is cleared when the endpoint changes. Credentials are resolved per request and sent only in the `Authorization` header. See [docs/remote-providers.md](docs/remote-providers.md) and ADR 0014 for known limitations.
- Rust-owned local model runtime discovery/lifecycle limited to an approved executable and loopback endpoint.
- No terminal, filesystem, MCP, agent tool, or arbitrary command execution features.

Planned:

- Explicit permissions for filesystem, terminal, MCP, and external tools.
- Security review before agent execution or model runtime process management becomes user-facing.

## Sensitive Data

Do not commit secrets, API keys, model credentials, tokens, `.env` files, or private user data.

## Dependency Advisories

Dependency advisory checks run on a schedule and when dependency manifests or lockfiles change.

- JavaScript advisories use `bun audit`.
- Rust advisories use `cargo audit --deny yanked`.
- High, critical, withdrawn, or explicitly denied advisories block release qualification until fixed or documented.
- Unmaintained or unsound RustSec warnings require triage and tracking, but do not fail ordinary PRs unless explicitly escalated.
- Exceptions must name an owner, affected package, severity, reason, mitigation, and revisit date.
- Advisory service outages should be triaged and rerun; missing advisory evidence blocks a security-baseline release claim.
