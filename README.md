# Vea

**A local-first desktop control plane for every coding agent.**

Vea coordinates multiple projects and parallel agents while selecting the provider, model, and reasoning effort that best fit each task. It is designed to use supported API credentials and existing provider CLI/subscription sessions without scraping credentials, bypassing limits, or hiding where project data is sent.

> Status: architecture and executable foundation. Vea is not yet ready for production use.

## What Vea is building

- **Many projects at once** — durable task graphs, fair scheduling, independent run state.
- **Parallel, isolated execution** — one branch and Git worktree per mutating attempt.
- **Task-aware routing** — capability, role, effort, budget, quota confidence, latency, and reliability all contribute to a visible route decision.
- **Provider freedom** — direct model APIs and native agent runtimes use separate, capability-aware adapters.
- **Pi-compatible ecosystem** — Agent Skills, MCP, restricted plugins, and an explicit privileged Pi compatibility tier are separate extension planes.
- **Honest subscription utilization** — documented telemetry is used when available; unknown quota remains unknown.
- **Local-first security** — the renderer has no direct shell, filesystem, keychain, or provider-secret authority.

## Repository map

```text
apps/desktop        Tauri v2 + React desktop UI
apps/agent-service  TypeScript scheduler/runtime sidecar foundation
crates/vea-policy   Canonical Rust action authorization core
crates/vea-store    Durable Rust SQLite event store and projections
packages/domain     Durable task and run contracts
packages/config     Versioned, validated user configuration
packages/scheduler  Deterministic cross-project scheduler
packages/routing    Model/provider/effort route selection
packages/protocol   Bounded sidecar framing protocol
packages/extensions Skills, MCP, plugin, and Pi compatibility contracts
schemas/            Public JSON Schemas
docs/PLAN.md        Implementation-ready architecture and roadmap
```

## Quick start

Prerequisites: Node.js 22+, Rust 1.94+, and the platform dependencies required by [Tauri v2](https://v2.tauri.app/start/prerequisites/).

```bash
corepack enable
pnpm install
pnpm check
pnpm dev:web       # browser UI with local demo state
pnpm dev           # Tauri desktop app
```

The first install may use `npx pnpm@11.18.0 install` if Corepack is unavailable.

## Configuration

Vea uses a versioned JSON configuration with no secrets in the file. Credentials are referenced by opaque IDs and will be stored through the OS credential store.

```bash
cp examples/vea.config.example.json ~/.config/vea/config.json
```

See [configuration documentation](docs/configuration.md) and [the schema](schemas/vea-config.schema.json).

## Safety boundary

A Git worktree prevents agents from colliding; it is **not a security sandbox**. Model output, repository text, skills, plugins, and MCP results are untrusted. Privileged actions must pass typed host policy and exact, digest-bound approval. Arbitrary shell, automatic push/merge, and arbitrary executable plugins/MCP servers are outside the safe MVP.

Read [SECURITY.md](SECURITY.md) and the [threat model](docs/security/threat-model.md) before contributing privileged functionality.

## Project status

The detailed build sequence, domain model, routing algorithm, security gates, tests, and milestones are in [docs/PLAN.md](docs/PLAN.md). The initial repository implements the shared contracts and UI shell so development can proceed in reviewable vertical slices.

## License

[MIT](LICENSE)
