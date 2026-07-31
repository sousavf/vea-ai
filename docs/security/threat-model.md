# Security/platform review: threat model and safe MVP boundary

## Current status

This threat model was created before the initial application scaffold and remains the security baseline for all subsequent work. The repository now contains a Tauri/React shell and tested TypeScript contracts, but privileged providers, credentials, filesystem brokers, worktree execution, plugins, and MCP execution are not implemented. The P0 controls and explicit exclusions below remain release gates.

A worktree is a concurrency and change-isolation mechanism, not a security sandbox. Any design that treats it as containment is a blocker.

## 1. Security objective and assumptions

The application is a desktop-first TypeScript orchestrator. It can cross high-impact boundaries: local files, source repositories, credentials, subprocesses, external providers, plugins, and MCP servers. Its central security invariant should be:

> Untrusted content may propose an action, but only trusted policy plus an attributable user decision may authorize it. Data access, network access, and code execution are separate capabilities and never follow implicitly from a model response.

Assumptions for the MVP:

- One interactive user on one desktop OS account; no team tenancy or remote control.
- The host OS, keychain, and logged-in user are trusted. Host administrators, malware running as the same user, kernel compromise, and physical compromise are out of scope and must not be claimed as prevented.
- Provider responses, model output, repository content, web content, skill text, plugin metadata, and all MCP responses are untrusted input.
- Projects are user-selected and trusted enough to open. Worktrees may contain malicious text and symlinks even when the project owner is trusted.
- The product is not a general-purpose endpoint sandbox. OS-specific confinement may reduce impact but does not make arbitrary code safe.

## 2. Assets, actors, and trust boundaries

### Assets

1. Provider API keys, OAuth access/refresh tokens, and existing CLI login state.
2. Project source, unpublished changes, repository history, and adjacent host files.
3. User prompts, provider responses, tool arguments/results, and derived embeddings or caches.
4. User identity, billing quota, rate limits, and provider account standing.
5. Integrity of executable adapters, plugins, skills, MCP definitions, updates, and policy.
6. Audit records and the association between user intent, model request, authorization, and side effect.

### Actors and likely threats

- A malicious instruction embedded in a repository, issue, document, tool result, or web response.
- A compromised or over-privileged plugin/MCP server.
- A malicious dependency or update.
- A model that hallucinates unsafe arguments or leaks context without malicious intent.
- A local user making an ambiguous approval, or another same-account process reading weakly protected data.
- A remote provider compromise or provider behavior/terms change.

### Trust boundaries / data flows

1. **Desktop UI → privileged broker:** renderer requests must cross typed, schema-validated IPC. If the shell is Electron, use `contextIsolation: true`, `sandbox: true`, `nodeIntegration: false`, a narrow preload API, strict CSP, no remote content in the privileged renderer, and origin-check every IPC route.
2. **Broker → credential store:** only opaque credential references cross into ordinary application state. Secret material is retrieved just in time.
3. **Broker → provider:** TLS network egress crosses organizational/data-residency boundaries and may send proprietary content.
4. **Broker → project/worktree:** paths and repository-controlled configuration cross into host filesystem and Git behavior.
5. **Broker → command/plugin/MCP process:** this is a code-execution boundary, not merely a tool call.
6. **Model/tool output → action planner:** this is always an untrusted-data boundary, even for a model selected by the user.
7. **Updater/dependency registry → installed app:** signed release and supply-chain boundary.

## 3. Priority threat register

