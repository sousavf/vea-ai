import { describe, expect, it } from "vitest";
import type { Project, TaskNode } from "@vea/domain";
import { schedule, writeScopesConflict } from "./index.js";

const projects: Project[] = [
  {
    id: "alpha",
    displayName: "Alpha",
    repoRoot: "/alpha",
    defaultBranch: "main",
    trustState: "trusted",
    weight: 1,
  },
  {
    id: "beta",
    displayName: "Beta",
    repoRoot: "/beta",
    defaultBranch: "main",
    trustState: "trusted",
    weight: 1,
  },
];

function task(id: string, projectId: string, writeScopes: string[], priority = 0): TaskNode {
  return {
    id,
    projectId,
    graphId: `graph-${projectId}`,
    title: id,
    instructions: id,
    kind: "agent",
    role: "implement",
    effort: "medium",
    status: "ready",
    priority,
    dependsOn: [],
    readScopes: ["**"],
    writeScopes,
    requiredCapabilities: [],
    budget: { maxTurns: 20, maxMinutes: 20 },
    readyAt: "2026-01-01T00:00:00Z",
  };
}

const limits = {
  globalMaxRuns: 2,
  perProjectMaxRuns: 2,
  perAdapterMaxRuns: { codex: 2 },
  perAccountMaxRuns: { primary: 2 },
  unknownWriteScopeConflicts: true,
};

describe("scope conflict detection", () => {
  it("detects overlapping prefixes conservatively", () => {
    expect(writeScopesConflict(["src/auth/**"], ["src/auth/login.ts"])).toBe(true);
    expect(writeScopesConflict(["src/auth/**"], ["src/billing/**"])).toBe(false);
    expect(writeScopesConflict(["**"], ["docs/**"])).toBe(true);
  });
});

describe("cross-project scheduler", () => {
  it("selects independent projects fairly and deterministically", () => {
    const result = schedule({
      projects,
      readyTasks: [task("alpha-a", "alpha", ["src/a/**"]), task("beta-a", "beta", ["src/**"])],
      activeRuns: [],
      routeByTask: {
        "alpha-a": { adapterId: "codex", accountId: "primary" },
        "beta-a": { adapterId: "codex", accountId: "primary" },
      },
      limits,
    });
    expect(result.selectedTaskIds).toEqual(["alpha-a", "beta-a"]);
  });

  it("provides proportional service for integer and fractional project weights", () => {
    function simulate(alphaWeight: number, ticks = 2_200): number {
      const weightedProjects = [
        { ...projects[0]!, weight: alphaWeight },
        { ...projects[1]!, weight: 1 },
      ];
      let deficits: Readonly<Record<string, number>> = {};
      let cursorProjectId: string | undefined;
      const counts = { alpha: 0, beta: 0 };

      for (let tick = 0; tick < ticks; tick += 1) {
        const alphaTask = task(`alpha-${tick}`, "alpha", ["src/a/**"]);
        const betaTask = task(`beta-${tick}`, "beta", ["src/b/**"]);
        const result = schedule({
          projects: weightedProjects,
          readyTasks: [alphaTask, betaTask],
          activeRuns: [],
          routeByTask: {
            [alphaTask.id]: { adapterId: "codex", accountId: "primary" },
            [betaTask.id]: { adapterId: "codex", accountId: "primary" },
          },
          limits: { ...limits, globalMaxRuns: 1 },
          deficits,
          ...(cursorProjectId ? { cursorProjectId } : {}),
        });
        const selected = result.selectedTaskIds[0];
        if (selected?.startsWith("alpha")) counts.alpha += 1;
        if (selected?.startsWith("beta")) counts.beta += 1;
        deficits = result.deficits;
        cursorProjectId = result.cursorProjectId;
      }

      expect(counts.alpha).toBeGreaterThan(0);
      expect(counts.beta).toBeGreaterThan(0);
      return counts.alpha / counts.beta;
    }

    for (const weight of [10, 2, 1.1, 0.25]) {
      const observedRatio = simulate(weight);
      expect(observedRatio).toBeGreaterThanOrEqual(weight * 0.9);
      expect(observedRatio).toBeLessThanOrEqual(weight * 1.1);
    }
  });

  it("prevents write conflicts inside a project", () => {
    const result = schedule({
      projects,
      readyTasks: [task("alpha-b", "alpha", ["src/auth/**"])],
      activeRuns: [
        {
          taskId: "alpha-a",
          projectId: "alpha",
          adapterId: "codex",
          accountId: "primary",
          writeScopes: ["src/auth/login.ts"],
        },
      ],
      routeByTask: { "alpha-b": { adapterId: "codex", accountId: "primary" } },
      limits,
    });
    expect(result.selectedTaskIds).toEqual([]);
    expect(result.blocked["alpha-b"]).toBe("conflict");
  });

  it("honors account concurrency", () => {
    const result = schedule({
      projects,
      readyTasks: [task("beta-a", "beta", ["src/**"])],
      activeRuns: [
        {
          taskId: "alpha-a",
          projectId: "alpha",
          adapterId: "codex",
          accountId: "primary",
          writeScopes: ["src/**"],
        },
      ],
      routeByTask: { "beta-a": { adapterId: "codex", accountId: "primary" } },
      limits: { ...limits, perAccountMaxRuns: { primary: 1 } },
    });
    expect(result.selectedTaskIds).toEqual([]);
    expect(result.blocked["beta-a"]).toBe("concurrency");
  });
});
