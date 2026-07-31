import { describe, expect, it } from "vitest";
import { canTransitionTask, type TaskGraph, validateTaskGraph } from "./index.js";

function graph(dependencies: Record<string, string[]>): TaskGraph {
  return {
    id: "graph-1",
    projectId: "project-1",
    version: 1,
    title: "Test graph",
    baseRevision: "abc123",
    nodes: Object.entries(dependencies).map(([id, dependsOn]) => ({
      id,
      projectId: "project-1",
      graphId: "graph-1",
      title: id,
      instructions: "test",
      kind: "agent",
      role: "implement",
      effort: "medium",
      status: "waiting",
      priority: 0,
      dependsOn,
      readScopes: ["**"],
      writeScopes: ["src/**"],
      requiredCapabilities: [],
      budget: { maxTurns: 10, maxMinutes: 10 },
    })),
  };
}

describe("validateTaskGraph", () => {
  it("returns a stable topological order", () => {
    const result = validateTaskGraph(graph({ build: ["plan"], plan: [], review: ["build"] }));
    expect(result).toEqual({ valid: true, order: ["plan", "build", "review"], errors: [] });
  });

  it("rejects cycles", () => {
    const result = validateTaskGraph(graph({ first: ["second"], second: ["first"] }));
    expect(result.valid).toBe(false);
    expect(result.errors).toContain("Task graph contains a cycle");
  });

  it("rejects missing dependencies", () => {
    const result = validateTaskGraph(graph({ build: ["missing"] }));
    expect(result.valid).toBe(false);
    expect(result.errors[0]).toContain("missing task");
  });

  it("rejects tasks attached to a different graph", () => {
    const input = graph({ build: [] });
    const result = validateTaskGraph({
      ...input,
      nodes: input.nodes.map((node) => ({ ...node, graphId: "other-graph" })),
    });
    expect(result.valid).toBe(false);
    expect(result.errors[0]).toContain("different graph");
  });
});

describe("task state transitions", () => {
  it("permits the happy path and rejects terminal mutation", () => {
    expect(canTransitionTask("ready", "queued")).toBe(true);
    expect(canTransitionTask("running", "succeeded")).toBe(true);
    expect(canTransitionTask("running", "blocked")).toBe(true);
    expect(canTransitionTask("succeeded", "running")).toBe(false);
  });
});
