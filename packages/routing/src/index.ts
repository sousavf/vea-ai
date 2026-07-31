import type { Effort, ModelDescriptor, QuotaSnapshot, RouteDecision, TaskNode } from "@vea/domain";

export interface RouteCandidate {
  id: string;
  model: ModelDescriptor;
  providerEfforts: Readonly<Partial<Record<Effort, string>>>;
  healthy: boolean;
  termsReviewed: boolean;
  projectAllowed: boolean;
  adapterLoad: number;
  rolePreference: number;
  reliability: number;
  latencyScore: number;
  costScore: number;
  budgetHeadroom: number;
  quota?: QuotaSnapshot;
}

export interface RoutingPolicy {
  allowEffortDowngrade: boolean;
  maxEstimatedCostUsd: number;
  estimatedCostUsd: Readonly<Record<string, number>>;
  now: string;
}

export interface RoutingResult {
  decision: RouteDecision | null;
  rejected: readonly { candidateId: string; reasons: readonly string[] }[];
}

const orderedEfforts: readonly Effort[] = ["off", "low", "medium", "high", "max"];

function clamp(value: number): number {
  return Math.max(0, Math.min(1, value));
}

function resolveEffort(
  requested: Effort,
  efforts: Readonly<Partial<Record<Effort, string>>>,
  allowDowngrade: boolean,
): { normalized: Effort; provider: string; downgraded: boolean } | null {
  const exact = efforts[requested];
  if (exact) return { normalized: requested, provider: exact, downgraded: false };
  if (!allowDowngrade) return null;
  const requestedIndex = orderedEfforts.indexOf(requested);
  for (let index = requestedIndex - 1; index >= 0; index -= 1) {
    const normalized = orderedEfforts[index];
    if (!normalized) continue;
    const provider = efforts[normalized];
    if (provider) return { normalized, provider, downgraded: true };
  }
  return null;
}

function quotaIsFresh(candidate: RouteCandidate, now: string): boolean {
  if (!candidate.quota?.expiresAt) return true;
  return Date.parse(candidate.quota.expiresAt) > Date.parse(now);
}

function rejectReasons(task: TaskNode, candidate: RouteCandidate, policy: RoutingPolicy): string[] {
  const reasons: string[] = [];
  if (!candidate.healthy) reasons.push("adapter-unhealthy");
  if (!candidate.termsReviewed) reasons.push("terms-not-reviewed");
  if (!candidate.projectAllowed) reasons.push("project-not-allowed");
  if (quotaIsFresh(candidate, policy.now) && candidate.quota?.remainingRatio === 0) {
    reasons.push("quota-exhausted");
  }
  const estimatedCost = policy.estimatedCostUsd[candidate.id];
  const taskCap = task.budget.maxEstimatedCostUsd ?? Number.POSITIVE_INFINITY;
  const effectiveCap = Math.min(policy.maxEstimatedCostUsd, taskCap);
  if (estimatedCost !== undefined && (!Number.isFinite(estimatedCost) || estimatedCost < 0)) {
    reasons.push("invalid-cost-estimate");
  } else if (estimatedCost === undefined && Number.isFinite(effectiveCap)) {
    reasons.push("cost-estimate-required");
  } else if (estimatedCost !== undefined && estimatedCost > effectiveCap) {
    reasons.push("budget-exceeded");
  }
  if (
    !task.requiredCapabilities.every((capability) =>
      candidate.model.capabilities.tags.includes(capability),
    )
  ) {
    reasons.push("missing-capability");
  }
  if (!resolveEffort(task.effort, candidate.providerEfforts, policy.allowEffortDowngrade)) {
    reasons.push("unsupported-effort");
  }
  return reasons;
}

function score(candidate: RouteCandidate, now: string): number {
  const quotaHeadroom = quotaIsFresh(candidate, now)
    ? (candidate.quota?.remainingRatio ?? candidate.budgetHeadroom)
    : candidate.budgetHeadroom;
  return (
    0.35 +
    clamp(quotaHeadroom) * 0.2 +
    clamp(candidate.rolePreference) * 0.15 +
    clamp(candidate.reliability) * 0.1 +
    clamp(candidate.latencyScore) * 0.1 +
    clamp(candidate.costScore) * 0.05 +
    (1 - clamp(candidate.adapterLoad)) * 0.05
  );
}

export function routeTask(
  task: TaskNode,
  candidates: readonly RouteCandidate[],
  policy: RoutingPolicy,
): RoutingResult {
  const rejected: { candidateId: string; reasons: readonly string[] }[] = [];
  const eligible: { candidate: RouteCandidate; score: number }[] = [];

  for (const candidate of candidates) {
    const reasons = rejectReasons(task, candidate, policy);
    if (reasons.length > 0) {
      rejected.push({ candidateId: candidate.id, reasons });
      continue;
    }
    eligible.push({ candidate, score: score(candidate, policy.now) });
  }

  const chosen = eligible.sort(
    (left, right) =>
      right.score - left.score || left.candidate.id.localeCompare(right.candidate.id),
  )[0];
  if (!chosen) return { decision: null, rejected };

  const effort = resolveEffort(
    task.effort,
    chosen.candidate.providerEfforts,
    policy.allowEffortDowngrade,
  );
  if (!effort) return { decision: null, rejected };

  const reasons = [
    `capabilities matched`,
    `role preference ${chosen.candidate.rolePreference.toFixed(2)}`,
    chosen.candidate.quota?.source === "unknown"
      ? "subscription quota unknown"
      : quotaIsFresh(chosen.candidate, policy.now)
        ? `quota source ${chosen.candidate.quota?.source ?? "configured-budget"}`
        : "quota snapshot expired; used configured budget",
  ];
  if (effort.downgraded) reasons.push(`effort lowered from ${task.effort} to ${effort.normalized}`);

  return {
    decision: {
      taskId: task.id,
      chosenCandidateId: chosen.candidate.id,
      normalizedEffort: effort.normalized,
      providerEffort: effort.provider,
      score: Number(chosen.score.toFixed(6)),
      reasons,
      rejected,
    },
    rejected,
  };
}
