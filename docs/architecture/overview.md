# Architecture overview

Vea is a local-first desktop control plane with four trust-separated layers.

```text
React renderer (untrusted UI)
  -> narrow Tauri commands/channels
Rust host (policy, persistence, secrets, Git/files/process brokers)
  -> bounded versioned stdio
TypeScript agent service (DAG scheduler, router, Pi loop, adapters)
  -> supervised provider/API/runtime integrations
```

## Core decisions

- Tauri v2 desktop shell; React and TypeScript UI.
- Rust host is the sole privileged authority and SQLite writer.
- TypeScript sidecar owns orchestration logic but cannot add credentials or authorize actions.
- A task graph is durable and versioned; scheduling truth is not embedded in prompts.
- Every mutating attempt leases a unique branch and worktree.
- Direct model APIs and provider-native agent runtimes are distinct adapter types.
- Skills, MCP, restricted plugins, and privileged Pi compatibility are separate extension planes.
- Route decisions are deterministic, persisted, inspectable, and honest about unknown quota.

The complete design is in [`../PLAN.md`](../PLAN.md).
