# Implementation progress

Updated: 2026-08-01

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
- Added the Rust `vea-policy` authorization core with independent capability grants, typed policy rules, bounded canonical actions, action/policy/state digests, short-lived approvals, and shared Rust/TypeScript golden vectors.
- Added the Rust `vea-store` durable SQLite core with checksummed transactional migrations, online pre-upgrade backups, exact-retry command receipts, optimistic aggregate revisions, append-only domain/audit ledgers, replayable project and side-effect projections, and fail-closed integrity checks.
- Added crash-safe side-effect lifecycle persistence; interrupted `started` actions become `unknown` on startup and are never replayed automatically.

## Validation

- TypeScript workspace typecheck: passing.
- Vitest unit suite: passing.
- Web production build: passing.
- Rust format, Clippy with warnings denied, and tests: passing.
- Tauri native no-bundle build: validated locally on macOS during review.
- JSON files and Prettier formatting: passing.
- Foundation, canonical-policy, and durable-store milestones are pushed to `origin/main`; the final store review found no landing blockers.

## Not implemented

- Durable graph, task, attempt, approval-consumption, and worktree-lease aggregates.
- Host/UI/store wiring for canonical actions, approvals, and typed renderer queries.
- OS keychain/OAuth broker.
- Secure filesystem/Git/worktree/process brokers.
- Sidecar packaging and supervision from Rust.
- Real provider API or native runtime adapters.
- Executable Agent Skills scripts, MCP servers, plugins, or privileged Pi packages.
- Merge, push, remote control, team sync, or hosted control plane.

## Publication status

The M0 foundation, canonical policy core, and durable event store are published. Provider execution and privileged broker wiring remain intentionally disabled.
