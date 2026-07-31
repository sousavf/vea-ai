# Security policy

Vea is pre-release software and must not be trusted with sensitive repositories or production credentials yet.

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability. Once the GitHub repository is published, use GitHub private vulnerability reporting. Until then, contact the repository owner privately.

Include the affected version, reproduction steps, impact, and any suggested mitigation. Do not access data that is not yours.

## Security invariants

- The Tauri renderer has no direct shell, filesystem, keychain, database, or credential access.
- Repository content, model output, skills, plugins, and MCP data are untrusted.
- A worktree is concurrency isolation, not a security sandbox.
- Credentials are never stored in project or Vea JSON configuration.
- A provider receives data only through an explicit, disclosed adapter and account.
- Side effects require host policy and an exact action digest; changed actions require new approval.
- Provider integrations must use documented APIs, SDKs, OAuth flows, or CLI contracts.
- Vea does not scrape credentials, automate consumer web sessions, or evade provider limits.

The full threat model and release gates are in [`docs/security/threat-model.md`](docs/security/threat-model.md).
