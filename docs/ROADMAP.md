# Roadmap

The implementation sequence is security-first. Extension breadth never weakens the default trust boundary.

## M0 — Foundation

- [x] MIT repository structure and contribution/security docs
- [x] Tauri v2 + React desktop shell
- [x] Versioned TypeScript domain, configuration, protocol, scheduler, router, and extension-plane contracts
- [x] Sidecar handshake foundation and tests
- [ ] Rust-host action/capability schemas and canonical digest
- [ ] Target-specific sidecar packaging spike
- [ ] SQLite event store migration 0001

## M1 — Multi-project local control plane

- [ ] Trusted project onboarding
- [ ] Durable project, graph, task, attempt, and audit records
- [ ] Typed renderer queries and update channels
- [ ] Task graph editor with cycle diagnostics
- [ ] Restart/rehydration tests

## M2 — Safe parallel execution

- [ ] Canonical path containment broker
- [ ] Git worktree leases, drift detection, and recovery
- [ ] Deterministic scheduler wired to durable state
- [ ] Route decision/reservation ledger
- [ ] Exact approval UI and audit lineage

## M3 — First useful agent MVP

- [ ] OS credential-store broker
- [ ] Pi-owned agent loop and host-mediated patch tools
- [ ] Two reviewed direct API adapters
- [ ] Normalized streaming, usage, cancellation, diffs, and validation
- [ ] Signed internal builds and cross-platform security tests

## M4+ — Provider and ecosystem breadth

- [ ] Native agent adapters, one provider/review gate at a time
- [ ] Portable Agent Skills
- [ ] Reviewed MCP support, disabled by default
- [ ] Restricted out-of-process plugin SDK
- [ ] Explicit privileged Pi compatibility tier

See [`PLAN.md`](PLAN.md) for domain models, algorithms, acceptance criteria, dependencies, and risks.
