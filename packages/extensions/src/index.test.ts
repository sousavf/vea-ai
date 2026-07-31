import { describe, expect, it } from "vitest";
import {
  canEnableWithoutApproval,
  parseExtensionManifest,
  requiresExecutableTrust,
} from "./index.js";

const digest = `sha256:${"a".repeat(64)}`;

describe("extension trust planes", () => {
  it("keeps portable skills capability-free", () => {
    const skill = parseExtensionManifest({
      manifestVersion: 1,
      id: "example.skill",
      displayName: "Example skill",
      version: "1.0.0",
      digest,
      plane: "skill",
      source: ".agents/skills/example/SKILL.md",
      capabilities: [],
    });
    expect(requiresExecutableTrust(skill)).toBe(false);
    expect(canEnableWithoutApproval(skill)).toBe(true);
  });

  it("requires privileged Pi compatibility to be explicit", () => {
    expect(() =>
      parseExtensionManifest({
        manifestVersion: 1,
        id: "example.pi-package",
        displayName: "Pi package",
        version: "1.0.0",
        digest,
        plane: "pi-compat",
        package: "example-pi-package",
        privileged: false,
        capabilities: [],
      }),
    ).toThrow();

    const manifest = parseExtensionManifest({
      manifestVersion: 1,
      id: "example.pi-package",
      displayName: "Pi package",
      version: "1.0.0",
      digest,
      plane: "pi-compat",
      package: "example-pi-package",
      privileged: true,
      capabilities: "full-system-access",
    });
    expect(requiresExecutableTrust(manifest)).toBe(true);
  });

  it("rejects undeclared manifest fields", () => {
    expect(() =>
      parseExtensionManifest({
        manifestVersion: 1,
        id: "example.plugin",
        displayName: "Plugin",
        version: "1.0.0",
        digest,
        plane: "restricted-plugin",
        entrypoint: "index.js",
        facadeRange: "^1",
        capabilities: [],
        nodeIntegration: true,
      }),
    ).toThrow();
  });
});
