import Ajv2020 from "ajv/dist/2020.js";
import { describe, expect, it } from "vitest";
import configJsonSchema from "../../../schemas/vea-config.schema.json";
import { defaultConfig, parseConfigResult } from "./index.js";

describe("Vea configuration", () => {
  it("builds secure defaults", () => {
    expect(defaultConfig.scheduler.globalMaxRuns).toBe(4);
    expect(defaultConfig.mcp.enabled).toBe(false);
    expect(defaultConfig.plugins.enabled).toBe(false);
    expect(defaultConfig.security.diagnosticContentLogging).toBe(false);
  });

  it("keeps runtime defaults compatible with the public JSON Schema", () => {
    const minimal = {
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
    };
    expect(parseConfigResult(minimal).ok).toBe(true);
    const validateSchema = new Ajv2020({ strict: false }).compile(configJsonSchema);
    expect(validateSchema(minimal), JSON.stringify(validateSchema.errors)).toBe(true);
  });

  it("rejects unknown fields", () => {
    const result = parseConfigResult({ ...defaultConfig, hiddenBackdoor: true });
    expect(result.ok).toBe(false);
  });

  it("accepts opaque credential references without secret values", () => {
    const result = parseConfigResult({
      ...defaultConfig,
      accounts: [
        {
          id: "openai-main",
          adapter: "openai-api",
          auth: { type: "apiKey", credentialRef: "cred_01" },
          enabled: true,
          allowedProjects: ["*"],
        },
      ],
    });
    expect(result.ok).toBe(true);
  });

  it("does not permit project-controlled MCP or plugin entries in v1", () => {
    expect(parseConfigResult({ ...defaultConfig, mcp: { enabled: true, servers: [{}] } }).ok).toBe(
      false,
    );
    expect(
      parseConfigResult({ ...defaultConfig, plugins: { enabled: true, packages: ["pkg"] } }).ok,
    ).toBe(false);
  });
});
