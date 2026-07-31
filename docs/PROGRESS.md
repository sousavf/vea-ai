# Implementation progress

Updated: 2026-07-31

## Completed locally

- Initialized Git on `main` and published the public repository at `https://github.com/sousavf/vea-ai`.
- Chosen architecture: Tauri v2 desktop, React/TypeScript UI, Rust privilege host, TypeScript agent service.
- Chosen license: MIT.
- Added architecture, roadmap, threat model, provider gate, extension compatibility, and contribution docs.
- Added versioned contracts for domain state, strict configuration, task graphs, runtime events, routing, scheduling, sidecar framing, and extension trust planes.
- Added a Pi-like multi-project UI shell with project/task/run/route/usage/isolation views and an explicit browser-demo state.
- Added a stateful, handshake-first agent-service protocol foundation.
- Added deterministic weighted deficit round-robin scheduling with write-scope and concurrency checks.
- Added task-aware routing with effort normalization, task/policy budgets, quota freshness, and inspectable decisions.
- Restricted the Tauri renderer capability list to no core permissions; only the registered app metadata command is currently callable.
- Added cross-platform CI definitions and supply-chain lockfiles.

## Validation

- TypeScript workspace typecheck: passing.
- Vitest unit suite: passing.
- Web production build: passing.
- Rust format, Clippy with warnings denied, and tests: passing.
- Tauri native no-bundle build: validated locally on macOS during review.
- JSON files and Prettier formatting: passing.
- Initial M0 commit is pushed to `origin/main`; the working tree is clean.

## Not implemented

- SQLite event store and migrations.
- Canonical action digests, approvals, and audit persistence.
- OS keychain/OAuth broker.
- Secure filesystem/Git/worktree/process brokers.
- Sidecar packaging and supervision from Rust.
- Real provider API or native runtime adapters.
- Executable Agent Skills scripts, MCP servers, plugins, or privileged Pi packages.
- Merge, push, remote control, team sync, or hosted control plane.

## Publication status

Local step 3 is complete and the final review found no blocker in the scoped M0 scaffold. The user approved step 4; the initial foundation is committed and pushed to the public GitHub repository.
