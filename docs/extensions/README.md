# Extension compatibility

Vea uses four explicit extension planes because they have different authority and portability.

| Plane                       | Purpose                                | Default trust                                       |
| --------------------------- | -------------------------------------- | --------------------------------------------------- |
| Agent Skills                | Portable instructions in `SKILL.md`    | Untrusted text; no independent permissions          |
| MCP                         | Portable tools, resources, and prompts | Untrusted server; host-mediated capabilities        |
| Restricted plugins          | Versioned Vea worker facade            | Out-of-process, manifest capabilities, default deny |
| Privileged Pi compatibility | Deep Pi extensions/packages            | Equivalent to installing trusted software           |

Vea will not silently translate one plane into another. A skill cannot grant a tool, MCP metadata cannot authorize execution, and a restricted plugin cannot become privileged through configuration.

The TypeScript contracts live in [`packages/extensions`](../../packages/extensions). Full implementation is gated by the security milestones in [`docs/PLAN.md`](../PLAN.md).
