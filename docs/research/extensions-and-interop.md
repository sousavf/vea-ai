# Research: 2026 extensibility and interoperability for a desktop-first open-source coding-agent orchestrator

## Summary

Use a **layered extension architecture**, not one universal plugin API: MCP for portable tools/resources/prompts, Agent Skills for portable instructions, a Pi-compatible TypeScript extension surface for deep in-process lifecycle customization, and explicit adapters for provider agent SDKs/CLIs. The desktop host must own trust, credentials, consent, capability negotiation, event normalization, and audit; neither MCP metadata, a skill's `allowed-tools`, nor a package manifest is a security boundary.

The strongest practical base is Pi's SDK/RPC model plus a host-owned compatibility layer: embed Pi in a dedicated Node service when possible, retain RPC/subprocess isolation as a fallback, and treat Claude Agent SDK, Codex SDK/app-server, Gemini CLI, and Copilot SDK as separate agent runtimes whose native features cannot be losslessly normalized.

## Findings

1. **MCP is the portable capability plane, but 2026 introduces a major protocol-era boundary.** MCP defines JSON-RPC interactions among host, 1:1 client/server connectors, and servers exposing tools, resources, and prompts. The `2026-07-28` line is stateless and carries protocol version and capabilities per request; versions `2025-11-25` and earlier use an `initialize` handshake. A desktop client that wants ecosystem coverage therefore needs a **dual-era MCP client**, probing `server/discover` for modern stdio servers and falling back to legacy initialization as specified. Optional extensions must be explicitly negotiated and must have a core fallback. [MCP specification](https://modelcontextprotocol.io/specification/2026-07-28) · [Architecture](https://modelcontextprotocol.io/specification/2026-07-28/architecture) · [Versioning and compatibility](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning)

2. **Support only the standard MCP transports in the core.** Current standard bindings are newline-delimited JSON-RPC over a client-launched stdio subprocess and Streamable HTTP using POST plus a request-scoped JSON/SSE response. Custom transports are legal but reduce interoperability; old HTTP+SSE belongs behind a legacy adapter. Prefer stdio for local servers because it limits reachability to the launching client, and Streamable HTTP plus the MCP OAuth profile for remote servers. MCP explicitly says HTTP auth should follow its authorization profile while stdio should obtain explicitly configured credentials from the environment. [Transport overview](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports) · [Authorization](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization)

3. **MCP schemas are portable only within a negotiated subset.** The 2026 spec requires JSON Schema 2020-12 by default, graceful handling of unsupported dialects, no automatic network dereferencing of `$ref`, and resource bounds against schema-validation denial of service. In practice, Gemini CLI strips `$schema` and `additionalProperties`, rewrites some `anyOf` defaults, sanitizes names, and truncates names for Gemini API compatibility. This is direct evidence that “valid MCP” does not mean “accepted unchanged by every model API.” Maintain both the **canonical MCP schema** and a per-provider transformed schema, with diagnostics whenever semantics are weakened. [MCP base protocol/schema rules](https://modelcontextprotocol.io/specification/2026-07-28/basic) · [Gemini CLI MCP implementation](https://github.com/google-gemini/gemini-cli/blob/main/docs/tools/mcp-server.md)

4. **The MCP host, not the server, is the policy boundary.** The spec assigns the host connection permissions, lifecycle, consent, context aggregation, and isolation between servers. Tool descriptions and annotations are untrusted. Current security guidance requires showing the exact command before installing/running a local server, recommends sandboxing and least privilege, warns against passing the ambient environment, and covers OAuth SSRF, malicious authorization URLs, token audience confusion, and localhost/proxy escalation. A desktop app should launch each local MCP server with a minimal environment, scoped filesystem/network access, output/time limits, and per-tool confirmation policy. [MCP architecture](https://modelcontextprotocol.io/specification/2026-07-28/architecture) · [Security best practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices)

5. **Agent Skills is the correct portable instruction format.** The format is deliberately small: a directory containing `SKILL.md` with YAML frontmatter (`name` and `description` required), Markdown instructions, and optional `scripts/`, `references/`, and `assets/`. `allowed-tools` is explicitly experimental and varies by client, so it must never grant authority. The standard's progressive-disclosure model loads name/description at startup, full instructions on activation, and supporting resources only on demand. [Agent Skills specification](https://agentskills.io/specification)

6. **Implement the cross-client `.agents/skills/` convention and lenient import, strict export.** The implementation guide recommends project and user client-specific directories plus `.agents/skills/`, deterministic project-over-user precedence, bounded scanning, project trust checks, diagnostics, and lenient handling of cosmetic violations. It also recommends protecting activated skill content from compaction and deduplicating activation. Pi already scans `.agents/skills/`, can consume Claude Code and Codex skill directories, warns on most violations rather than rejecting them, and exposes explicit `/skill:name` activation. Recommendation: accept common deviations with warnings, but create/export spec-conforming skills whose directory matches `name`. [Agent Skills integration guide](https://agentskills.io/client-implementation/adding-skills-support) · [Pi skills docs](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/skills.md)

7. **Treat skills as untrusted prompt/code bundles, not passive documentation.** Skills can inject instructions and include executable scripts. Require project trust before discovery, surface provenance and content hash, distinguish bundled/user/project/organization scopes, and ask for approval when a skill first crosses a capability boundary. A dedicated `activate_skill` tool is preferable to raw arbitrary reads because it can enforce enablement, preserve content during compaction, list resources without eager loading, and audit activation. Do not automatically execute scripts merely because a skill references them. [Agent Skills integration guide](https://agentskills.io/client-implementation/adding-skills-support) · [Pi skills security note](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/skills.md)

8. **Pi extensions are an effective deep plugin surface, but inherently privileged.** Pi TypeScript extensions can register tools, commands, flags, providers and UI; intercept prompt, model, message, tool, compaction, session and provider lifecycles; and contribute skill/prompt/theme paths. Pi packages distribute these resources through npm, git, or local paths. Pi explicitly warns that extensions/packages run with full system access, while project-local extensions load only after project trust. This is suitable as a compatibility target, but a desktop product should execute third-party extensions in a dedicated agent service or worker, never the Electron/Tauri renderer, and should expose a versioned facade rather than arbitrary desktop internals. [Pi extension API](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/extensions.md) · [Pi packages](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/packages.md)

9. **Pi offers both an in-process SDK and language-neutral RPC, which maps well to desktop architecture.** The SDK exposes sessions, streaming events, provider/model runtime, tools, extensions, skills, compaction, queues, and session trees. RPC uses strict LF-delimited JSON over stdio, supports correlated responses plus asynchronous lifecycle/tool/message events, and bridges extension dialogs to a client UI; some TUI-only extension features intentionally degrade or no-op in RPC. Use the SDK inside a dedicated Node agent-service process for type safety and full extension support, and use RPC for crash containment, non-Node hosts, or untrusted/provider-native runtimes. [Pi SDK](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/sdk.md) · [Pi RPC](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/rpc.md)

10. **Separate model-provider adapters from provider-agent adapters.** Pi's provider registration can add/override native provider implementations, model catalogs, auth, OAuth, custom streaming, and OpenAI-/Anthropic-style wire APIs. This is the right path when the orchestrator owns the agent loop and wants consistent tools, sessions, policies, and compaction. By contrast, Claude Agent SDK, Codex SDK/app-server, Gemini CLI, and Copilot SDK own their own loops, sessions and permission semantics; wrap them behind a separate `AgentRuntimeAdapter`, not a model API interface. [Pi provider registration](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/extensions.md#piregisterprovidername-config) · [Claude Agent SDK overview](https://platform.claude.com/docs/en/agent-sdk/overview)

11. **Provider agent surfaces converge on subprocess/event streams but not on one protocol.** OpenAI's TypeScript Codex SDK itself wraps the Codex CLI and exchanges JSONL; it offers thread run/resume, structured output and streamed events. The richer Codex app-server is a separate, version-specific JSON-RPC-like protocol whose TypeScript/JSON schema can be generated from the installed CLI version. Gemini CLI headless mode emits JSONL event types (`init`, `message`, `tool_use`, `tool_result`, `error`, `result`). Copilot SDK talks JSON-RPC to Copilot CLI and negotiates protocol v2-v3, with documented SDK/CLI feature gaps. These should be pinned, handshaken, supervised, and conformance-tested independently. [Codex SDK](https://github.com/openai/codex/blob/main/sdk/typescript/README.md) · [Codex app-server](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md) · [Gemini headless mode](https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/headless.md) · [Copilot SDK compatibility](https://docs.github.com/en/copilot/how-tos/copilot-sdk/troubleshooting/compatibility)

12. **Native SDKs preserve capabilities that a common abstraction will otherwise erase.** Claude Agent SDK exposes the Claude Code loop, tools, hooks, subagents, MCP, permissions, sessions, skills and plugins; other languages must use headless CLI subprocess mode. Codex app-server has native approvals, thread fork/steer, skills, MCP calls and version-generated schemas. Copilot exposes hooks, skills, MCP, custom tools, BYOK and session management but documents CLI-only UI/management functions. Therefore the normalized API should expose a stable minimum plus `capabilities`, `nativeConfig`, and namespaced raw events rather than pretending every runtime supports identical semantics. [Claude Agent SDK overview](https://platform.claude.com/docs/en/agent-sdk/overview) · [Codex app-server](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md) · [Copilot SDK compatibility](https://docs.github.com/en/copilot/how-tos/copilot-sdk/troubleshooting/compatibility)

13. **MCP support inside a vendor runtime does not make that runtime adapter MCP.** Gemini extensions commonly bundle MCP servers plus commands, context, skills and hooks. Copilot SDK supports local stdio and remote HTTP/SSE MCP servers with tool allowlists. Those configurations and lifecycle behaviors remain vendor-specific. Keep one host-owned MCP subsystem for portable integrations; only delegate MCP to a provider runtime when a native-only feature requires it, and record which layer owns approval and credentials to avoid double execution or double prompting. [Gemini extension authoring](https://github.com/google-gemini/gemini-cli/blob/main/docs/extensions/writing-extensions.md) · [Copilot SDK MCP](https://docs.github.com/en/copilot/how-tos/copilot-sdk/features/mcp)

## Concrete architectural recommendations

### 1. Four extension planes

| Plane                 | Contract                                             | Runs where                                                 | Intended portability |
| --------------------- | ---------------------------------------------------- | ---------------------------------------------------------- | -------------------- |
| Portable capabilities | MCP tools/resources/prompts                          | Sandboxed local subprocess or authenticated remote service | Cross-client         |
| Portable instructions | Agent Skills `SKILL.md`                              | Parsed by host; scripts only through normal approved tools | Cross-client         |
| Deep host plugins     | Pi-compatible, versioned TypeScript extension facade | Dedicated agent-service/worker process                     | VEA/Pi ecosystem     |
| Provider runtimes     | `AgentRuntimeAdapter` for SDK/CLI/app-server         | Supervised subprocess or isolated service                  | Provider-specific    |

Do not silently translate a Pi extension into MCP or a skill into a plugin: they have different lifecycle, authority, and distribution semantics.

### 2. Desktop process topology

- **Renderer/UI:** no provider keys, shell, plugin imports, or MCP process spawning.
- **Desktop main process:** windowing, secure credential broker, OS keychain, signed IPC and update/install UX.
- **Agent service:** Pi SDK session runtime, normalized event bus, policy engine, skill catalog, MCP clients and provider adapters.
- **One child/process boundary per privileged integration where feasible:** local MCP server, vendor CLI runtime, and untrusted plugin worker. Apply kill-on-parent-exit, bounded queues, cancellation, timeouts and output caps.
- Persist host-owned session metadata separately from provider-native session IDs. Store an adapter/version tuple so resume can fail clearly rather than corrupt state.

### 3. Stable normalized runtime contract

Define a small host contract such as:

- lifecycle: `start`, `resume`, `fork?`, `send`, `steer?`, `cancel`, `dispose`;
- event envelope: `{schemaVersion, runtime, sessionId, sequence, timestamp, kind, correlationId?, payload, raw?}`;
- common event kinds: session state, message delta/final, tool proposed/started/progress/completed, approval requested/resolved, usage, warning/error, run settled;
- capability document: attachments, structured output, steering, fork, MCP ownership, skills, custom tools, reasoning levels, sandbox modes, raw-event namespaces;
- explicit unsupported errors rather than emulation for approval, fork, steering, or session semantics.

Preserve unknown provider events under `raw` and never expose chain-of-thought as a required portable field.

### 4. MCP implementation policy

- Ship a dual-era client and pin/test the exact MCP SDK/spec revisions supported.
- Core transports: stdio + current Streamable HTTP; legacy SSE only in compatibility mode.
- Namespace displayed tools by server identity while retaining original server/tool names on the wire.
- Store canonical and provider-lowered tool schemas; report removals/truncations and reject transformations that alter required semantics.
- Default-deny local server install/run; display exact executable, args, cwd, environment names, filesystem roots and network policy.
- Pass an allowlisted environment only. Keep credentials per server in the keychain/broker.
- Require per-tool consent by risk class, even if the server labels itself read-only; server annotations are advisory.
- Implement OAuth in the main/agent service, not a webview: PKCE, issuer/audience checks, safe OS URL opening, strict redirect handling, SSRF defenses and step-up scopes.
- Treat Skills-over-MCP, MCP Apps and Tasks as negotiated optional modules, not baseline dependencies.

### 5. Skills policy

- Scan `~/.agents/skills`, project `.agents/skills`, and product-specific locations; project overrides user, with deterministic diagnostics.
- Gate all project skill discovery on workspace trust; bound recursion/file counts/sizes and avoid following escaping symlinks.
- Strictly validate on authoring/export, leniently import safe cosmetic deviations, skip missing descriptions or unparseable YAML.
- Preserve activated skill instructions across compaction and deduplicate activation.
- Treat `compatibility` as display/filter metadata and `allowed-tools` as advisory only; host policy always wins.
- Record provenance, license, content hash and activation audit. Never auto-run bundled scripts.

### 6. Plugin/package policy

- Offer a Pi-compatible subset first: tools, commands, lifecycle events, provider registration, resource discovery and host-mediated UI.
- Version the facade independently and expose feature detection. Avoid leaking Electron/Tauri objects or mutable internal session classes.
- Require a manifest declaring extension entrypoints, requested host capabilities, supported facade range and package integrity; npm/git source alone is not trust.
- Pin package versions/commits, review before update, show diffs/permission changes, and support per-resource enable/disable.
- Full Pi compatibility may require full process privilege; label that mode clearly. A safer restricted worker API should be the default for marketplace packages.

### 7. Provider adapter strategy

1. **Preferred normalized path:** Pi-owned loop plus direct provider adapters for Anthropic/OpenAI/Google/OpenAI-compatible endpoints. This maximizes consistent policy, MCP, skills, sessions and UI.
2. **Native-agent path:** official SDK where stable (Claude Agent SDK, Codex SDK, Copilot SDK), with CLI/app-server behind it where the SDK itself uses that boundary.
3. **CLI fallback:** only documented structured JSON/JSONL modes; never scrape ANSI/TUI text.
4. At startup, pin/check executable version, negotiate or generate schemas where available, run a health probe, and publish capabilities.
5. Conformance-test cancellation, partial JSONL frames, stderr noise, approval round trips, tool correlation, crash/restart, resume across versions and unknown events for every adapter.

### 8. Compatibility boundaries to make explicit in product UX

- MCP protocol era and optional extensions;
- JSON Schema dialect/features after provider lowering;
- tool-name character/length limits and collision strategy;
- image/audio/resource content support;
- permission/sandbox semantics (not equivalent across providers);
- reasoning-level names and availability;
- session persistence, resume/fork and compaction ownership;
- SDK-only versus CLI-only features;
- TUI-only Pi extension UI methods when running over RPC;
- authentication terms/licensing: an open-source host does not make commercial provider SDKs/services open or freely redistributable.

## Sources

### Kept

- [MCP 2026-07-28 specification](https://modelcontextprotocol.io/specification/2026-07-28) — normative protocol overview and security principles.
- [MCP architecture](https://modelcontextprotocol.io/specification/2026-07-28/architecture) — host/client/server responsibility and isolation.
- [MCP versioning and compatibility](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning) — modern/legacy boundary and dual-era behavior.
- [MCP transports](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports) — standard transport bindings.
- [MCP authorization](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization) — normative HTTP OAuth behavior.
- [MCP security best practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices) — local execution, OAuth, SSRF and consent threats.
- [Agent Skills specification](https://agentskills.io/specification) — normative portable skill format.
- [Agent Skills implementation guide](https://agentskills.io/client-implementation/adding-skills-support) — cross-client discovery and progressive disclosure guidance.
- [Pi extensions](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/extensions.md) — primary extension/provider/event API documentation.
- [Pi skills](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/skills.md) — primary compatibility and discovery behavior.
- [Pi packages](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/packages.md) — primary package/distribution and trust behavior.
- [Pi SDK](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/sdk.md) and [RPC](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/rpc.md) — primary embedding contracts.
- [Claude Agent SDK overview](https://platform.claude.com/docs/en/agent-sdk/overview) — official capabilities and deployment choices.
- [OpenAI Codex SDK](https://github.com/openai/codex/blob/main/sdk/typescript/README.md) and [app-server](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md) — official subprocess and rich protocol surfaces.
- [Gemini CLI headless](https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/headless.md), [MCP](https://github.com/google-gemini/gemini-cli/blob/main/docs/tools/mcp-server.md), and [extensions](https://github.com/google-gemini/gemini-cli/blob/main/docs/extensions/writing-extensions.md) — official structured mode and compatibility evidence.
- [Copilot SDK MCP](https://docs.github.com/en/copilot/how-tos/copilot-sdk/features/mcp) and [compatibility](https://docs.github.com/en/copilot/how-tos/copilot-sdk/troubleshooting/compatibility) — official protocol/features boundary.

### Dropped

- Search-result summaries and third-party comparison articles — redundant once primary specifications, repositories and vendor docs were available.
- MCP server directories/marketplaces — useful for discovery, but not authoritative evidence for protocol compatibility or safety.
- Agent Skills client showcase — the fetched page did not expose a reliable client list, so no adoption claims rely on it.
- Older localized Anthropic Claude Code SDK pages — superseded by the current official Agent SDK documentation.

## Gaps

- Provider CLIs and app-server protocols move quickly and generally do not promise one cross-vendor compatibility window; exact minimum versions must be selected and verified in implementation CI.
- No primary source establishes that arbitrary Pi extensions can be safely sandboxed without reducing compatibility; restricted plugins and full-compatibility plugins should remain separate trust tiers.
- Licensing/redistribution, OAuth client registration and branding terms require product-specific legal review for each bundled provider runtime.
- Performance and crash-isolation tradeoffs between in-process Pi SDK and subprocess RPC need benchmarks in the target desktop shell and operating systems.

## Acceptance report

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Focused research only on MCP, Agent Skills, Pi extension/package/SDK/RPC surfaces, provider SDK/CLI adapters, compatibility boundaries, and concrete architecture; no project files were modified."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "The brief contains 13 sourced findings, concrete layered architecture and security recommendations, primary-source URLs, explicit dropped sources, gaps, and residual risks sufficient for independent review."
    }
  ],
  "changedFiles": [
    "/tmp/vea-extension-research.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "Focused multi-angle web searches and primary-source fetches via research tools",
      "result": "passed",
      "summary": "Reviewed current MCP, Agent Skills, Pi, Anthropic, OpenAI, Google, and GitHub primary documentation."
    },
    {
      "command": "Write /tmp/vea-extension-research.md",
      "result": "passed",
      "summary": "Research brief written to the authoritative output path."
    }
  ],
  "validationOutput": [
    "Primary-source citations are embedded inline and enumerated under Sources.",
    "No project-file editing tool call was made; the only write target was /tmp/vea-extension-research.md.",
    "No tests were applicable to a research-only artifact."
  ],
  "residualRisks": [
    "Fast-moving provider CLI/app-server protocols require pinned-version conformance tests before implementation.",
    "Provider licensing, redistribution, OAuth registration, and branding terms need product-specific legal review.",
    "Safe sandboxing may intentionally reduce full Pi extension compatibility."
  ],
  "noStagedFiles": true,
  "diffSummary": "Added only the requested temporary research artifact; project files remain unchanged.",
  "reviewFindings": [
    "no blockers",
    "review gate remains required by the parent/reviewer per the acceptance contract"
  ],
  "manualNotes": "Research artifact only. The no-staged-files assertion is based on making no project writes or staging operations in this run; no shell execution facility was used for an independent git status check."
}
```