| ID  | Scenario                                                                        | Impact                                              | Required mitigation                                                                                                                | MVP disposition              |
| --- | ------------------------------------------------------------------------------- | --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | ---------------------------- |
| T1  | Prompt injection asks the agent to reveal secrets or invoke tools               | Credential/source exfiltration, destructive actions | Treat content as data; capability mediation outside the model; destination-aware disclosure confirmation; no secret-reading tool   | P0                           |
| T2  | Secret lands in config, SQLite, logs, crash dump, prompt, argv, env, or Git     | Account compromise                                  | OS keychain; opaque IDs; field-level redaction; minimal child environment; secret scanning tests; revoke flow                      | P0                           |
| T3  | OAuth redirect/session is intercepted or confused                               | Account takeover                                    | System browser, Authorization Code + PKCE, state and nonce, exact loopback redirect, ephemeral port, callback timeout              | P0                           |
| T4  | App scrapes/reuses undocumented browser or CLI credentials                      | Account and terms violation                         | Only documented provider auth and documented CLI integration; never copy token files/cookies                                       | Exclude otherwise            |
| T5  | Plugin or MCP server receives ambient filesystem/network/secrets                | Host compromise/exfiltration                        | Deny by default, separate process, explicit manifest capabilities, scoped broker APIs, no ambient secrets                          | Third-party code excluded    |
| T6  | Shell interpolation or malicious filename changes command meaning               | Arbitrary command execution                         | No shell; executable plus argv arrays; fixed templates; canonical paths; allowlisted environment and executable                    | P0                           |
| T7  | A process escapes a worktree via absolute path, `..`, symlink, or child process | Adjacent-file modification                          | Realpath/canonical containment checks; no arbitrary command; OS confinement; separately authorize external paths                   | P0                           |
| T8  | Git hooks, filters, submodules, or config execute during open/checkout          | Code execution                                      | Trusted repositories only; suppress hooks; do not initialize submodules; do not assume checkout is passive; harden Git config      | P0                           |
| T9  | Concurrent agents modify the same branch/worktree                               | Corruption, confused audit/approval                 | One run/agent per worktree and branch; leases/locks; detect HEAD/dirty-state drift; serialize integration                          | P0                           |
| T10 | Audit logging itself captures credentials/source                                | Secondary breach                                    | Metadata-first event schema, structured redaction before persistence, restricted permissions, retention/delete controls            | P0                           |
| T11 | Remote MCP URL reaches localhost/private networks or follows redirect           | SSRF/local service access                           | MCP disabled by default; explicit endpoint approval; IP/redirect policy; TLS; re-resolve and pin connection target where practical | Exclude arbitrary remote MCP |
| T12 | Malicious or compromised app/plugin update                                      | Persistent code execution                           | Signed builds and updates, pinned lockfile, provenance/SBOM, review and rollback                                                   | P0 before distribution       |
| T13 | Provider receives content the user did not intend to disclose                   | Privacy/IP incident                                 | Per-project provider policy, pre-send scope summary, minimization, provider/data-retention disclosure                              | P0                           |
| T14 | Retried or duplicated action runs twice                                         | Duplicate side effects/cost                         | Stable action IDs, idempotency keys where available, execution state machine, never silently retry writes                          | P0                           |
| T15 | Approval UI presents a benign summary but executes changed arguments            | Authorization bypass                                | Bind approval to canonical action digest, capability, destination, and exact args; invalidate on any mutation                      | P0                           |

## 4. Mandatory control design

### Secrets, OAuth, and CLI sessions

- Store API keys and refresh tokens only in the platform credential store (Keychain, Credential Manager, or Secret Service through a maintained abstraction). Persist `{credentialId, provider, account label, scopes, createdAt}` in app state, never the value.
- Access tokens should be memory-only when feasible. Do not put secrets in URLs, command-line arguments, general environment variables, telemetry, exception messages, prompt transcripts, or renderer state.
- Redaction is defense in depth, not the primary control. Use structured fields and mark secret-bearing values at creation; redact before serialization. Free-text regex redaction cannot be guaranteed complete.
- OAuth uses the provider's registered public desktop client flow, system browser, Authorization Code with PKCE S256, unpredictable `state`, OIDC `nonce` where applicable, exact redirect matching, a loopback listener bound only to `127.0.0.1`/`::1`, short timeout, and one-time callback consumption. Do not embed provider login pages in a webview.
- Request the minimum scopes. Display provider/account/scopes and support disconnect, revocation guidance, and expired-token recovery. Account switching must not accidentally reuse another project's provider identity.
- A CLI session remains owned by the provider CLI. Integrate only by invoking a documented CLI/API contract; do not read, decrypt, import, or copy its credential database, browser profile, cookie store, or token cache. Pass only a minimal environment and do not infer that “CLI installed” means user consent to use its account.
- Never forward one provider's credential to another provider, plugin, model, skill, MCP server, or child command. Tools receive brokered operations or short-lived, audience-restricted credentials only if the provider officially supports them.
- Provide revoke/rotate and secure-delete semantics, while documenting that backups, provider logs, and OS behavior may prevent guaranteed erasure.

