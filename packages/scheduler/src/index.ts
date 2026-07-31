import type { Effort, Project, TaskNode } from "@vea/domain";

export interface ActiveRun {
  taskId: string;
  projectId: string;
  adapterId: string;
  accountId: string;
  writeScopes: readonly string[];
}

export interface SchedulingLimits {
  globalMaxRuns: number;
  perProjectMaxRuns: number;
  perAdapterMaxRuns: Readonly<Record<string, number>>;
  perAccountMaxRuns: Readonly<Record<string, number>>;
  unknownWriteScopeConflicts: boolean;
}

export interface SchedulingSnapshot {
  projects: readonly Project[];
  readyTasks: readonly TaskNode[];
  activeRuns: readonly ActiveRun[];
  routeByTask: Readonly<Record<string, { adapterId: string; accountId: string }>>;
  limits: SchedulingLimits;
  deficits?: Readonly<Record<string, number>>;
  cursorProjectId?: string;
}

export type BlockReason = "trust" | "route" | "concurrency" | "conflict";

export interface SchedulerDecision {
  selectedTaskIds: readonly string[];
  blocked: Readonly<Record<string, BlockReason>>;
  deficits: Readonly<Record<string, number>>;
  cursorProjectId?: string;
}

const effortCost: Record<Effort, number> = { off: 1, low: 1, medium: 2, high: 4, max: 6 };

