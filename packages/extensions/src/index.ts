import { z } from "zod";

export const extensionPlanes = ["skill", "mcp", "restricted-plugin", "pi-compat"] as const;
export type ExtensionPlane = (typeof extensionPlanes)[number];

export const capabilitySchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("project.read"), globs: z.array(z.string()).min(1) }).strict(),
  z
    .object({ kind: z.literal("project.propose-write"), globs: z.array(z.string()).min(1) })
    .strict(),
  z.object({ kind: z.literal("network"), hosts: z.array(z.string()).min(1) }).strict(),
  z.object({ kind: z.literal("provider"), operations: z.array(z.string()).min(1) }).strict(),
  z.object({ kind: z.literal("ui"), surfaces: z.array(z.string()).min(1) }).strict(),
]);

export type ExtensionCapability = z.infer<typeof capabilitySchema>;

const baseManifest = {
  manifestVersion: z.literal(1),
  id: z.string().regex(/^[a-z0-9][a-z0-9.-]{1,127}$/),
  displayName: z.string().min(1).max(100),
  version: z.string().min(1).max(64),
  digest: z.string().regex(/^sha256:[a-f0-9]{64}$/),
  homepage: z.url().optional(),
};

export const extensionManifestSchema = z.discriminatedUnion("plane", [
  z
    .object({
      ...baseManifest,
      plane: z.literal("skill"),
      source: z.string().min(1),
      capabilities: z.tuple([]),
    })
    .strict(),
  z
    .object({
      ...baseManifest,
      plane: z.literal("mcp"),
      transport: z.enum(["stdio", "streamable-http"]),
      capabilities: z.array(capabilitySchema),
      reviewed: z.boolean(),
    })
    .strict(),
  z
    .object({
      ...baseManifest,
      plane: z.literal("restricted-plugin"),
      entrypoint: z.string().min(1),
      facadeRange: z.string().min(1),
      capabilities: z.array(capabilitySchema),
    })
    .strict(),
  z
    .object({
      ...baseManifest,
      plane: z.literal("pi-compat"),
      package: z.string().min(1),
      privileged: z.literal(true),
      capabilities: z.literal("full-system-access"),
    })
    .strict(),
]);

export type ExtensionManifest = z.infer<typeof extensionManifestSchema>;

export interface SkillDescriptor {
  name: string;
  description: string;
  path: string;
  scope: "bundled" | "user" | "project";
  contentHash: string;
  allowedToolsAdvisory: readonly string[];
  enabled: boolean;
}

export interface ExtensionRecord {
  manifest: ExtensionManifest;
  enabledProjectIds: readonly string[];
  approvedCapabilityDigest?: string;
  state: "discovered" | "disabled" | "awaiting-approval" | "enabled" | "revoked";
}

export function parseExtensionManifest(input: unknown): ExtensionManifest {
  return extensionManifestSchema.parse(input);
}

export function requiresExecutableTrust(manifest: ExtensionManifest): boolean {
  return manifest.plane !== "skill";
}

export function canEnableWithoutApproval(manifest: ExtensionManifest): boolean {
  return manifest.plane === "skill" && manifest.capabilities.length === 0;
}
