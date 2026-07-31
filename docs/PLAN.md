# Implementation Plan

> This is the living architecture plan. It originated as a pre-scaffold planning artifact; the final provenance record is historical, while implementation status is tracked in [`ROADMAP.md`](ROADMAP.md) and [`PROGRESS.md`](PROGRESS.md).

## Goal

Deliver an MIT-licensed, desktop-first TypeScript/Tauri v2 control plane that runs multiple trusted local projects concurrently, executes durable task DAGs through isolated Git worktrees, routes work across documented provider APIs and official agent runtimes, and mediates every privileged action through a local security boundary.

## Scope and approved architecture decisions

- **Desktop shell:** Tauri v2 with a React/TypeScript renderer and a Rust privileged host.
- **Agent runtime:** a bundled, version-pinned TypeScript agent-service sidecar. It contains the Pi-owned loop, scheduler, routing logic, and adapter implementations, but does not become the security authority.
- **Source of truth:** SQLite owned exclusively by the Rust host. No cloud account or synchronization is required.
- **Extension planes:** MCP for tools/resources/prompts, Agent Skills for instructions, a versioned restricted TypeScript plugin facade, and separate adapters for provider-native agent SDKs/CLIs.
- **Provider boundary:** direct model APIs and provider-native agents are distinct adapter types. Authentication is limited to documented API, OAuth, SDK, and CLI contracts. Vea never scrapes browsers, token stores, private endpoints, or quota mechanisms.
- **Execution boundary:** one unique branch and worktree per mutating attempt. A worktree is concurrency isolation, not a security sandbox.
- **MVP privilege:** trusted local repositories, reviewed direct-provider adapters, no arbitrary shell, no arbitrary plugin or MCP executable, no automatic merge/push, and no unattended background operation.
- **Authority:** model output, repositories, skills, plugins, and MCP data may propose actions; only the Rust policy broker plus an action-digest-bound user approval may authorize side effects.

## Architecture overview

```text
Tauri webview (untrusted UI)
  | typed invoke commands; command-scoped Tauri Channels
  v
Rust desktop host (security and persistence authority)
  |- SQLite/event log/materialized views
  |- OS credential store and OAuth callback broker
  |- policy + approval/action-digest enforcement
  |- canonical path/file broker
  |- hardened Git/worktree broker
  |- provider HTTP transport (adds credentials after authorization)
  |- fixed executable/process supervisor
  `- signed/version-pinned updater
  |
  | versioned, bounded, length-prefixed stdio protocol
  v
Bundled TypeScript agent-service (trusted app code; crash boundary)
  |- deterministic DAG scheduler
  |- quota/budget router
  |- Pi-owned agent loop and normalized sessions
  |- direct model adapters using host provider transport
  |- native-agent adapters using host process supervisor
  |- skills catalog
  `- later: MCP clients and restricted plugin workers
       |
       `- one supervised process per CLI/MCP/plugin integration
```

The Rust host commits domain commands and events before acknowledging them. The agent-service rebuilds its scheduler state from a host snapshot plus events after restart. Provider text and tool proposals never execute inside the renderer or bypass the host policy layer.

## Concrete monorepo layout

```text
/
├── LICENSE
├── README.md
├── SECURITY.md
├── CONTRIBUTING.md
├── CODE_OF_CONDUCT.md
├── package.json
├── pnpm-lock.yaml
├── pnpm-workspace.yaml
├── turbo.json
├── tsconfig.base.json
├── rust-toolchain.toml
├── Cargo.toml
├── deny.toml
├── .github/
│   └── workflows/{ci.yml,release.yml,security.yml}
├── apps/
│   ├── desktop/
│   │   ├── package.json
│   │   ├── vite.config.ts
│   │   ├── src/
│   │   │   ├── app/App.tsx
│   │   │   ├── bridge/{desktopBridge.ts,mockBridge.ts}
│   │   │   ├── features/
│   │   │   │   ├── projects/
│   │   │   │   ├── graph/
│   │   │   │   ├── sessions/
│   │   │   │   ├── approvals/
│   │   │   │   ├── diffs/
│   │   │   │   └── settings/
│   │   │   └── state/{queries.ts,streamReducer.ts}
│   │   └── src-tauri/
│   │       ├── Cargo.toml
│   │       ├── build.rs
│   │       ├── tauri.conf.json
│   │       ├── capabilities/main.json
│   │       ├── icons/
│   │       └── src/{main.rs,commands.rs,state.rs}
│   └── agent-service/
│       ├── package.json
│       ├── src/{main.ts,server.ts,rehydrate.ts}
│       └── build/README.md
├── packages/
│   ├── protocol/
│   │   ├── schema/{ipc.json,events.json,config.json,adapter.json}
│   │   ├── src/{generated.ts,codec.ts,version.ts}
│   │   └── scripts/generate.ts
│   ├── domain/src/
│   │   ├── ids.ts
│   │   ├── project.ts
│   │   ├── taskGraph.ts
│   │   ├── run.ts
│   │   ├── routing.ts
│   │   ├── action.ts
│   │   ├── extensions.ts
│   │   └── events.ts
│   ├── config/src/{load.ts,validate.ts,merge.ts,defaults.ts}
│   ├── scheduler/src/{scheduler.ts,readiness.ts,conflicts.ts,fairQueue.ts}
│   ├── routing/src/{router.ts,eligibility.ts,effort.ts,quota.ts,score.ts}
│   ├── runtime-core/src/{agentRuntime.ts,modelProvider.ts,eventNormalizer.ts}
│   ├── adapter-pi/src/{runtime.ts,tools.ts,session.ts}
│   ├── adapters/
│   │   ├── anthropic-api/
│   │   ├── openai-api/
│   │   ├── google-api/
│   │   ├── openai-compatible/
│   │   ├── claude-agent/
│   │   ├── codex/
│   │   ├── gemini-cli/
│   │   └── copilot/
│   ├── skills/src/{catalog.ts,scanner.ts,activate.ts,provenance.ts}
│   ├── mcp/src/{client.ts,legacyClient.ts,transport.ts,schemaLowering.ts}
│   ├── plugin-sdk/src/{facade.ts,manifest.ts,capabilities.ts}
│   └── testkit/src/{fakeClock.ts,fakeRuntime.ts,fixtures.ts,contractSuite.ts}
├── crates/
│   ├── vea-store/src/{lib.rs,migrations.rs,commands.rs,queries.rs,audit.rs}
│   ├── vea-policy/src/{lib.rs,capability.rs,canonical_action.rs,decision.rs}
│   ├── vea-secrets/src/{lib.rs,keychain.rs,oauth.rs,redact.rs}
│   ├── vea-fs/src/{lib.rs,containment.rs,patch.rs,safe_delete.rs}
│   ├── vea-git/src/{lib.rs,inspect.rs,worktree.rs,lease.rs,integrate.rs}
│   ├── vea-process/src/{lib.rs,catalog.rs,supervisor.rs,limits.rs}
│   ├── vea-provider-transport/src/{lib.rs,http.rs,destination.rs,stream.rs}
│   └── vea-sidecar-ipc/src/{lib.rs,frame.rs,handshake.rs,supervisor.rs}
├── migrations/
│   ├── 0001_core.sql
│   ├── 0002_events_audit.sql
│   └── 0003_extensions.sql
├── schemas/
│   ├── vea-config.schema.json
│   ├── vea-project.schema.json
│   ├── plugin-manifest.schema.json
│   └── task-graph.schema.json
├── tests/
│   ├── contract/adapters/
│   ├── fixtures/{repos,events,provider-streams,malicious-inputs}/
│   ├── integration/{recovery,worktrees,policy,store}/
│   └── e2e/{mock-desktop,platform-smoke}/
└── docs/
    ├── architecture/{overview.md,security-boundary.md,events.md}
    ├── configuration.md
    ├── providers/{matrix.md,adapter-authoring.md}
    ├── extensions/{skills.md,mcp.md,plugins.md}
    └── release/{threat-model.md,signing.md,rollback.md}
