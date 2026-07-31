export const effortLevels = ["off", "low", "medium", "high", "max"] as const;
export type Effort = (typeof effortLevels)[number];

export const taskRoles = ["plan", "implement", "review", "research", "validate"] as const;
export type TaskRole = (typeof taskRoles)[number];

export const taskStatuses = [
  "draft",
  "waiting",
  "ready",
  "queued",
  "running",
  "succeeded",
  "failed",
  "cancelled",
  "blocked",
] as const;
export type TaskStatus = (typeof taskStatuses)[number];

export type TaskKind = "agent" | "approval" | "integration" | "validation";
export type TrustState = "untrusted" | "trusted" | "revoked";

export interface Project {
  id: string;
  displayName: string;
  repoRoot: string;
  defaultBranch: string;
  trustState: TrustState;
  weight: number;
}

export interface TaskBudget {
  maxTurns: number;
  maxMinutes: number;
  maxEstimatedCostUsd?: number;
}

export interface TaskNode {
  id: string;
  projectId: string;
  graphId: string;
  title: string;
  instructions: string;
  kind: TaskKind;
  role: TaskRole;
  effort: Effort;
  status: TaskStatus;
  priority: number;
  dependsOn: readonly string[];
  readScopes: readonly string[];
  writeScopes: readonly string[];
  requiredCapabilities: readonly string[];
  budget: TaskBudget;
  readyAt?: string;
}

export interface TaskGraph {
  id: string;
  projectId: string;
  version: number;
  title: string;
  baseRevision: string;
  nodes: readonly TaskNode[];
}

export interface GraphValidation {
  valid: boolean;
  order: readonly string[];
  errors: readonly string[];
}

export function validateTaskGraph(graph: TaskGraph): GraphValidation {
  const errors: string[] = [];
  const nodes = new Map(graph.nodes.map((node) => [node.id, node]));

  if (nodes.size !== graph.nodes.length) errors.push("Task IDs must be unique");
  if (graph.version < 1) errors.push("Graph version must be at least 1");
  if (!graph.baseRevision.trim()) errors.push("Graph base revision is required");

  const indegree = new Map<string, number>();
  const dependents = new Map<string, string[]>();

  for (const node of graph.nodes) {
    indegree.set(node.id, 0);
    if (node.projectId !== graph.projectId) {
      errors.push(`Task ${node.id} belongs to a different project`);
    }
    if (node.graphId !== graph.id) {
      errors.push(`Task ${node.id} belongs to a different graph`);
    }
  }

  for (const node of graph.nodes) {
    for (const dependencyId of node.dependsOn) {
      if (dependencyId === node.id) {
        errors.push(`Task ${node.id} cannot depend on itself`);
        continue;
      }
      if (!nodes.has(dependencyId)) {
        errors.push(`Task ${node.id} depends on missing task ${dependencyId}`);
        continue;
      }
      indegree.set(node.id, (indegree.get(node.id) ?? 0) + 1);
      const entries = dependents.get(dependencyId) ?? [];
      entries.push(node.id);
      dependents.set(dependencyId, entries);
    }
  }

  const queue = [...indegree.entries()]
    .filter(([, degree]) => degree === 0)
    .map(([id]) => id)
    .sort();
  const order: string[] = [];

  while (queue.length > 0) {
    const id = queue.shift();
    if (!id) break;
    order.push(id);
    for (const dependentId of (dependents.get(id) ?? []).sort()) {
      const next = (indegree.get(dependentId) ?? 1) - 1;
      indegree.set(dependentId, next);
      if (next === 0) {
        queue.push(dependentId);
        queue.sort();
      }
    }
  }

  if (order.length !== graph.nodes.length) errors.push("Task graph contains a cycle");
  return { valid: errors.length === 0, order, errors };
}

const taskTransitions: Record<TaskStatus, readonly TaskStatus[]> = {
  draft: ["waiting"],
  waiting: ["ready", "blocked", "cancelled"],
  ready: ["queued", "blocked", "cancelled"],
  queued: ["running", "cancelled"],
  running: ["succeeded", "failed", "cancelled", "blocked"],
  succeeded: [],
  failed: [],
  cancelled: [],
  blocked: ["waiting", "cancelled"],
};

export function canTransitionTask(from: TaskStatus, to: TaskStatus): boolean {
  return taskTransitions[from].includes(to);
}

export interface RuntimeCapabilities {
  attachments: readonly ("image" | "audio" | "file-ref")[];
  structuredOutput: boolean;
  tools: boolean;
  steering: boolean;
  fork: boolean;
  resume: "none" | "same-version" | "portable";
  skills: "host" | "native" | "none";
  mcpOwnership: "host" | "runtime" | "none";
  efforts: readonly Effort[];
  tags: readonly string[];
}

export interface ModelDescriptor {
  id: string;
  adapterId: string;
  accountId: string;
  modelId: string;
  displayName: string;
  capabilities: RuntimeCapabilities;
  inputCostPerMillion?: number;
  outputCostPerMillion?: number;
  contextWindow?: number;
}

export interface QuotaSnapshot {
  accountId: string;
  source: "official" | "configured-budget" | "observed-limit" | "unknown";
  remainingRatio?: number;
  confidence: "high" | "medium" | "low" | "unknown";
  observedAt: string;
  expiresAt?: string;
}

export interface RouteDecision {
  taskId: string;
  chosenCandidateId: string;
  normalizedEffort: Effort;
  providerEffort: string;
  score: number;
  reasons: readonly string[];
  rejected: readonly { candidateId: string; reasons: readonly string[] }[];
}

export type RuntimeEventKind =
  | "session.state"
  | "message.delta"
  | "message.final"
  | "tool.proposed"
  | "tool.started"
  | "tool.progress"
  | "tool.completed"
  | "approval.requested"
  | "approval.resolved"
  | "usage"
  | "warning"
  | "error"
  | "run.settled";

export interface RuntimeEventEnvelope {
  schemaVersion: 1;
  runtime: { id: string; version: string };
  hostSessionId: string;
  sequence: number;
  timestamp: string;
  kind: RuntimeEventKind;
  correlationId?: string;
  payload: unknown;
  raw?: { namespace: string; type: string; value: unknown };
}
