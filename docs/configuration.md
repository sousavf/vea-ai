# Configuration

Vea reads versioned JSON configuration in this order:

1. bundled product defaults;
2. user configuration in the platform config directory;
3. trusted-project `.vea/project.json` for non-secret project settings;
4. task policy and an explicit one-run UI override.

Later layers may tighten security but project files cannot add credentials, executables, arbitrary endpoints, plugins, or MCP processes, and cannot weaken approval policy.

Validate user configuration against [`schemas/vea-config.schema.json`](../schemas/vea-config.schema.json). The initial TypeScript parser is in [`packages/config`](../packages/config).

Credentials are referenced by opaque `credentialRef` values. Secret material must never appear in this file, logs, task graphs, environment dumps, or renderer state.