### Prompt injection and action authorization

- Separate the **planner** (untrusted model output) from an application-owned **policy enforcement point**. The model cannot grant itself a tool, increase a scope, suppress confirmation, or alter policy.
- Parse tool calls against versioned schemas; reject unknown fields, invalid enums, path escapes, excessive sizes, and ambiguous encodings. Never execute prose or code fences as commands.
- Label content by origin and preserve provenance through summaries. Instructions found in files, web pages, issue bodies, tool/MCP results, commit messages, and generated text have no authority to change system policy.
- Use a capability matrix based on `{project, worktree, tool, operation, resource, destination, provider/account, run}`. Default deny. Authorization is short-lived and cannot be generalized from read to write, local to network, or one host/path to another.
- Show confirmation for every externally visible or destructive action in the MVP: command execution, file writes outside a staged edit set, Git push, network destination changes, sending project content, installing/enabling integrations, and secret use. The dialog shows exact destination, affected paths, canonical argv or structured request, data categories, and reversibility.
- Bind the approved canonical payload to a digest. Re-plan or mutation invalidates approval. Batch approval must enumerate bounded homogeneous actions; do not offer “always allow everything.”
- Limit iteration count, wall time, token/cost budget, bytes read/sent, and tool-result size. Stop on policy denial instead of asking the model to work around it.

### Plugin, skill, and MCP permissions

Treat these concepts distinctly:

- A **skill** is declarative prompt/instruction content and has no capability on its own. It can request only tools already granted to the run. Skill text is untrusted and cannot hide or auto-accept an approval.
- A **plugin** is executable code and therefore equivalent to installing software.
- A **stdio MCP server** is a local executable plugin. A **remote MCP server** is an untrusted network principal whose tool metadata and results may be malicious.

For any post-MVP executable integration:

- Use a versioned manifest with an immutable package digest and separately declared capabilities: project read globs, proposed writes, process execution, network host/port, provider operations, UI contribution, clipboard, and secret aliases. No wildcard capability in normal installation.
- Show permissions before installation and on expansion after update. Pin versions/digests, verify publisher/signature where an ecosystem supports it, allow disable/revoke, and keep a kill switch. A signature proves origin, not safety.
- Run executable integrations out of process under an OS sandbox/profile where supported. Communicate over bounded, authenticated IPC with timeouts, message-size limits, schema validation, and cancellation. Do not load third-party npm modules into the privileged desktop process.
- Provide file content and provider operations through scoped broker APIs rather than ambient filesystem or credential access. Network is separately denied unless declared and approved.
- Tool descriptions, JSON schemas, resource text, and error messages from MCP are untrusted. Namespace tools by server identity; prevent name collision/spoofing. Reconfirm after server identity, endpoint, schema, or capability changes.
- Remote MCP must authenticate the server, use TLS, constrain redirects and resolved addresses, and prevent unintended access to loopback, RFC1918/link-local ranges, cloud metadata, and Unix sockets. If local-network MCP is a desired future feature, make it a separate visibly dangerous permission.

### Command execution and sandboxing

