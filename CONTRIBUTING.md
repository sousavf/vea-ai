# Contributing to Vea

Vea welcomes issues, design review, adapter research, documentation, and code contributions.

## Development

```bash
npx pnpm@11.18.0 install
npx pnpm@11.18.0 check
```

Use small changes with tests. Never commit provider keys, OAuth tokens, CLI credential stores, private transcripts, or repository contents used as fixtures.

## Architecture rules

1. Keep the renderer unprivileged.
2. Route privileged operations through typed host policy.
3. Do not treat worktrees, containers, signatures, or redaction as complete security boundaries.
4. Keep direct model adapters distinct from native agent runtime adapters.
5. Represent unsupported capabilities explicitly; do not emulate silently.
6. Keep subscription quota `unknown` unless supported telemetry provides evidence.
7. Do not add arbitrary shell or executable integrations without a security decision and adversarial tests.

Read [`docs/PLAN.md`](docs/PLAN.md) before proposing structural changes.

## Commits and pull requests

- Explain the user-visible behavior and security impact.
- Add or update tests for contracts and state transitions.
- List validation commands and remaining risks.
- Provider adapters must update `docs/providers/matrix.md`.
