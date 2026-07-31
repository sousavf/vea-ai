import { z } from "zod";

const accountSchema = z
  .object({
    id: z.string().min(1),
    adapter: z.string().min(1),
    auth: z.discriminatedUnion("type", [
      z.object({ type: z.literal("apiKey"), credentialRef: z.string().min(1) }).strict(),
      z.object({ type: z.literal("oauth"), credentialRef: z.string().min(1) }).strict(),
      z.object({ type: z.literal("cliSession") }).strict(),
    ]),
    enabled: z.boolean(),
    allowedProjects: z.array(z.string().min(1)).min(1),
    maxConcurrentRuns: z.number().int().min(1).max(32).default(2),
  })
  .strict();

const modelAliasSchema = z
  .object({
    account: z.string().min(1),
    model: z.string().min(1),
    efforts: z
      .partialRecord(z.enum(["off", "low", "medium", "high", "max"]), z.string().min(1))
      .default({}),
    tags: z.array(z.string()).default([]),
  })
  .strict();

export const veaConfigSchema = z
  .object({
    $schema: z.string().optional(),
    configVersion: z.literal(1),
    ui: z
      .object({
        theme: z.enum(["system", "light", "dark"]).default("system"),
        density: z.enum(["comfortable", "compact"]).default("comfortable"),
        confirmProviderSubmission: z.boolean().default(true),
      })
      .strict(),
    scheduler: z
      .object({
        globalMaxRuns: z.number().int().min(1).max(64).default(4),
        perProjectMaxRuns: z.number().int().min(1).max(16).default(2),
        defaultProviderMaxRuns: z.number().int().min(1).max(32).default(2),
        fairness: z.literal("weighted-deficit-round-robin").default("weighted-deficit-round-robin"),
        projectWeights: z.record(z.string(), z.number().min(0.1).max(100)).default({}),
        unknownWriteScopeConflicts: z.boolean().default(true),
      })
      .strict(),
    accounts: z.array(accountSchema).default([]),
    models: z.record(z.string(), modelAliasSchema).default({}),
    routing: z
      .object({
        defaultPolicy: z.string().min(1).default("balanced"),
        allowCrossProviderFallback: z.boolean().default(false),
        preferSubscription: z.boolean().default(true),
        maxEstimatedCostUsdPerTask: z.number().nonnegative().default(5),
      })
      .strict(),
    security: z
      .object({
        trustedProjects: z.array(z.string()).default([]),
        diagnosticContentLogging: z.literal(false).default(false),
        auditRetentionDays: z.number().int().min(1).max(3650).default(90),
      })
      .strict(),
    skills: z
      .object({
        enabled: z.boolean().default(true),
        userRoots: z.array(z.string()).default(["~/.agents/skills"]),
        maxFilesPerSkill: z.number().int().min(1).max(1000).default(100),
        maxBytesPerSkill: z.number().int().min(1024).max(10_485_760).default(1_048_576),
      })
      .strict(),
    mcp: z
      .object({ enabled: z.boolean().default(false), servers: z.array(z.never()).default([]) })
      .strict(),
    plugins: z
      .object({ enabled: z.boolean().default(false), packages: z.array(z.never()).default([]) })
      .strict(),
  })
  .strict();

export type VeaConfig = z.infer<typeof veaConfigSchema>;

export const defaultConfig: VeaConfig = veaConfigSchema.parse({
  configVersion: 1,
  ui: {},
  scheduler: {},
  accounts: [],
  models: {},
  routing: {},
  security: {},
  skills: {},
  mcp: {},
  plugins: {},
});

export function parseConfig(input: unknown): VeaConfig {
  return veaConfigSchema.parse(input);
}

export function parseConfigResult(
  input: unknown,
):
  | { ok: true; config: VeaConfig }
  | { ok: false; issues: readonly { path: string; message: string }[] } {
  const result = veaConfigSchema.safeParse(input);
  if (result.success) return { ok: true, config: result.data };
  return {
    ok: false,
    issues: result.error.issues.map((issue) => ({
      path: issue.path.join("."),
      message: issue.message,
    })),
  };
}
