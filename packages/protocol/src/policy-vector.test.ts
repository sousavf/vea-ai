import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import vector from "../../../tests/fixtures/policy-v1.json";

function canonicalJson(value: unknown): string {
  if (value === null || typeof value === "boolean" || typeof value === "string") {
    return JSON.stringify(value);
  }
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) throw new Error("canonical policy values require safe integers");
    return String(value);
  }
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (typeof value === "object") {
    const record = value as Record<string, unknown>;
    return `{${Object.keys(record)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`)
      .join(",")}}`;
  }
  throw new Error(`unsupported canonical value: ${typeof value}`);
}

function domainHash(domain: string, canonical: string): string {
  const hash = createHash("sha256")
    .update(Buffer.from(domain, "utf8"))
    .update(Buffer.from(canonical, "utf8"))
    .digest("hex");
  return `sha256:${hash}`;
}

describe("Vea policy v1 cross-language vector", () => {
  it("matches Rust canonical bytes and all domain-separated digests", () => {
    const action = canonicalJson(vector.action);
    expect(action).toBe(vector.canonicalAction);
    expect(domainHash("vea\0action\0v1\0", action)).toBe(vector.actionDigest);

    const policy = canonicalJson(vector.policy);
    expect(policy).toBe(vector.canonicalPolicy);
    expect(domainHash("vea\0policy\0v1\0", policy)).toBe(vector.policyDigest);

    const binding = canonicalJson({
      action_digest: vector.actionDigest,
      destination_state: vector.executionState.destination_state,
      policy_digest: vector.policyDigest,
      project_state: vector.executionState.project_state,
      resource_state: vector.executionState.resource_state,
    });
    expect(binding).toBe(vector.canonicalBinding);
    expect(domainHash("vea\0approval-binding\0v1\0", binding)).toBe(vector.bindingDigest);
  });
});
