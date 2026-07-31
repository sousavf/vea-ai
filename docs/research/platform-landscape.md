# Research: Open-source agent orchestration and desktop coding-agent landscape

## Summary

The market is splitting into two layers: (1) desktop control planes such as Mux, 1Code, Emdash, and goose that multiplex coding agents and isolate work with Git worktrees, and (2) programmable runtimes such as OpenHands and LangGraph that provide routing, delegation, persistence, and graph execution. The clearest opening for a Pi-like multi-project system is to combine the desktop products’ workspace UX with an explicit task DAG and a quota-aware scheduler: current products expose model choice, subscription login, token/cost metrics, or parallel runs, but primary sources do not show a mature closed loop that routes work by task effort **and** remaining subscription quota.

## Findings

1. **Desktop competitors converge on a project → workspace/chat → isolated worktree model.** Mux (AGPL-3.0) is a desktop/browser “coding agent multiplexer” with local worktree and SSH runtimes, a central Git-divergence view, multi-model support, and status/cost UI. 1Code (Apache-2.0) runs Claude Code and Codex, gives each chat its own worktree, and layers a Kanban, Git client, background sandboxes, model selector, and sub-agent display on top. Emdash (Apache-2.0) is the cleanest agent-agnostic variant: it detects installed provider CLIs, creates one branch/worktree per task, supports remote hosts over SSH/SFTP, and keeps app state in local SQLite. Transferable pattern: make `Project`, `Workspace`, `AgentRun`, `Branch`, and `Artifact/Diff` separate durable entities; treat a worktree as an execution lease, not as the task itself. [Mux](https://github.com/coder/mux) · [Mux worktree runtime](https://mux.coder.com/runtime/worktree) · [1Code](https://github.com/21st-dev/1code) · [Emdash](https://github.com/generalaction/emdash)

2. **Agent adapters are becoming a protocol boundary rather than embedded vendor logic.** goose (Apache-2.0) ships desktop, CLI, and API surfaces and supports ACP providers that wrap Claude Code, Codex, Amp, and Pi while passing goose extensions through as MCP servers. Emdash instead detects locally installed CLIs; 1Code bundles/downloads Claude and Codex binaries; Mux uses its own agent loop. Transferable pattern: define a harness-neutral run contract (start/resume/cancel/events/usage/permissions/artifacts), then implement ACP, CLI-process, and native-loop adapters. This keeps subscriptions and vendor authentication at the edge while the scheduler and UI remain provider-neutral. [goose repository](https://github.com/block/goose) · [goose ACP providers](https://goose-docs.ai/docs/guides/acp-providers) · [Emdash providers](https://emdash.sh/docs/providers)

3. **Parallelism is well represented, but mostly as flat fan-out rather than a user-visible dependency graph.** Mux’s “Best of N” launches sibling agents on the same prompt and synthesizes the strongest result; it distinguishes retries from focused variants. goose can run subagents sequentially or in parallel, shows their tool calls, bounds them by turns/timeouts, and supports reusable subrecipes. OpenHands’ TaskToolSet persists resumable specialist conversations but is explicitly synchronous/sequential. These are useful primitives, yet none of these desktop sources documents a first-class task DAG with dependency edges, readiness, critical path, blocked states, and merge gates. [Mux Best of N](https://mux.coder.com/agents/best-of-n) · [goose subagents](https://goose-docs.ai/docs/guides/context-engineering/subagents/) · [OpenHands TaskToolSet](https://docs.openhands.dev/sdk/guides/task-tool-set)

4. **LangGraph supplies the execution semantics a coding control plane is missing.** Its official workflow guide separates predetermined workflows from dynamic agents and documents sequential chains, conditional routing, parallel nodes, orchestrator-worker fan-out through `Send`, evaluator-optimizer loops, persistence, streaming, and human interrupts. Transferable pattern: store a versioned task graph whose nodes declare inputs, workspace/write scope, preferred harness/model/effort, validation, retries, and budget; schedule only ready nodes; use reducers for fan-in; represent review/approval as interruptible nodes. Keep Git/worktree and quota policy above the generic graph runtime. [LangGraph workflows and agents](https://docs.langchain.com/oss/python/langgraph/workflows-agents) · [LangGraph repository](https://github.com/langchain-ai/langgraph)

5. **Model/effort routing exists as separate mechanisms that can be unified.** Mux supports per-workspace and one-shot model overrides plus model-relative thinking levels (`off` through `max` / numeric levels), which is a strong UI abstraction for heterogeneous providers. Aider’s architect mode deliberately assigns planning to a main model and edits to a separate editor model. OpenHands provides a pluggable `Router` (currently a rule-based multimodal example), named LLM usage IDs, and per-model/conversation cost, token, reasoning-token, context, and latency metrics. Transferable pattern: normalize user intent to an effort class, then resolve `(role, effort, capabilities, remaining budget/quota)` into a harness/model configuration; record the routing reason and permit per-task overrides. [Mux models and thinking levels](https://mux.coder.com/config/models) · [Aider architect/editor mode](https://aider.chat/docs/usage/modes.html#architect-mode-and-the-editor-model) · [OpenHands model routing](https://docs.openhands.dev/sdk/guides/llm-routing) · [OpenHands metrics](https://docs.openhands.dev/sdk/guides/metrics)

6. **Subscription support is emerging, but quota awareness remains the strongest differentiation opportunity.** goose’s ACP adapters reuse existing Claude Code or ChatGPT Plus/Pro authentication and explicitly warn that sessions fail when subscription limits are exceeded. OpenHands supports OAuth login, cached credentials, and refresh for ChatGPT Plus/Pro Codex access; its metrics cover API token/cost usage. Anthropic documents live context indicators, `/cost` for API billing, model switching, and the built-in Opus-plan/Sonnet-execute pattern, but usage metering depends on sign-in type. The sources show authentication, model selection, and spend telemetry—not a portable API for remaining subscription allowance. Differentiate with an `Entitlement/QuotaSnapshot` layer that distinguishes hard provider limits, rolling windows, API budgets, and unknown estimates; reserve quota before fan-out; degrade effort/model or defer low-priority DAG nodes; never silently treat token cost as subscription quota. [goose ACP providers](https://goose-docs.ai/docs/guides/acp-providers) · [OpenHands subscriptions](https://docs.openhands.dev/sdk/guides/llm-subscriptions) · [Anthropic usage and limits](https://support.claude.com/en/articles/14552983-models-usage-and-limits-in-claude-code)

7. **Recipes point toward portable task specifications, but need stronger DAG and workspace semantics.** goose recipes package instructions, parameters, tools/extensions, provider/model, max turns, structured JSON output, shell validation/retry, and subrecipes; repeated subrecipes may be forced sequential, otherwise parallel. This is a useful declarative substrate. For a Pi-like system, extend the idea with stable task IDs, explicit `dependsOn`, declared read/write sets, worktree strategy, acceptance checks, quota ceiling, effort policy, and output/artifact schemas. Avoid putting scheduling truth in prompts. [goose recipe reference](https://goose-docs.ai/docs/guides/recipes/recipe-reference/)

## Competitor/pattern brief

| Project                                                          | Primary strength                                                               | Transferable pattern                                                                                         | Gap / differentiation opening                                                                                               |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------- |
| [Mux](https://github.com/coder/mux)                              | Polished parallel-agent desktop/browser UX; worktree/SSH runtimes; custom loop | Workspace status, Git divergence, costs, Best-of-N, model-relative effort control                            | No documented quota-aware scheduler or first-class cross-project DAG                                                        |
| [1Code](https://github.com/21st-dev/1code)                       | Broad all-in-one desktop/web surface                                           | Chat-per-worktree, Kanban, Git/PR workflow, local/cloud execution, harness switching                         | Kanban sessions are not documented as dependency-aware tasks; some automation/background features are subscription services |
| [Emdash](https://github.com/generalaction/emdash)                | Local-first, agent-agnostic parallel desktop                                   | Installed-CLI detection, task-per-worktree, SQLite state, remote execution                                   | Primarily a parallel workspace manager; no documented model/effort or quota router                                          |
| [goose](https://github.com/block/goose)                          | Native cross-platform agent plus reusable orchestration                        | ACP/MCP boundary, recipes, parallel subagents, bounded turns, validation/retry                               | Recipes/subrecipes are not a general visible DAG; subscription exhaustion is handled as an error condition                  |
| [OpenHands SDK](https://github.com/OpenHands/software-agent-sdk) | Modular production agent runtime                                               | Router interface, subscription OAuth, metrics registry, resumable specialist agents, sandbox/server boundary | Built-in routing is early; TaskToolSet is synchronous rather than a parallel DAG scheduler                                  |
| [LangGraph](https://github.com/langchain-ai/langgraph)           | Stateful graph execution semantics                                             | Conditional/parallel graph, dynamic workers, persistence, interrupts, fan-in                                 | Generic framework: no Git worktree lifecycle, coding-harness adapter, or subscription entitlement model                     |
| [CrewAI](https://github.com/crewAIInc/crewAI)                    | High-level role-based crews plus event-driven flows                            | Separate autonomous collaboration (“Crews”) from deterministic control (“Flows”)                             | Generic automation rather than a multi-project coding desktop; less direct worktree/quota relevance                         |
| [Aider](https://github.com/Aider-AI/aider)                       | Focused terminal pair programmer                                               | Architect/editor model split; explicit plan-vs-edit roles                                                    | Single-session tool, not a multi-project orchestration control plane                                                        |

## Recommended architecture pattern

- **Control plane:** durable `Project → TaskGraph → Task → RunAttempt` model; event log plus materialized state for UI responsiveness.
- **Execution plane:** ephemeral local worktree, SSH directory, or sandbox per write-capable attempt; explicit setup/cleanup and branch ownership.
- **Harness plane:** ACP-first adapter contract with CLI and native-loop fallbacks; normalize events, permissions, resume IDs, usage, and artifacts.
- **Policy plane:** route by task role/capabilities/effort plus quota and budget snapshots; support plan/execute model splitting and recorded overrides.
- **Scheduler:** DAG readiness + per-project write-conflict checks + global/provider concurrency + quota reservations; fan-out/fan-in and reviewer gates are graph primitives.
- **Observability:** per-attempt tokens/cost/latency/context, provider limit events, worktree/commit lineage, acceptance checks, and routing rationale.

## Sources

- Kept: [Mux repository](https://github.com/coder/mux), [worktree runtime](https://mux.coder.com/runtime/worktree), [Best of N](https://mux.coder.com/agents/best-of-n), and [models](https://mux.coder.com/config/models) — primary product/docs evidence for parallel workspace UX, fan-out, and effort controls.
- Kept: [1Code repository](https://github.com/21st-dev/1code) — primary README and source tree for worktree-per-chat, Kanban, harness, and license claims.
- Kept: [Emdash repository](https://github.com/generalaction/emdash) — primary README/source for local-first CLI multiplexing, worktrees, SQLite, and SSH.
- Kept: [goose repository](https://github.com/block/goose), [ACP providers](https://goose-docs.ai/docs/guides/acp-providers), [subagents](https://goose-docs.ai/docs/guides/context-engineering/subagents/), and [recipe reference](https://goose-docs.ai/docs/guides/recipes/recipe-reference/) — primary evidence for desktop/API surfaces, subscriptions, delegation, and declarative workflows.
- Kept: [OpenHands routing](https://docs.openhands.dev/sdk/guides/llm-routing), [subscriptions](https://docs.openhands.dev/sdk/guides/llm-subscriptions), [metrics](https://docs.openhands.dev/sdk/guides/metrics), and [TaskToolSet](https://docs.openhands.dev/sdk/guides/task-tool-set) — primary SDK evidence for routing, OAuth, telemetry, and resumable subagents.
- Kept: [LangGraph workflows](https://docs.langchain.com/oss/python/langgraph/workflows-agents) and [repository](https://github.com/langchain-ai/langgraph) — primary graph-runtime semantics.
- Kept: [Aider modes](https://aider.chat/docs/usage/modes.html) and [Anthropic usage guidance](https://support.claude.com/en/articles/14552983-models-usage-and-limits-in-claude-code) — primary evidence for split-model roles and provider-side usage behavior.
- Dropped: Superset — relevant worktree UI, but its repository uses Elastic License 2.0 (source-available rather than conventional OSI open source), and equivalent architectural evidence was available from AGPL/Apache projects.
- Dropped: AutoGen — official repository now marks it maintenance mode and directs new users to Microsoft Agent Framework; not a strong current foundation recommendation.
- Dropped: SEO comparison pages and unofficial “best agent UI” lists — excluded in favor of project repositories and official documentation.

## Gaps

- Subscription providers generally do not expose a stable, documented “remaining quota” API; precise proactive quota routing may require best-effort parsing of harness status/events and must represent uncertainty.
- Product claims were checked against current official repositories/docs, but no hands-on benchmark compared worktree startup, parallel-run reliability, merge-conflict behavior, or crash recovery.
- License and feature surfaces can change quickly; verify pinned commits/releases before adopting code or protocols.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Produced only the requested competitor/pattern research brief at /tmp/vea-landscape-research.md; no project files were modified."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Brief includes direct findings, a competitor matrix, transferable architecture, explicit gaps, and inline URLs to primary repositories and official documentation."
    }
  ],
  "changedFiles": [
    "/tmp/vea-landscape-research.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "web_search with four focused query angles (orchestration, desktop worktrees, task DAGs, subscriptions/quotas)",
      "result": "passed",
      "summary": "Located primary repositories and official documentation; excluded unofficial and license-mismatched results."
    },
    {
      "command": "fetch_content/get_search_content for Mux, 1Code, Emdash, goose, OpenHands, LangGraph, Aider, CrewAI, AutoGen, Anthropic, and OpenAI sources",
      "result": "passed",
      "summary": "Reviewed full primary-source content for the claims retained in the brief."
    }
  ],
  "validationOutput": [
    "Output written to the authoritative /tmp path.",
    "All retained competitor and pattern claims include primary-source URLs.",
    "No project-source edits or tests were required for this research-only task."
  ],
  "residualRisks": [
    "No hands-on runtime benchmark was performed.",
    "Provider subscription quota APIs remain undocumented or unavailable, so quota-aware routing is partly a product opportunity rather than an immediately portable integration."
  ],
  "noStagedFiles": true,
  "diffSummary": "Added one research artifact under /tmp; project working tree intentionally unchanged.",
  "reviewFindings": [
    "no blockers"
  ],
  "manualNotes": "Review gate remains for the parent/reviewer; source-available Superset and maintenance-mode AutoGen were intentionally not recommended as core open-source foundations."
}
```