function scopePrefix(scope: string): string | null {
  const normalized = scope.replaceAll("\\", "/").replace(/^\.\//, "");
  if (!normalized || normalized === "**" || normalized === "*") return null;
  const wildcard = normalized.search(/[?*[]/);
  return (wildcard === -1 ? normalized : normalized.slice(0, wildcard)).replace(/\/$/, "");
}

export function writeScopesConflict(
  left: readonly string[],
  right: readonly string[],
  unknownConflicts = true,
): boolean {
  if (left.length === 0 || right.length === 0) return unknownConflicts;
  for (const leftScope of left) {
    for (const rightScope of right) {
      const leftPrefix = scopePrefix(leftScope);
      const rightPrefix = scopePrefix(rightScope);
      if (leftPrefix === null || rightPrefix === null) return unknownConflicts;
      if (
        leftPrefix === rightPrefix ||
        leftPrefix.startsWith(`${rightPrefix}/`) ||
        rightPrefix.startsWith(`${leftPrefix}/`)
      ) {
        return true;
      }
    }
  }
  return false;
}

function increment(counts: Map<string, number>, key: string): void {
  counts.set(key, (counts.get(key) ?? 0) + 1);
}

function stableTaskOrder(left: TaskNode, right: TaskNode): number {
  return (
    right.priority - left.priority ||
    (left.readyAt ?? "").localeCompare(right.readyAt ?? "") ||
    left.id.localeCompare(right.id)
  );
}

export function schedule(snapshot: SchedulingSnapshot): SchedulerDecision {
  const projectById = new Map(snapshot.projects.map((project) => [project.id, project]));
  const projectRuns = new Map<string, number>();
  const adapterRuns = new Map<string, number>();
  const accountRuns = new Map<string, number>();
  const activeScopes = new Map<string, ReadonlyArray<readonly string[]>>();

  for (const run of snapshot.activeRuns) {
    increment(projectRuns, run.projectId);
    increment(adapterRuns, run.adapterId);
    increment(accountRuns, run.accountId);
    const scopes = activeScopes.get(run.projectId) ?? [];
    activeScopes.set(run.projectId, [...scopes, run.writeScopes]);
  }

  const blocked: Record<string, BlockReason> = {};
  const queues = new Map<string, TaskNode[]>();

  for (const task of [...snapshot.readyTasks].sort(stableTaskOrder)) {
    const project = projectById.get(task.projectId);
    if (!project || project.trustState !== "trusted") {
      blocked[task.id] = "trust";
      continue;
    }
    const route = snapshot.routeByTask[task.id];
    if (!route) {
      blocked[task.id] = "route";
      continue;
    }
    const adapterLimit =
      snapshot.limits.perAdapterMaxRuns[route.adapterId] ?? Number.POSITIVE_INFINITY;
    const accountLimit =
      snapshot.limits.perAccountMaxRuns[route.accountId] ?? Number.POSITIVE_INFINITY;
    if (
      (projectRuns.get(task.projectId) ?? 0) >= snapshot.limits.perProjectMaxRuns ||
      (adapterRuns.get(route.adapterId) ?? 0) >= adapterLimit ||
      (accountRuns.get(route.accountId) ?? 0) >= accountLimit
    ) {
      blocked[task.id] = "concurrency";
      continue;
    }
    const conflicts = (activeScopes.get(task.projectId) ?? []).some((scopes) =>
      writeScopesConflict(task.writeScopes, scopes, snapshot.limits.unknownWriteScopeConflicts),
    );
    if (conflicts) {
      blocked[task.id] = "conflict";
      continue;
    }
    const queue = queues.get(task.projectId) ?? [];
    queue.push(task);
    queues.set(task.projectId, queue);
  }

  const deficits: Record<string, number> = {};
  for (const project of snapshot.projects)
    deficits[project.id] = snapshot.deficits?.[project.id] ?? 0;
  const selected: string[] = [];
  const availableSlots = Math.max(0, snapshot.limits.globalMaxRuns - snapshot.activeRuns.length);
  const projectOrder = [...snapshot.projects].sort((left, right) =>
    left.id.localeCompare(right.id),
  );
  const requestedCursor = snapshot.cursorProjectId
    ? projectOrder.findIndex((project) => project.id === snapshot.cursorProjectId)
    : 0;
  let cursor = Math.max(0, requestedCursor);
  let visitsWithoutWork = 0;
  const maxVisitsWithoutWork = Math.max(1, projectOrder.length * 64);

  // Weighted deficit round robin visits projects in stable order. Persisting the
  // cursor and deficits across ticks prevents a high-weight project from
  // monopolizing a one-slot scheduler while retaining proportional bursts.
  while (
    selected.length < availableSlots &&
    [...queues.values()].some((queue) => queue.length > 0) &&
    visitsWithoutWork < maxVisitsWithoutWork
  ) {
    const projectIndex = cursor;
    const project = projectOrder[projectIndex];
    cursor = projectOrder.length === 0 ? 0 : (cursor + 1) % projectOrder.length;
    if (!project) break;

    const queue = queues.get(project.id);
    if (!queue || queue.length === 0) {
      visitsWithoutWork += 1;
      continue;
    }
    if ((projectRuns.get(project.id) ?? 0) >= snapshot.limits.perProjectMaxRuns) {
      for (const queuedTask of queue.splice(0)) blocked[queuedTask.id] = "concurrency";
      visitsWithoutWork += 1;
      continue;
    }

    const task = queue[0];
    if (!task) {
      visitsWithoutWork += 1;
      continue;
    }
    const cost = effortCost[task.effort];
    if ((deficits[project.id] ?? 0) < cost) {
      const normalizedWeight = Number.isFinite(project.weight)
        ? Math.min(100, Math.max(0.1, project.weight))
        : 1;
      const quantum = normalizedWeight * 2;
      deficits[project.id] = (deficits[project.id] ?? 0) + quantum;
    }
    if ((deficits[project.id] ?? 0) < cost) {
      visitsWithoutWork += 1;
      continue;
    }
    queue.shift();

    const conflicts = (activeScopes.get(task.projectId) ?? []).some((scopes) =>
      writeScopesConflict(task.writeScopes, scopes, snapshot.limits.unknownWriteScopeConflicts),
    );
    if (conflicts) {
      blocked[task.id] = "conflict";
      visitsWithoutWork += 1;
      continue;
    }

    const route = snapshot.routeByTask[task.id];
    if (!route) {
      blocked[task.id] = "route";
      visitsWithoutWork += 1;
      continue;
    }
    const adapterLimit =
      snapshot.limits.perAdapterMaxRuns[route.adapterId] ?? Number.POSITIVE_INFINITY;
    const accountLimit =
      snapshot.limits.perAccountMaxRuns[route.accountId] ?? Number.POSITIVE_INFINITY;
    if (
      (adapterRuns.get(route.adapterId) ?? 0) >= adapterLimit ||
      (accountRuns.get(route.accountId) ?? 0) >= accountLimit
    ) {
      blocked[task.id] = "concurrency";
      visitsWithoutWork += 1;
      continue;
    }

    selected.push(task.id);
    deficits[project.id] = (deficits[project.id] ?? 0) - cost;
    increment(projectRuns, project.id);
    increment(adapterRuns, route.adapterId);
    increment(accountRuns, route.accountId);
    const scopes = activeScopes.get(project.id) ?? [];
    activeScopes.set(project.id, [...scopes, task.writeScopes]);

    const nextTask = queue[0];
    const canContinueCurrentVisit = nextTask
      ? (deficits[project.id] ?? 0) >= effortCost[nextTask.effort]
      : selected.length >= availableSlots && (deficits[project.id] ?? 0) >= cost;
    if (canContinueCurrentVisit) cursor = projectIndex;
    visitsWithoutWork = 0;
  }

  for (const queue of queues.values()) {
    for (const task of queue) {
      if (!(task.id in blocked)) blocked[task.id] = "concurrency";
    }
  }

  const cursorProjectId = projectOrder[cursor]?.id;
  return cursorProjectId
    ? { selectedTaskIds: selected, blocked, deficits, cursorProjectId }
    : { selectedTaskIds: selected, blocked, deficits };
}
