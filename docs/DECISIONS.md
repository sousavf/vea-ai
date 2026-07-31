# Architecture decisions

## ADR-001 — Desktop-first Tauri and TypeScript

**Decision:** Tauri v2 hosts a React/TypeScript UI. Privileged policy and persistence live in Rust; orchestration runs in a version-pinned TypeScript sidecar.

**Reason:** The UI and adapter ecosystem benefit from TypeScript while Tauri provides a narrow native privilege boundary and cross-platform distribution.

## ADR-002 — Separate extension planes

**Decision:** Agent Skills, MCP, restricted plugins, and privileged Pi compatibility remain separate contracts.

**Reason:** They differ in lifecycle, authority, portability, and sandboxability. One universal plugin API would hide dangerous privilege transitions.

## ADR-003 — Direct models are not native agents

**Decision:** Direct provider model adapters and provider-native agent/CLI adapters implement distinct interfaces with capability negotiation.

**Reason:** Native runtimes own sessions, permissions, tools, and resume semantics that cannot be normalized losslessly.

## ADR-004 — Honest quota routing

**Decision:** Route optimization may use official telemetry, configured budgets, observed limit events, and conservative concurrency. Unknown subscription quota stays unknown.

**Reason:** Token cost is not subscription capacity, and most providers expose no stable remaining-quota API.

## ADR-005 — Worktrees isolate changes, not trust

**Decision:** Each mutating run leases a branch/worktree, but all file/process actions still pass host policy.

**Reason:** Git worktrees share repository metadata and cannot contain a malicious process.

## ADR-006 — MIT license

**Decision:** Vea is licensed under MIT.

**Reason:** The project prioritizes simple, broad reuse and contribution compatibility.
