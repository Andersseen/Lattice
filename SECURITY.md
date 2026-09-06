# Security

Lattice is early-stage software and should not be treated as a hardened security boundary yet.

## Reporting

Please report vulnerabilities privately to the maintainers. Do not open a public issue for security-sensitive findings.

## Current Security Posture

Implemented now:

- Rust-owned Tauri command boundary.
- Structured IPC error shape.
- No credential storage.
- No terminal, filesystem, MCP, provider, model runtime, or arbitrary command execution features.

Planned:

- OS-secure credential storage through Rust-owned APIs.
- Explicit permissions for filesystem, terminal, MCP, and external tools.
- Security review before agent execution or model runtime process management becomes user-facing.

## Sensitive Data

Do not commit secrets, API keys, model credentials, tokens, `.env` files, or private user data.