- **MVP rule: no arbitrary shell.** Use a small catalog of broker-owned operations. Spawn an absolute, policy-approved executable directly with an argv array (`shell: false`); never concatenate a command string or invoke `sh -c`, `cmd /c`, or PowerShell script text.
- Resolve and pin the executable path; do not trust a project-modified `PATH`, `.env`, shell aliases, package scripts, or shim. Use a minimal allowlisted environment, dedicated temporary directory, fixed locale where parsing output, closed inherited file descriptors, and no credential-bearing variables.
- Enforce canonical cwd/path containment both when authorizing and immediately before use. Reject NUL, platform device paths, alternate data streams where relevant, `..` escapes, and symlink/junction traversal. Avoid time-of-check/time-of-use by using descriptor-relative operations where the platform permits.
- Apply wall-clock timeout, output/byte limits, process-tree cancellation, and resource limits. Capture stdout/stderr as untrusted bounded data. Do not silently retry side effects.
- Require exact-action confirmation before execution and report exit status. “Read-only” commands can still execute hooks, contact networks, or read secrets; classify by implementation, not command name.
- OS sandboxing is required before broadening the command catalog, but is not sufficient alone. The design needs per-OS profiles denying home-directory access outside the selected project, keychain, device nodes, IPC, and network unless granted. Unsupported platforms must lose the feature rather than run unsandboxed.
- Do not claim containers are a universal desktop security boundary: daemon sockets, bind mounts, rootful runtimes, host networking, and platform VM sharing can defeat it.

### Worktree and Git isolation

- Create a dedicated worktree and unique branch for each mutating run. The original checkout is not a workspace. Hold an app-level lease, record initial repository identity/HEAD/status, and reject unexpected drift.
- Allow only user-selected trusted repositories in MVP. A Git worktree shares object storage and repository metadata and does not prevent a process from modifying the main repository or host.
- Canonicalize repository and worktree roots. Every file broker operation must verify the final resolved target remains under the worktree; verify each existing parent against symlinks/junctions and create new files safely. Treat `.git` files/directories and linked-worktree administration paths as protected.
- Do not automatically initialize/update submodules, run package lifecycle scripts, trust repository-local executables, or open an integrated shell. Suppress Git hooks for brokered operations. Audit and harden exposure to Git clean/smudge/process filters and user/repository config before any checkout of untrusted repositories; merely setting `core.hooksPath` does not disable filters.
- Integration back to the user's branch is a separate, confirmed action showing the commit range/diff and current target HEAD. Do not push automatically. Preserve conflicting worktrees for recovery rather than force-cleaning.
- Cleanup verifies ownership/lease and never uses user-controlled paths for recursive deletion. Record abandoned worktrees and let the user inspect them.

### Audit logs, telemetry, and privacy

Use an append-only structured event stream with restrictive user-only filesystem permissions. Each event should include:

- event ID, monotonic sequence, timestamp, app/policy version;
- run, project, worktree, provider/account alias, plugin/MCP identity and digest;
- initiating actor and content provenance;
- requested capability and canonical action digest;
- policy decision, approval identity/time/scope, execution start/end, result class;
- bounded/redacted argv or request summary, canonical affected paths, and network destination.

Defaults:

- Do not log credential values, authorization headers, OAuth codes, environment dumps, full prompts/responses, source contents, clipboard, or raw tool output. Make content logging a separate opt-in diagnostic mode with warning, expiry, and local-only handling.
- Redact before persistence and telemetry, not only in the log viewer. Test known secret formats plus field-level secrecy. Keep crash reporting off or local by default until its scrubber and consent are validated.
- Define retention, size rotation, export, and deletion. Avoid cross-project identifiers in telemetry. Document provider-side logs separately.
- A local hash chain can reveal accidental modification but cannot provide strong tamper evidence against the same OS user who controls both log and key. Do not market it as a forensic guarantee.

### Provider terms and platform governance

Create a maintained provider matrix before enabling each adapter:

- permitted official API/SDK/CLI and authentication method;
- OAuth client type, scopes, token-storage and branding requirements;
- whether account/session automation, credential brokering, model routing, or resale is permitted;
- rate limits, concurrency, retries, caching, and attribution;
- allowed input types, sensitive-data restrictions, region/residency, retention/training defaults, deletion, and subprocessors;
- output/IP terms, acceptable-use restrictions, safety requirements, and incident/contact process;
- date/version reviewed and an owner for re-review.

Do not scrape consumer web apps, automate browser sessions, share credentials among users, bypass quotas, impersonate an official client, or rely on undocumented endpoints. “Bring your own key” does not remove the product's obligations. The UI must disclose which provider/account receives which project data before first use and when routing changes. Obtain legal/product review for every provider and executable integration; technical controls cannot establish contractual permission.

