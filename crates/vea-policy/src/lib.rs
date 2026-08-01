mod action;
mod approval;
mod canonical;
mod engine;

pub use action::{
    ACTION_SCHEMA_VERSION, ActionError, ActionOperation, ActionProposal, CapabilityKind,
    CapabilityScope, Destination, MAX_ACTION_TTL_MS, MAX_CANONICAL_ACTION_BYTES, PathCapability,
    Provenance, Reversibility, RiskClass,
};
pub use approval::{
    ApprovalDecision, ApprovalError, ApprovalGrant, ApprovalStatus, AuthorizedAction,
    verify_approval,
};
pub use canonical::{
    ActionDigest, ApprovalBindingDigest, CanonicalError, PolicyDigest, StateDigest,
};
pub use engine::{
    ApprovalRequirement, DenialReason, EffectivePolicy, ExecutionState, MAX_CANONICAL_POLICY_BYTES,
    POLICY_SCHEMA_VERSION, PolicyDecision, PolicyError, RuleDecision, evaluate_action,
};