```

`packages/adapters/*` use the same package template; only adapters admitted by the provider matrix are included in a release. The platform-specific sidecar build produces target-triple-named binaries/resources expected by Tauri v2's `bundle.externalBin`; the renderer never receives shell-plugin permission.

## Domain model

All IDs are UUIDv7 strings generated by the Rust host. Timestamps are RFC 3339 UTC plus monotonic sequence numbers for event ordering. Optimistic revisions reject stale UI or sidecar commands.

| Entity               | Required fields and invariants                                                                                                                                                                                                                          |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Project`            | `id`, `displayName`, canonical `repoRoot`, `repoIdentity`, default branch, `trustState`, provider/data policy, created/updated timestamps. A project must be explicitly selected and trusted before repository config or skills are read.               |
| `TaskGraph`          | `id`, `projectId`, `version`, `title`, `status`, `baseRevision`, node IDs, edge list. Published versions are immutable; edits create a new version. The graph must be acyclic.                                                                          |
| `TaskNode`           | `id`, `graphId`, `kind`, `title`, instructions, `dependsOn`, role, effort, priority, read/write globs, required capabilities, routing override, budget, validation spec, retry policy, integration policy. Scheduling truth lives here, not in prompts. |
| `TaskEdge`           | `from`, `to`, `condition` (`succeeded`, `settled`, or named output predicate). MVP supports `succeeded`; richer predicates are post-MVP.                                                                                                                |
| `RunAttempt`         | `id`, `taskId`, `attempt`, state, route-decision ID, session ID, worktree lease ID, budget/reservation, start/end, terminal reason. A mutating attempt has exactly one worktree lease.                                                                  |
| `AgentSession`       | normalized session ID, runtime adapter/version, provider-native session ID, capability snapshot, sequence cursor, resume compatibility. Provider-native IDs are never the host primary key.                                                             |
| `WorktreeLease`      | project, attempt, canonical path, unique branch, base/initial HEAD, expected current HEAD, lease state, owner PID/instance, dirty-state digest. One active mutator per worktree/branch.                                                                 |
| `RouteDecision`      | eligible/rejected candidates and reasons, chosen adapter/account/model, normalized/provider effort, estimate, reservation, policy version, override source. It is immutable and auditable.                                                              |
| `ProviderAccount`    | adapter ID, account alias, auth type, opaque `credentialRef`, terms-review version, scopes, status. It contains no secret value.                                                                                                                        |
| `ModelDescriptor`    | stable key, provider model ID, modality/tool/context capabilities, supported efforts, price metadata, retirement/version status. Catalog observations are timestamped.                                                                                  |
| `QuotaSnapshot`      | account, source (`official`, `configured_budget`, `observed_limit`, `unknown`), window, remaining amount if known, confidence, observed time, expiry. Unknown is never converted into a fake percentage.                                                |
| `QuotaReservation`   | route, estimated tokens/cost/turns, expiry, state, actual usage. API budgets use numeric reservations; subscriptions only use documented telemetry or conservative concurrency slots.                                                                   |
| `ActionProposal`     | stable action ID, run, capability, typed operation, canonical resource/destination/args, provenance, reversibility, risk class, policy version. Model prose cannot be an action.                                                                        |
| `Approval`           | action ID, canonical digest, exact display payload, actor, decision, expiry, time. Mutation, expiry, target drift, or policy change invalidates it.                                                                                                     |
| `Artifact`           | run, type (`patch`, `commit`, `diff`, `validation`, `structured-output`), local reference, content digest, provenance, size. Audit records use summaries rather than artifact contents.                                                                 |
| `IntegrationAttempt` | source branch/commit range, target branch and expected HEAD, diff digest, approval, result/conflicts. Integration is serialized per target branch and never pushes automatically.                                                                       |
| `SkillRecord`        | name, scope, source path, content hash, provenance, validation diagnostics, enabled state. `allowed-tools` is advisory only.                                                                                                                            |
| `IntegrationRecord`  | MCP/plugin identity, kind, version/digest, manifest, approved capabilities, trust tier, enabled projects. Executable integrations default disabled.                                                                                                     |
| `DomainEvent`        | event ID, aggregate ID/type, aggregate revision, global sequence, schema version, kind, redacted payload, causation/correlation IDs. Events are append-only.                                                                                            |
| `AuditEvent`         | actor, project/run/action IDs, policy decision, approval digest, provider/account alias, canonical affected paths/destination, result class. No raw secret, environment, source, prompt, or tool-output field.                                          |

### State machines

- `TaskNode`: `draft -> waiting -> ready -> queued -> running -> {succeeded, failed, cancelled, blocked}`. A failed dependency makes a downstream node `blocked` unless its edge policy permits settled input.
- `RunAttempt`: `created -> routing -> reserving -> provisioning -> awaiting_approval -> starting -> running -> validating -> {succeeded, failed, cancelled, interrupted, unknown_outcome}`.
- `WorktreeLease`: `requested -> provisioned -> active -> {preserved, integrating, cleaning} -> released`; crash recovery moves unverified leases to `preserved`, never directly to deleted.
- `ActionProposal`: `proposed -> policy_denied | approval_required -> approved -> executing -> {succeeded, failed, unknown_outcome}`. Only read-only, predeclared broker operations may be policy-allowed without UI confirmation; MVP confirmations remain mandatory for provider submission and every side effect listed in the security review.

### Persistence

- SQLite runs in WAL mode with foreign keys enabled and one Rust-host writer. Migrations are transactional and backed up before incompatible upgrades.
- `domain_events` is append-only; materialized tables (`projects`, `task_graphs`, `tasks`, `attempts`, `leases`, `route_decisions`, `approvals`, `artifacts`) are updated in the same transaction.
- `side_effects` stores an idempotency/action ID and phases `authorized`, `started`, `finished`, `unknown`. Startup recovery never replays `started` writes automatically.
- Session messages are local-only application data with retention controls. Audit logs are a separate metadata-first stream. Credentials, OAuth codes, auth headers, child environments, and CLI credential content are never stored.
- Database and log files use per-user OS permissions/ACLs. At-rest encryption is not advertised as protection against the same logged-in OS user; optional database encryption can be added only after cross-platform key-recovery design.

## Adapter interfaces

The following contracts belong in `packages/runtime-core`; their schemas are mirrored in `packages/protocol` so no adapter-specific object crosses a process boundary.

```ts
export type Effort = "off" | "low" | "medium" | "high" | "max";

export interface RuntimeCapabilities {
  attachments: readonly ("image" | "audio" | "file-ref")[];
  structuredOutput: boolean;
  tools: boolean;
  steering: boolean;
  fork: boolean;
  resume: "none" | "same-version" | "portable";
  skills: "host" | "native" | "none";
  mcpOwnership: "host" | "runtime" | "none";
  sandboxModes: readonly string[];
  efforts: readonly Effort[];
  rawEventNamespaces: readonly string[];
}

export interface AgentRuntimeAdapter {
  readonly id: string;
  readonly version: string;
  probe(ctx: ProbeContext): Promise<RuntimeProbe>;
  capabilities(ctx: AccountContext): Promise<RuntimeCapabilities>;
  start(input: StartRunInput, signal: AbortSignal): AsyncIterable<RuntimeEvent>;
  resume(input: ResumeRunInput, signal: AbortSignal): AsyncIterable<RuntimeEvent>;
  send(input: SendInput, signal: AbortSignal): Promise<void>;
  steer?(input: SteerInput, signal: AbortSignal): Promise<void>;
  fork?(input: ForkInput, signal: AbortSignal): Promise<NativeSessionRef>;
  cancel(run: RuntimeRunRef): Promise<CancelResult>;
  dispose(session: RuntimeSessionRef): Promise<void>;
}

export interface ModelProviderAdapter {
  readonly id: string;
  readonly version: string;
  probe(ctx: ProviderTransportContext): Promise<ProviderProbe>;
  listModels(ctx: ProviderTransportContext): Promise<ModelDescriptor[]>;
  normalizeEffort(model: ModelDescriptor, effort: Effort): ProviderEffort;
  lowerToolSchema(input: CanonicalToolSchema): SchemaLoweringResult;
  stream(
    request: ModelRequest,
    ctx: ProviderTransportContext,
    signal: AbortSignal,
  ): AsyncIterable<ModelStreamEvent>;
}
```

`ProviderTransportContext` exposes only a host RPC handle, account alias, approved destination, and correlation ID. It never contains a key. The Rust host validates provider/path/method/body limits, adds the credential immediately before TLS transport, strips auth data from errors, and returns bounded stream frames.

```ts
export interface RuntimeEventEnvelope {
  schemaVersion: 1;
  runtime: { id: string; version: string };
  hostSessionId: string;
  sequence: number;
  timestamp: string;
  kind:
    | "session.state"
    | "message.delta"
    | "message.final"
    | "tool.proposed"
    | "tool.started"
    | "tool.progress"
    | "tool.completed"
    | "approval.requested"
    | "approval.resolved"
    | "usage"
    | "warning"
    | "error"
    | "run.settled";
  correlationId?: string;
  payload: unknown;
  raw?: { namespace: string; type: string; value: unknown };
}
```

Unknown native events are retained only under a namespaced, size-bounded `raw` field. Chain-of-thought is never required or persisted as a portable field. Unsupported fork, steering, approval, or resume semantics return typed `UNSUPPORTED_CAPABILITY`; adapters must not silently emulate them.

Additional host contracts:

```ts
export interface HostBridge {
  commit(command: DomainCommand): Promise<CommandResult>;
  query<T extends HostQuery>(query: T): Promise<HostQueryResult<T>>;
  proposeAction(action: ActionProposal): Promise<ActionDecision>;
  providerStream(request: AuthorizedProviderRequest): AsyncIterable<ProviderFrame>;
  spawnReviewedRuntime(request: ReviewedRuntimeSpawn): AsyncIterable<ProcessFrame>;
}

export interface Scheduler {
  tick(snapshot: SchedulingSnapshot, now: Instant): SchedulerDecision[];
}

export interface Router {
  route(request: RouteRequest, snapshot: RoutingSnapshot): RouteResult;
}
```

Every adapter must pass a common conformance suite for handshake/version mismatch, partial frames, stderr noise, unknown events, cancellation, approval correlation, usage normalization, timeout, crash, and version-incompatible resume.

## Configuration schema

### Locations and precedence

1. Product defaults bundled read-only.
2. User config at the platform configuration directory (`$XDG_CONFIG_HOME/vea/config.json`, macOS Application Support equivalent, or Windows AppData equivalent).
3. Per-project `.vea/project.json`, read only after project trust and only for non-secret, project-scoped fields.
4. Per-task graph policy and explicit one-run UI override.

Later layers may tighten security. Project config cannot add credentials, executable paths, plugin/MCP processes, arbitrary endpoints, or weaken approval/data policies. Credentials are referenced by opaque IDs created through UI flows. Unknown keys and unsupported `configVersion` fail closed with path-specific diagnostics.

### User configuration v1

```json
{
  "$schema": "https://vea.dev/schemas/vea-config-v1.json",
  "configVersion": 1,
  "ui": {
    "theme": "system",
    "density": "comfortable",
    "confirmProviderSubmission": true
  },
  "scheduler": {
    "globalMaxRuns": 4,
    "perProjectMaxRuns": 2,
    "defaultProviderMaxRuns": 2,
    "fairness": "weighted-deficit-round-robin",
    "projectWeights": {},
    "unknownWriteScopeConflicts": true,
    "leaseHeartbeatSeconds": 10
  },
  "accounts": [
    {
      "id": "anthropic-primary",
      "adapter": "anthropic-api",
      "auth": { "type": "apiKey", "credentialRef": "cred_opaque" },
      "enabled": true,
      "allowedProjects": ["*"]
    }
  ],
  "models": {
    "planner": {
      "account": "anthropic-primary",
      "model": "provider-model-id",
      "effortMap": {
        "low": "provider-low",
        "medium": "provider-medium",
        "high": "provider-high"
      }
    }
  },
  "routing": {
    "defaultPolicy": "balanced",
    "policies": {
      "balanced": {
        "roles": {
          "plan": ["planner"],
          "implement": ["planner"],
          "review": ["planner"]
        },
        "allowCrossProviderFallback": false,
        "maxEstimatedCostUsdPerTask": 5,
        "preferSubscription": true
      }
    }
  },
  "security": {
    "trustedProjects": [],
    "providerDataRules": {},
    "contentLogRetentionDays": 30,
    "auditRetentionDays": 90,
    "diagnosticContentLogging": false
  },
  "skills": {
    "enabled": true,
    "userRoots": ["~/.agents/skills"],
    "maxFilesPerSkill": 100,
    "maxBytesPerSkill": 1048576
  },
  "mcp": { "enabled": false, "servers": [] },
  "plugins": { "enabled": false, "packages": [] }
}
```

The JSON Schema constrains integer ranges, enums, IDs, URI schemes, byte limits, and `additionalProperties: false`. A custom endpoint is accepted only by an explicitly enabled adapter such as `openai-compatible`, is shown as a distinct data destination, and cannot impersonate a built-in provider ID.

### Project configuration v1

```json
{
  "$schema": "https://vea.dev/schemas/vea-project-v1.json",
  "configVersion": 1,
  "project": {
    "displayName": "Example",
    "defaultBaseBranch": "main",
    "providerPolicy": "balanced",
    "allowedAccounts": ["anthropic-primary"],
    "dataClassification": "private"
  },
  "tasks": {
    "defaultEffort": "medium",
    "defaultBudget": { "maxTurns": 30, "maxMinutes": 30 },
    "validationProfiles": {
      "safe-static": ["broker.gitDiffCheck", "broker.patchContainmentCheck"]
    }
  },
  "skills": { "roots": [".agents/skills"], "enabled": [] }
}
```

No command string, hook, script, environment variable, credential, MCP server, or plugin package is legal in project configuration in the MVP.

## Scheduler and routing algorithm

### DAG validation and readiness

1. On graph publish, verify node/edge IDs, perform Kahn topological sorting, reject cycles, validate dependency/output references, normalize read/write globs, and require a base revision.
2. In a single host transaction, mark a waiting node `ready` when all required predecessors succeeded and required artifacts exist. Mark it `blocked` if a required predecessor terminally failed.
3. Rehydrate timers and attempts from durable events. A crash cannot create a second attempt for an action whose outcome is unknown.

### Scheduling tick

The scheduler is a deterministic pure function over a versioned snapshot and injected clock:

1. Collect ready nodes across all open projects.
2. Exclude nodes violating global, per-project, per-adapter, per-account, or user-configured concurrency limits.
3. Exclude mutating nodes whose declared write set is unknown or overlaps another active mutating task in the same project when conservative conflict mode is enabled. Path scopes are canonical project-relative POSIX globs; `**`, absent scopes, and rename across scopes conflict with everything. Variant/Best-of-N runs may share a logical task but always use separate worktrees and never integrate concurrently.
4. Exclude nodes lacking a safe worktree slot, current repository trust, a valid base revision, or a routable candidate.
5. Order each project's candidates by priority, critical-path rank, ready time, then stable task ID.
6. Select projects with weighted deficit round robin so one busy repository cannot starve others. Cost is the task's effort weight (`low=1`, `medium=2`, `high=4`, `max=6`).
7. Request a route and quota reservation. Commit `RunAttemptCreated`, `RouteChosen`, and `QuotaReserved` atomically before provisioning.
8. Provision a unique worktree/branch, record its initial HEAD/status, then request any required provider-data/action approval.
9. Dispatch only after the host returns the committed attempt revision and valid approval digest.
10. On completion, persist usage, release the reservation delta, run declared broker validations, and transition the task. Integration is a separate serialized, confirmed operation.

Scheduler decisions carry explicit reason codes (`dependency`, `conflict`, `concurrency`, `quota`, `approval`, `trust`, `route`, `lease`) for the UI.

### Routing

1. **Build candidates:** expand model aliases into `(adapter, account, runtime, model, effort)` options.
2. **Hard eligibility:** require enabled terms-reviewed adapter; healthy/version-compatible runtime; documented authentication; project/account allowance; input modality, context, tool, structured-output, sandbox, and effort capabilities; data-destination policy; remaining API budget; and concurrency capacity.
3. **Task override:** honor an explicit task/run override only if it remains eligible. Otherwise return a visible error rather than silently substitute.
4. **Normalize effort:** map Vea's `off/low/medium/high/max` through adapter/model metadata. If the requested level is unsupported, choose the nearest lower eligible level only when policy permits; otherwise reject.
5. **Estimate and reserve:** estimate input/output tokens, cost, turns, and wall time from task history and adapter metadata. Reserve numeric API cost/token budgets atomically. For subscriptions, use only official limit events or configured concurrency ceilings; mark remaining quota `unknown` when it is unknown.
6. **Score eligible candidates:** deterministic weighted score from capability fit (35%), quota/budget headroom (20%), configured role preference (15%), historical success/reliability (10%), latency (10%), estimated monetary cost (5%), and current adapter load (5%). Normalize inputs to `[0,1]`; stable alias ID breaks ties. Persist each component.
7. **Fallback:** a provider limit can downgrade effort/model within the same approved account if policy permits. Crossing provider/account/data destination requires a preapproved fallback and renewed destination disclosure; it is never silent.
8. **Failure:** distinguish transient transport, official rate/limit, context overflow, policy denial, adapter incompatibility, and ambiguous side effect. Retry only idempotent reads or a provider request with a supported idempotency key. Never auto-retry a write/tool action with ambiguous outcome.

The router optimizes documented capacity; it does not evade rolling limits, parallel-session restrictions, or consumer-product terms.

## IPC and process topology

### Renderer to Rust host

- Define a small command surface: project list/open/trust, graph/query/mutate, run start/cancel, approval resolve, artifact/diff read, provider account setup, settings update, and audit query/export/delete.
- Use Tauri v2 commands for request/response and command-scoped `Channel` values for long-lived streams. Do not use a globally addressable event as authority for approvals or side effects.
- Validate all DTOs in Rust against generated protocol types, verify window label and current app state, impose message/collection/string limits, and return typed error codes.
- `capabilities/main.json` grants only the custom commands and minimum dialog/window functionality. It grants no renderer shell, generic filesystem, process, keychain, or updater invocation.
- Bundle only local UI assets; enforce a strict CSP, disable remote script/content execution, and open external URLs through an allowlisted Rust command into the system browser.

### Rust host to agent-service

- The host alone launches the fixed bundled sidecar declared through Tauri v2 `externalBin`. Startup verifies the packaged digest/version before use.
- Use stdin/stdout anonymous pipes with a 4-byte length prefix and JSON payload, not newline parsing. First frames negotiate protocol range, build ID, instance nonce, maximum frame size, and feature flags. Anonymous parent-owned pipes provide endpoint binding; no listening TCP port is opened.
- Every frame has `protocolVersion`, `requestId`, `correlationId`, `sequence`, `kind`, and payload. Default maximum frame is 1 MiB; large artifacts use host-issued local handles and chunked reads.
- Apply bounded queues, producer backpressure, request deadlines, heartbeat, cancellation, stderr/output caps, and kill-the-process-tree on host exit. Stdout is protocol-only; bounded stderr becomes redacted diagnostic metadata.
- On sequence gaps, malformed frames, or version mismatch, terminate and mark in-flight attempts `interrupted` or `unknown_outcome` according to their side-effect phase. Restart with exponential backoff and a cap, then rehydrate from the host.

### Child runtimes

- Only the Rust process broker can launch a reviewed executable. It resolves an absolute pinned path, uses an argv array with `shell=false`, a fixed cwd, minimal environment, closed handles, timeout/output limits, and process-tree cancellation.
- A documented CLI owns its existing login session. Vea invokes only its structured SDK/JSONL/app-server contract and never reads or copies its credential files.
- Each native CLI, future MCP server, and future plugin worker gets a separate supervised process where feasible. Their raw data is untrusted and passes schema/policy checks.

### Pi-like UI information architecture

- Left project rail: simultaneous project status, active/queued/blocked counts, provider-limit warnings.
- Project column: task DAG/Kanban toggle, dependency/critical-path indicators, worktree/branch state.
- Main pane: session transcript with normalized tool/approval cards, plan/execute split, streaming and cancellation.
- Right inspector: exact diff/artifacts, route rationale, model/effort/account, quota confidence, budget/usage, validation, and audit lineage.
- Bottom composer: task/run scope, provider/model/effort override, attachments, and data-destination disclosure.
- Approval sheet: exact canonical argv/structured request, affected paths, destination/data classes, reversibility, digest-linked decision, and no blanket “always allow everything.”

## Security model

### Trust boundary and central invariant

The renderer, model output, repository content, provider responses, skills, plugin metadata/code, and MCP descriptions/results are untrusted. The bundled Rust host and signed bundled agent-service are trusted application code; the host is the only authorization authority. The host OS, OS credential store, and current desktop user are trusted. Same-user malware, administrators, kernel compromise, and harmful actions knowingly approved by the user are out of scope and must remain documented residual risks.

### Mandatory controls

- **Credentials:** OS keychain only; SQLite stores opaque references. Provider HTTP auth is attached inside the Rust transport after policy approval. OAuth uses the system browser, Authorization Code + PKCE S256, state/nonce, exact loopback redirect, ephemeral port, one-time callback, minimal scopes, timeout, disconnect, and revocation UX.
- **Provider terms:** `docs/providers/matrix.md` is a release gate containing permitted API/SDK/CLI, auth, concurrency/rate rules, retention/training, branding, reviewed date, and owner. No adapter ships without legal/product review.
- **Actions:** versioned schemas, canonical encoding, size/path/enum validation, central capability matrix, exact digest binding, short expiry, destination-aware disclosure, and reapproval on any mutation or target drift.
- **Filesystem:** canonical realpath containment at authorization and immediately before access; reject escapes, NUL, device paths, alternate streams, symlink/junction traversal, and protected `.git` administration paths. New file creation uses safe parent traversal and non-following operations where supported.
- **Git:** trusted repositories only; unique worktree/branch per mutating attempt; leases and HEAD/dirty drift checks; no submodule initialization, hooks, package scripts, automatic merge, or push. MVP repository inspection rejects active clean/smudge/process filters and other checkout-time executable configuration. Integration displays commit range/diff and requires current-target-HEAD-bound approval.
- **Commands:** no arbitrary shell. Broker catalog entries declare fixed executable, argument schema, cwd rule, environment allowlist, network policy, output/time limits, idempotency, and approval class. Broad command execution remains disabled until per-OS confinement is validated.
- **Content:** provenance follows file/web/provider/MCP/skill content through summaries. Content cannot grant capabilities. Bound turns, time, tokens/cost, bytes read/sent, tool result size, and attachment size.
- **Audit/privacy:** redact structured secret fields before persistence; metadata-first audit; local telemetry and crash reporting off by default; retention/export/delete controls; canary-secret tests across DB/logs/IPC/errors/process env/argv.
- **Supply chain:** lock dependencies, `cargo deny`, npm audit policy, SBOM, reproducible release inputs, signed app/update artifacts, sidecar digest verification, rollback instructions, and protected release keys.

### Extension security tiers

- **Skills:** host-parsed untrusted instructions with no authority. Scan bounded roots without escaping symlinks, after project trust; project overrides user deterministically. Record provenance/hash/license. Never auto-run scripts. `allowed-tools` is advisory.
- **MCP:** dual-era client eventually supports current stdio and Streamable HTTP; legacy SSE is compatibility-only. Core MVP execution is disabled or limited to bundled reviewed servers, pinned digest, fixed manifest, and explicit per-project enablement. Remote MCP adds OAuth, TLS, redirect/IP/DNS/SSRF defenses and is post-MVP.
- **Restricted plugins:** versioned worker facade and manifest with immutable digest and explicit file/network/process/provider/UI capabilities. Out-of-process, brokered resources, no ambient secrets/filesystem. This is the default future marketplace tier.
- **Full Pi compatibility:** privileged mode in the agent-service, clearly labeled as equivalent to installing software and never advertised as sandboxed. It is post-MVP and requires explicit trust/review per package.

## Tasks

1. **Bootstrap the MIT monorepo and protocol generation.**
   - Files: root manifests; `apps/desktop/*`; `apps/agent-service/*`; `packages/protocol/*`; `Cargo.toml`; CI workflows.
   - Changes: establish pnpm/Cargo workspaces, pinned runtimes, formatting/lint/typecheck/test commands, MIT metadata, JSON Schema generation to TypeScript/Rust, and a minimal Tauri v2 app plus fixed sidecar handshake.
   - Acceptance: clean install/build/test on macOS, Windows, and Linux CI; renderer can request a version handshake but has no shell/fs permissions; generated protocol files are reproducible.

2. **Implement durable domain commands, events, and migrations.**
   - Files: `packages/domain/*`, `crates/vea-store/*`, `migrations/0001_core.sql`, `migrations/0002_events_audit.sql`.
   - Changes: implement IDs, aggregate revisions, state transitions, append-only event and audit streams, materialized queries, idempotent command IDs, snapshots, backup/migration, retention, and crash recovery phases.
   - Acceptance: stale revisions fail, duplicate command IDs do not duplicate effects, restart rebuilds identical state, and migration rollback restores the pre-upgrade DB.

3. **Create the typed Tauri bridge and Pi-like UI shell.**
   - Files: `apps/desktop/src/bridge/*`, feature directories, `src-tauri/src/{commands,state}.rs`, `capabilities/main.json`, `tauri.conf.json`.
   - Changes: implement narrow invoke/query commands, channel streaming, local-only CSP, project rail/task/main/inspector/composer layout, mock bridge, loading/error/empty states, and accessibility baselines.
   - Acceptance: multiple projects render independent live states; no provider secret or raw privileged object reaches renderer state; keyboard navigation and screen-reader labels cover the core workflow.

4. **Implement central policy, canonical actions, approval, and audit.**
   - Files: `crates/vea-policy/*`, `crates/vea-store/src/audit.rs`, `apps/desktop/src/features/approvals/*`.
   - Changes: define capabilities/action schemas, deterministic canonical encoding/digest, policy decisions, expiry/drift invalidation, exact approval UI, side-effect phase logging, and metadata redaction.
   - Acceptance: a changed byte in args/path/destination/policy invalidates approval; unauthorized proposals cannot reach execution; seeded secrets are absent from persisted audit and UI events.

5. **Build secure project, path, and worktree brokers.**
   - Files: `crates/vea-fs/*`, `crates/vea-git/*`, `tests/fixtures/repos/*`, `tests/integration/worktrees/*`.
   - Changes: explicit project trust, canonical containment, safe patch staging, Git configuration inspection, unique branch/worktree creation, leases/heartbeats, drift detection, preservation/cleanup, and separate confirmed integration.
   - Acceptance: adversarial traversal/symlink/junction/Unicode/case/`.git` tests do not escape; two attempts never own one worktree; hooks/filters/submodules do not execute; crash preserves recoverable work.

6. **Implement the Rust/TypeScript sidecar protocol and supervision.**
   - Files: `crates/vea-sidecar-ipc/*`, `apps/agent-service/src/*`, `packages/protocol/schema/ipc.json`, `apps/agent-service/build/README.md`.
   - Changes: length-prefixed codec, version handshake, target-specific externalBin packaging, frame limits, backpressure, heartbeat, cancellation, restart/rehydration, digest verification, and structured diagnostics.
   - Acceptance: partial/oversized/malformed/out-of-order frames fail closed; process trees terminate with the app; sidecar crash does not corrupt state or replay a side effect; packaged sidecar starts on all release platforms.

7. **Implement deterministic task DAGs and the fair scheduler.**
   - Files: `packages/scheduler/*`, `packages/domain/src/taskGraph.ts`, `schemas/task-graph.schema.json`, graph UI.
   - Changes: DAG publish/versioning, cycle detection, readiness/blocked transitions, critical-path rank, conflict checks, weighted deficit round robin, concurrency limits, reason codes, cancellation, and fan-out/fan-in representation.
   - Acceptance: deterministic property tests cover graph order, no starvation, limit compliance, dependency blocking, write-conflict serialization, and parallel execution across safe worktrees/projects.

8. **Implement routing, quota, budgets, and model-relative effort.**
   - Files: `packages/routing/*`, routing domain/config files, UI route inspector.
   - Changes: candidate eligibility, capabilities, normalized effort maps, deterministic scoring, API reservations, honest unknown subscription quota, same-account downgrade, approved cross-provider fallback, and persisted rationale.
   - Acceptance: ineligible/terms-disabled routes never win; equal snapshots produce equal choices; reservations prevent oversubscription; fallback destination changes require renewed disclosure; UI distinguishes cost, quota, and confidence.

9. **Implement credential and official provider transport brokers.**
   - Files: `crates/vea-secrets/*`, `crates/vea-provider-transport/*`, provider account UI, `docs/providers/matrix.md`.
   - Changes: keychain CRUD, desktop PKCE where supported, credential-free TS transport handles, destination/body/stream limits, TLS/error redaction, revoke/rotate, and terms/privacy matrix gates.
   - Acceptance: canary key is absent from SQLite/logs/renderer/argv/env/errors; OAuth state/PKCE/replay/redirect tests pass; an adapter cannot redirect a built-in credential to another host.

10. **Add the Pi-owned loop and initial direct API adapters.**
    - Files: `packages/adapter-pi/*`, `packages/runtime-core/*`, reviewed initial packages under `packages/adapters/*`.
    - Changes: normalized sessions/events/tools, host-mediated file/patch tools, compaction-safe provenance, provider stream parsing, usage, cancellation, capability probes, and schema lowering diagnostics.
    - Acceptance: two reviewed direct APIs pass the adapter contract suite, can run in parallel in separate worktrees, propose bounded patches, and resume only when adapter/version compatibility permits.

11. **Add reviewed native-agent/CLI adapters without credential import.**
    - Files: `crates/vea-process/*`, `packages/adapters/{claude-agent,codex,gemini-cli,copilot}/*`, adapter fixtures/docs.
    - Changes: fixed executable catalog, version probe/handshake, structured JSONL/app-server/SDK protocols, native capability documents, minimal environment, timeout/output caps, approvals, crash and limit normalization.
    - Acceptance: each enabled adapter has provider-matrix approval and passes conformance tests; unsupported capabilities are explicit; Vea never reads the CLI credential store; unknown stderr/frames cannot become actions.

12. **Implement portable Skills.**
    - Files: `packages/skills/*`, `docs/extensions/skills.md`, skills UI.
    - Changes: trusted-project-gated scans of `.agents/skills` and user roots, bounded/symlink-safe traversal, lenient safe import, strict export, deterministic precedence, progressive activation, provenance/hash, compaction preservation, and audit.
    - Acceptance: malformed/oversized/escaping skills are skipped with diagnostics; activation grants no tool; scripts never auto-run; duplicate activation is deduplicated.

13. **Introduce MCP behind an explicit security release gate.**
    - Files: `packages/mcp/*`, host process/network policies, `schemas/plugin-manifest.schema.json`, MCP docs/tests.
    - Changes: first ship bundled reviewed stdio server support disabled by default; then dual-era discovery/initialize, current Streamable HTTP, canonical/provider-lowered schemas, namespacing, consent invalidation, OAuth/SSRF controls. Keep legacy SSE opt-in compatibility-only.
    - Acceptance: MVP bundled-server identity/digest/manifest changes invalidate consent; post-MVP protocol fixtures cover both eras, schema limits, redirects/DNS/private ranges, cancellation, and malicious metadata/results.

14. **Add restricted plugins before privileged Pi package compatibility.**
    - Files: `packages/plugin-sdk/*`, plugin worker/supervisor, manifest schema/docs.
    - Changes: versioned facade, capability manifest, immutable package digest, per-update permission diff, brokered files/network/provider/UI, worker limits and kill switch. Add full Pi-compatible mode only as a separately labeled privileged trust tier.
    - Acceptance: restricted plugins lack ambient desktop/keychain/filesystem/process access; undeclared calls fail; digest/manifest updates require approval; full-compat mode cannot be enabled implicitly.

15. **Harden release, recovery, privacy, and documentation.**
    - Files: CI/release/security workflows, `SECURITY.md`, `docs/release/*`, end-to-end/platform tests.
    - Changes: dependency policies, SBOM, signed builds/updates, sidecar integrity, release-key and rollback runbooks, retention/export/delete UX, provider disclosures, crash recovery, performance budgets, and security claims review.
    - Acceptance: signed artifacts pass fresh-machine smoke tests; rollback is rehearsed; no staged/untracked build secret enters artifacts; the P0 adversarial suite passes on every supported OS before enabling privileged features.

## MVP acceptance criteria

The MVP is complete only when all of the following hold:

1. A user can trust and open at least two local Git projects and see both in the project rail without a Vea cloud account.
2. The user can create/publish a validated task DAG with dependencies, roles, effort, budgets, and declared write scopes; cycles are rejected with actionable diagnostics.
3. Ready tasks from two projects execute concurrently while global/project/provider limits and weighted fairness are visible and enforced.
4. Every mutating attempt gets a unique branch/worktree and lease. The original checkout is not modified. Unexpected HEAD/dirty drift blocks integration.
5. The product supports at least two reviewed official direct API adapters. A documented native-agent/CLI adapter may ship only after its provider matrix and conformance gate pass.
6. Route choice is deterministic and inspectable, records rejected candidates and effort mapping, reserves API budget, and displays subscription quota as unknown unless documented telemetry exists.
7. A task can stream normalized messages, usage, tool proposals, approval requests, validation, diffs, and terminal state; cancel kills the relevant process tree and leaves recoverable state.
8. The renderer has no shell, filesystem, keychain, raw database, or provider-secret authority. All privileged requests pass typed Rust commands and central policy.
9. Provider submission, patch application, broker command execution, worktree integration, destination change, and credential use display exact digest-bound confirmation. Mutation or drift forces reapproval.
10. Credentials use the OS keychain or a documented CLI-owned session; seeded secrets do not appear in app state, SQLite, logs, IPC fixtures, child argv/env, crash fixtures, or repository changes.
11. Arbitrary shell, third-party plugins, arbitrary MCP, remote control, submodules/hooks/filters, package scripts, automatic merge/push, and blanket approvals remain unavailable.
12. Skills are declarative only, project discovery requires trust, and no skill script or `allowed-tools` field grants capability.
13. Restart during every run/action phase reconstructs durable state without duplicate side effects. Unknown outcomes require user inspection.
14. Audit history correlates user intent, route, policy, approval digest, side effect, paths/destination, and result while remaining metadata-first and locally deletable/exportable.
15. macOS, Windows, and Linux release builds pass the supported-platform security/worktree/sidecar smoke suite; signed artifacts, SBOM, provider disclosures, and rollback instructions exist.

## Test plan

| Layer               | Tests                                                                                                                                                                                          | Primary locations                                                                      |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Domain/store        | State-machine tables, event/materialization atomicity, optimistic revisions, idempotent commands, migration/backup, retention, corruption/restart fixtures                                     | `packages/domain/**/*.test.ts`, `crates/vea-store/tests`, `tests/integration/recovery` |
| DAG/scheduler       | Cycle/topology properties, dependency predicates, critical path, stable ordering, fairness/no starvation, concurrency, conflict globs, cancellation, clock determinism                         | `packages/scheduler/**/*.test.ts`                                                      |
| Routing             | Eligibility truth tables, effort mapping, score snapshots, tie stability, budget reservation races, unknown quota, fallback disclosure, limit normalization                                    | `packages/routing/**/*.test.ts`                                                        |
| Protocol/IPC        | Schema golden files, TS/Rust round trip, incompatible version, fuzzed lengths/payloads, partial frames, sequence gaps, backpressure, cancellation, crash/restart                               | `packages/protocol`, `crates/vea-sidecar-ipc/tests`                                    |
| Policy/approval     | Canonical encoding cross-language golden tests, digest mutation, expiry, policy/version/HEAD drift, unknown fields, size limits, prompt-injection proposals                                    | `crates/vea-policy/tests`, `tests/integration/policy`                                  |
| Filesystem/Git      | `../`, absolute/device/ADS, symlink chains, junctions, Unicode/case, TOCTOU, `.git` indirection, hostile filenames, filters/hooks/submodules, concurrent lease, dirty/HEAD drift, safe cleanup | `crates/vea-fs/tests`, `crates/vea-git/tests`, platform fixture repos                  |
| Process/provider    | No-shell argv preservation, hostile PATH/env, absolute executable pinning, timeout/output/process-tree limits, TLS destination binding, redirect, auth stripping                               | `crates/vea-process/tests`, `crates/vea-provider-transport/tests`                      |
| Adapter conformance | Probe/handshake, capability claims, recorded stream fixtures, partial JSONL/SSE, stderr noise, unknown events, approval correlation, cancellation, rate limit, usage, resume/version mismatch  | `tests/contract/adapters`                                                              |
| Skills/MCP/plugins  | Bounded scans, symlink escape, malformed YAML, activation dedupe; protocol-era fixtures, schema DoS limits, SSRF/redirect/DNS; manifest/digest/capability denial                               | package tests and `tests/fixtures/malicious-inputs`                                    |
| Secret/privacy      | Canary scan of DB, logs, IPC, renderer snapshots, telemetry/crash fixtures, argv/env, artifacts; OAuth PKCE/state/nonce/replay/redirect                                                        | `crates/vea-secrets/tests`, `tests/integration/policy`                                 |
| UI                  | Reducer event ordering, project isolation, graph states, route rationale, exact approvals, diff/usage, keyboard and accessibility checks using mock bridge                                     | `apps/desktop/src/**/*.test.tsx`, `tests/e2e/mock-desktop`                             |
| Native distribution | Fresh install, sidecar integrity/start/kill/restart, keychain, OAuth loopback, worktree lifecycle, cancel, upgrade/rollback on signed target artifacts                                         | `tests/e2e/platform-smoke`, release CI                                                 |

Browser-mode UI tests use a mock bridge; they do not claim to exercise Tauri privileges. Native smoke tests run against packaged Tauri v2 applications on each supported OS using the platform automation facility that is validated in the bootstrap spike. Provider CI uses recorded, redacted fixtures by default; opt-in live tests run only in protected jobs with spending caps and dedicated accounts.

Minimum adversarial scenarios mirror the security review: indirect secret-exfiltration instructions from files/model/MCP/tool descriptions/commit messages; path and argv attacks; approval mutation; crash at authorize/start/finish; canary-secret scans; integration identity/schema/endpoint changes; and simultaneous mutating leases plus external target-HEAD drift.

## Milestones

| Milestone                                                          | Deliverable                                                                                                                                                                  | Exit gate                                                                                                                                      |
| ------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| **M0 — Foundation and security contracts (weeks 1–2)**             | Tasks 1 and protocol/security portions of 4 and 6; monorepo, Tauri v2 shell, schema generation, sidecar packaging spike, capability/action schemas, provider matrix template | CI builds all target shells; renderer capability audit passes; packaged sidecar handshake works; action and trust-boundary review approved     |
| **M1 — Local-first multi-project shell (weeks 3–5)**               | Tasks 2, 3, and project-trust subset of 5; SQLite/events, project rail, graph editor/query, audit UI                                                                         | Two projects persist/reopen; graph validation works; restart tests pass; no secrets or general privileges exist                                |
| **M2 — Secure worktrees, DAG scheduling, and routing (weeks 6–9)** | Remainder of 4–8; patch workflow, leases, scheduler, quota/budget routing, route inspector                                                                                   | Parallel safe attempts across projects; conflict/fairness/property tests pass; exact approval and adversarial filesystem/Git suite pass        |
| **M3 — First useful agent MVP (weeks 10–13)**                      | Tasks 9–10 plus limited 15; keychain/OAuth broker, Pi-owned loop, two reviewed direct APIs, normalized sessions/diffs/validation/cancel                                      | All 15 MVP criteria pass except any explicitly post-MVP extension/provider breadth; signed internal builds and canary scan pass                |
| **M4 — Subscription/native runtime breadth (weeks 14–17)**         | Task 11, enabled one adapter at a time: Claude Agent SDK, Codex SDK/app-server, Gemini structured CLI, Copilot SDK subject to terms/review                                   | Each adapter independently passes provider matrix, version pin, capability disclosure, conformance, crash, and no-credential-import gates      |
| **M5 — Portable instructions and reviewed MCP (weeks 18–21)**      | Task 12 and bundled-reviewed subset of 13; Skills, disabled-by-default reviewed MCP, canonical/lowered schema diagnostics                                                    | Skills security tests pass; MCP identity/digest/capability consent and adversarial metadata tests pass; no arbitrary server install yet        |
| **M6 — Ecosystem and production release (post-MVP)**               | Full task 13, task 14, remainder of 15; remote MCP, restricted plugin SDK, optional privileged Pi compatibility, signed public releases                                      | Separate threat review for each capability; per-OS confinement; OAuth/SSRF suite; marketplace/update permission UX; release rollback rehearsal |

M4 adapters can proceed in parallel after M3's common conformance suite but are enabled independently. M5/M6 never block the secure coding-agent MVP and do not weaken its boundary to claim ecosystem breadth.

## Implementation status

The repository now contains the M0 scaffold: Tauri/React desktop UI, Rust host metadata command, TypeScript agent-service handshake, domain/configuration/protocol/scheduler/routing/extension contracts, schemas, tests, and CI. SQLite, privileged brokers, worktree execution, credentials, real providers, plugin execution, and MCP execution remain unimplemented.

Generated protocol files and lockfiles must be updated only by declared generation/install commands and checked for reproducibility.

## Dependencies

- Task 1 precedes all implementation work.
- Task 2 precedes persisted projects, graphs, scheduling, routing, recovery, and audit.
- Tasks 4–6 are security/runtime foundations and must complete before any provider agent can execute a tool or mutate a worktree.
- Task 7 depends on Tasks 2 and 5; Task 8 depends on Tasks 2 and 7.
- Task 9 must precede direct API credentials and streaming in Task 10.
- Task 10 provides the common runtime/event/tool path and conformance suite required by Task 11.
- Tasks 12–14 depend on project trust, central policy, process supervision, and audit. MCP and plugin phases also require separate security review gates.
- Task 15 starts in Task 1 as continuous CI/supply-chain work and becomes a release blocker after feature completion.

## Risks

- **Tauri sidecar packaging:** Pi compatibility needs a Node-capable runtime, while Tauri v2 packages target-specific external binaries. M0 must validate the selected bundled runtime/launcher, code-signing, update, native-module, size, and license behavior on all target OSes before agent features depend on it.
- **Provider churn and terms:** SDK/CLI/app-server protocols, auth methods, model names, redistribution, and account automation rules can change. Pin versions, keep adapters independently releasable/disableable, and require dated provider-matrix review.
- **Subscription quota opacity:** most subscriptions expose no stable remaining-quota API. Vea must show unknown/observed confidence, avoid false precision, and react to official limit events rather than claiming predictive control.
- **OS security variance:** path semantics, junctions, process-tree termination, keychains, sandboxing, and updater behavior differ. Broad command/plugin execution must remain unavailable on a platform until its controls pass there.
- **Git checkout behavior:** hooks are not the only execution mechanism; filters and repository/global configuration are hazardous. MVP must reject unsupported active checkout-time executable configuration rather than claiming hardening it has not verified.
- **SQLite/event growth:** streamed deltas and content can create write amplification. Persist bounded/coalesced message events, checkpoint materialized state, and benchmark multi-project recovery without weakening the append-only audit model.
- **Approval fatigue:** exact confirmations can become habitual. Keep action batches bounded and homogeneous, distinguish data destination and reversibility clearly, and test comprehension; do not solve fatigue with blanket approvals.
- **Extension compatibility versus containment:** full Pi packages have system-level power and cannot be honestly presented as restricted plugins. Preserve separate trust tiers and accept reduced compatibility in the safe default.
- **Adapter abstraction loss:** native agents have different resume, approvals, tools, MCP, and sandbox semantics. Capability documents, namespaced raw events, and explicit unsupported errors are required; UI must not promise parity.
- **Residual host risk:** worktrees, signatures, and redaction do not stop same-user malware or a harmful action knowingly approved by the user. Product language must retain these limitations.

## Historical planning provenance

The following record describes the original plan-only task and is retained for provenance; its repository-state statements are not current.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Created only the requested implementation-ready architecture and phased roadmap at /tmp/vea-architecture-plan.md. It covers the concrete monorepo, domain model, adapter interfaces, configuration, scheduler/router, Tauri v2 IPC/process topology, explicit security boundary, MVP criteria, tests, dependencies, risks, and milestones without modifying project files."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "The artifact incorporates all three required research inputs and supplies exact planned paths, state machines, contracts, algorithms, release gates, measurable acceptance criteria, and an independent validation matrix."
    }
  ],
  "changedFiles": [
    "/tmp/vea-architecture-plan.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "Read /tmp/vea-extension-research.md, /tmp/vea-landscape-research.md, and /tmp/vea-security-review.md with the file tools",
      "result": "passed",
      "summary": "All required research and security inputs were read before architecture planning."
    },
    {
      "command": "List /Users/sousavf/Documents/vea-ai with the file tools",
      "result": "passed",
      "summary": "Confirmed the repository has no application source to modify; only .git and pre-existing .pi-subagents entries were listed."
    },
    {
      "command": "Write /tmp/vea-architecture-plan.md",
      "result": "passed",
      "summary": "Wrote the plan to the authoritative external output path."
    }
  ],
  "validationOutput": [
    "The plan explicitly distinguishes direct model providers from native agent runtimes and Skills, MCP, restricted plugins, and privileged Pi compatibility as separate extension planes.",
    "The Tauri v2 design uses narrow commands/channels, Rust-owned privileges, and a fixed external sidecar; it does not grant renderer shell/filesystem access or assume Electron APIs.",
    "The safe MVP exclusions and adversarial tests from the security review are represented as release gates.",
    "No project file was written or staged; the sole write target is /tmp/vea-architecture-plan.md."
  ],
  "residualRisks": [
    "Target-specific Node-capable sidecar packaging and signing must be proven during M0 on macOS, Windows, and Linux.",
    "Provider terms, OAuth registration, redistribution, and CLI protocol versions require provider-specific review before an adapter is enabled.",
    "Subscription quota remains unknown where providers expose no documented telemetry.",
    "Per-OS filesystem, process, keychain, and sandbox behavior requires native validation before privileged scope expands."
  ],
  "noStagedFiles": true,
  "diffSummary": "Added one planning artifact under /tmp; no repository implementation diff was created.",
  "reviewFindings": [
    "no blockers in the architecture plan",
    "review gate: M0 must resolve and validate target-specific sidecar packaging before provider/runtime implementation",
    "review gate: every provider, MCP, plugin, or broader command capability requires its documented terms/security gate before release"
  ],
  "manualNotes": "No tests were added or run because this was a plan-only task against an empty repository. The no-staged-files statement is based on no repository writes/staging in this run and the supplied repository security review; no shell tool was available for an additional git status check."
}
```