## 5. Safe MVP boundary

### Allowed

- Single-user, foreground desktop operation with no inbound remote control.
- A small set of bundled, reviewed, version-pinned provider adapters using official documented APIs.
- API keys in the OS credential store and OAuth only where the provider supports a public desktop PKCE flow. Documented user-owned CLI invocation may be enabled per provider without importing its secrets.
- Explicitly selected trusted local Git repositories; one isolated worktree/branch per mutating run.
- Read-only project inspection through brokered APIs, plus bounded patch proposals. Applying a patch, integrating a branch, provider submission, or narrow broker command requires exact confirmation.
- Declarative bundled skills with no independent permissions.
- Local metadata-only audit log, visible run limits, cancellation, and provider/cost/data-destination display.
- If MCP is essential to product validation, only bundled and reviewed servers with pinned identity and a fixed capability manifest; disabled by default and enabled per project. Otherwise omit MCP execution entirely from MVP.

### Explicitly excluded

- Arbitrary third-party plugins, npm packages, downloaded skills with auto-granted tools, and arbitrary stdio MCP commands.
- User-entered shell commands, model-generated shell/script execution, package manager/lifecycle commands, privilege elevation, Docker socket access, and unrestricted executables.
- Arbitrary remote MCP endpoints, local-network discovery, servers that need general home-directory access, and MCP-originated secret requests.
- Untrusted repository checkout, automatic submodules, hooks/filters, automatic push/merge, or concurrent mutation of one worktree.
- Browser-cookie/token scraping, importing CLI credential files, undocumented provider endpoints, shared accounts, quota evasion, and unattended account switching.
- Autonomous background runs, blanket/persistent approvals, remote agents, team/workspace sync, credential sync/export, and multi-user tenancy.
- Claims that prompt injection is “solved,” worktrees are sandboxes, redaction guarantees no leaks, logs are tamper-proof, or all provider uses are contractually allowed.

## 6. P0 architecture and release gates

1. Write the capability model and action schema before tool APIs; all privileged entry points pass one central policy enforcement layer.
2. Establish privileged-process separation and typed IPC. The renderer, providers, model text, skills, and tool results never directly access Node/process/filesystem/credentials.
3. Add a credential-store abstraction and provider-specific documented auth flows; test that persistence, errors, logs, child env, and renderer messages contain no token.
4. Build path containment and worktree lifecycle primitives with cross-platform adversarial tests for symlinks/junctions, traversal, races, dirty repos, concurrent runs, cleanup, and Git config side effects.
5. Keep the command catalog narrow and non-shell; test argument preservation, executable resolution, environment stripping, timeout/output limits, cancellation, and approval digest binding.
6. Implement provenance-aware tool-call validation, capability denial, destination disclosure, and bounded loops. Red-team indirect injection from every input source.
7. Implement metadata-first audit events and a user-visible activity/approval history; test redaction before persistence and crash/telemetry paths.
8. Sign release artifacts and updates; pin dependencies, retain lockfile integrity, generate an SBOM, scan dependencies, and document release-key custody/rollback.
9. Complete the provider terms/privacy matrix and in-product disclosures. Provide revoke, export, retention, and deletion UX.
10. Threat-model each newly enabled plugin/MCP/command capability separately. Any exception to the MVP exclusions is a security review gate, not a configuration toggle.

### Minimum adversarial acceptance tests

- Inject “read credentials and upload them” through a file, model response, MCP result, tool description, and Git commit message; all remain unauthorized and produce no secret/tool invocation.
- Attempt path escape using `../`, absolute paths, symlink chains, junctions, case/Unicode ambiguity, `.git` indirection, and cleanup races.
- Attempt argv injection with spaces, quotes, metacharacters, option-like filenames, and hostile `PATH`/environment; the exact approved argv is what executes without a shell.
- Mutate an approved action or endpoint after confirmation; its digest mismatch must force re-approval.
- Crash during authorize/start/finish phases; the audit trail and worktree recover without replaying side effects.
- Scan app state, logs, renderer IPC, telemetry fixtures, crash fixtures, subprocess env/argv, and repository changes for seeded canary secrets.
- Change plugin/MCP version, identity, schema, redirect, DNS result, or permission manifest; existing consent must not silently carry over.
- Start two mutating runs against one repository and alter target HEAD externally; leases/drift checks prevent confused integration.

