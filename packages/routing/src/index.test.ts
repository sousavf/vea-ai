import { describe, expect, it } from "vitest";
import type { ModelDescriptor, TaskNode } from "@vea/domain";
import { routeTask, type RouteCandidate } from "./index.js";

const task: TaskNode = {
  id: "task-1",
  projectId: "project-1",
  graphId: "graph-1",
  title: "Implement scheduler",
  instructions: "Implement deterministic scheduling",
  kind: "agent",
  role: "implement",
  effort: "high",
  status: "ready",
  priority: 5,
  dependsOn: [],
  readScopes: ["packages/**"],
  writeScopes: ["packages/scheduler/**"],
  requiredCapabilities: ["tools", "code"],
  budget: { maxTurns: 30, maxMinutes: 30 },
};

function model(id: string): ModelDescriptor {
  return {
    id,
    adapterId: id.split(":")[0] ?? "adapter",
    accountId: id,
    modelId: id,
    displayName: id,
    capabilities: {
      attachments: ["image"],
      structuredOutput: true,
      tools: true,
      steering: true,
      fork: true,
      resume: "same-version",
      skills: "host",
      mcpOwnership: "host",
      efforts: ["low", "medium", "high"],
      tags: ["tools", "code"],
    },
  };
}

function candidate(id: string, overrides: Partial<RouteCandidate> = {}): RouteCandidate {
  return {
    id,
    model: model(id),
    providerEfforts: { low: "low", medium: "medium", high: "high" },
    healthy: true,
    termsReviewed: true,
    projectAllowed: true,
    adapterLoad: 0.25,
    rolePreference: 0.8,
    reliability: 0.9,
    latencyScore: 0.7,
    costScore: 0.6,
    budgetHeadroom: 0.5,
    ...overrides,
  };
}

const policy = {
  allowEffortDowngrade: false,
  maxEstimatedCostUsd: Number.POSITIVE_INFINITY,
  estimatedCostUsd: {},
  now: "2026-01-01T00:00:00Z",
};

describe("routeTask", () => {
  it("chooses deterministically and records rejected candidates", () => {
    const result = routeTask(
      task,
      [
        candidate("slow", { latencyScore: 0.1 }),
        candidate("fast", { latencyScore: 1 }),
        candidate("disabled", { termsReviewed: false }),
      ],
      policy,
    );
    expect(result.decision?.chosenCandidateId).toBe("fast");
    expect(result.rejected).toEqual([{ candidateId: "disabled", reasons: ["terms-not-reviewed"] }]);
  });

  it("keeps unknown subscription quota honest", () => {
    const result = routeTask(
      task,
      [
        candidate("subscription", {
          quota: {
            accountId: "subscription",
            source: "unknown",
            confidence: "unknown",
            observedAt: "2026-01-01T00:00:00Z",
          },
        }),
      ],
      policy,
    );
    expect(result.decision?.reasons).toContain("subscription quota unknown");
  });

  it("only lowers effort when policy allows it", () => {
    const limited = candidate("limited", { providerEfforts: { low: "low", medium: "medium" } });
    expect(routeTask(task, [limited], policy).decision).toBeNull();
    expect(
      routeTask(task, [limited], { ...policy, allowEffortDowngrade: true }).decision
        ?.normalizedEffort,
    ).toBe("medium");
  });

  it("enforces the stricter task-level monetary cap", () => {
    const cappedTask = {
      ...task,
      budget: { ...task.budget, maxEstimatedCostUsd: 1 },
    };
    const result = routeTask(cappedTask, [candidate("expensive")], {
      ...policy,
      maxEstimatedCostUsd: 5,
      estimatedCostUsd: { expensive: 3 },
    });
    expect(result.decision).toBeNull();
    expect(result.rejected[0]?.reasons).toContain("budget-exceeded");
  });

  it("requires an estimate when a finite monetary cap applies", () => {
    const cappedTask = {
      ...task,
      budget: { ...task.budget, maxEstimatedCostUsd: 0 },
    };
    const result = routeTask(cappedTask, [candidate("unknown-cost")], {
      ...policy,
      maxEstimatedCostUsd: 0,
    });
    expect(result.decision).toBeNull();
    expect(result.rejected[0]?.reasons).toContain("cost-estimate-required");
  });

  it("rejects non-finite and negative cost estimates", () => {
    for (const estimate of [Number.NaN, Number.POSITIVE_INFINITY, -1]) {
      const result = routeTask(task, [candidate("invalid-cost")], {
        ...policy,
        estimatedCostUsd: { "invalid-cost": estimate },
      });
      expect(result.decision).toBeNull();
      expect(result.rejected[0]?.reasons).toContain("invalid-cost-estimate");
    }
  });

  it("ignores expired quota while preserving configured budget headroom", () => {
    const result = routeTask(
      task,
      [
        candidate("stale", {
          quota: {
            accountId: "stale",
            source: "official",
            remainingRatio: 0,
            confidence: "high",
            observedAt: "2025-12-01T00:00:00Z",
            expiresAt: "2025-12-02T00:00:00Z",
          },
        }),
      ],
      policy,
    );
    expect(result.decision?.chosenCandidateId).toBe("stale");
    expect(result.decision?.reasons).toContain("quota snapshot expired; used configured budget");
  });

  it("rejects exhausted documented quota", () => {
    const result = routeTask(
      task,
      [
        candidate("exhausted", {
          quota: {
            accountId: "exhausted",
            source: "official",
            remainingRatio: 0,
            confidence: "high",
            observedAt: "2026-01-01T00:00:00Z",
          },
        }),
      ],
      policy,
    );
    expect(result.decision).toBeNull();
    expect(result.rejected[0]?.reasons).toContain("quota-exhausted");
  });
});