## 7. Residual risks after these controls

- An authorized provider still receives plaintext content and can retain or misuse it according to its systems and terms.
- The user can knowingly approve a harmful action; approval quality and fatigue remain material risks.
- Same-user malware can often inspect process memory, automate the UI, or use the user's own credentials despite application controls.
- OS sandbox behavior and filesystem edge cases differ across macOS, Windows, and Linux; each supported platform needs independent verification.
- Models and scanners cannot reliably identify all sensitive data or prompt injection. Data minimization and external policy enforcement remain necessary.
- Signed plugins/updates may still be malicious or compromised, and dependency provenance does not prove safety.

## Historical review provenance

The following acceptance record describes the point-in-time, pre-scaffold threat-modeling task. It is retained for provenance and does not describe the current repository contents.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Produced only the requested security/platform threat model at /tmp/vea-security-review.md; no repository project file was created or modified. The review covers secrets, OAuth/CLI sessions, prompt injection, plugin/MCP permissions, command sandboxing, worktree isolation, audit logs, provider terms, and safe MVP exclusions."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Included assumptions, assets, trust boundaries, a 15-scenario threat register, mandatory controls, explicit allowed/excluded MVP scope, P0 release gates, adversarial acceptance tests, residual risks, and repository-state command evidence."
    }
  ],
  "changedFiles": [
    "/tmp/vea-security-review.md (review artifact outside the repository)"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "git status --short && git diff --cached --name-only && find . -maxdepth 2 -type f -not -path './.git/*' -print | sort",
      "result": "passed",
      "summary": "Found no project files or staged files; only untracked .pi-subagents/ was present."
    },
    {
      "command": "pwd; git rev-parse --is-inside-work-tree; git log --oneline -5; git status --porcelain=v1",
      "result": "passed",
      "summary": "Confirmed /Users/sousavf/Documents/vea-ai is a Git worktree, main has no commits, and .pi-subagents/ is the sole untracked entry; git log itself reported the expected no-commits condition."
    },
    {
      "command": "python3 acceptance-report/topic validation; git status --short; git diff --cached --name-only",
      "result": "passed",
      "summary": "Parsed the fenced acceptance report as JSON, verified every required key and topic heading, and reconfirmed that no files are staged."
    }
  ],
  "validationOutput": [
    "functions.read returned ENOENT for both /Users/sousavf/Documents/vea-ai/plan.md and /Users/sousavf/Documents/vea-ai/progress.md.",
    "Repository inspection found no application source, tests, package manifest, plan, or progress file.",
    "Artifact validation reported: 275 lines; acceptance JSON valid; all required topic headings present.",
    "git diff --cached --name-only produced no paths."
  ],
  "residualRisks": [
    "No implementation or architecture plan exists to validate against the threat model.",
    "Provider-specific contractual requirements require legal/product review and cannot be resolved generically.",
    "Platform-specific sandbox and filesystem controls require separate macOS, Windows, and Linux validation.",
    "Same-user malware and harmful actions knowingly approved by the user remain outside reliable application-level prevention."
  ],
  "noStagedFiles": true,
  "diffSummary": "No repository diff; created only the required external review artifact under /tmp.",
  "reviewFindings": [
    "blocker: pre-implementation - the P0 architecture/release gates must be designed and tested before shipping privileged orchestration features.",
    "note: /Users/sousavf/Documents/vea-ai/plan.md and progress.md were requested but are absent, so no proposed design or milestone state could be reviewed.",
    "note: worktree isolation must not be represented as a security sandbox."
  ],
  "manualNotes": "Review gate completed. No tests were run because the repository contains no implementation or test suite. .pi-subagents/ was pre-existing orchestration state and was not modified."
}
```
