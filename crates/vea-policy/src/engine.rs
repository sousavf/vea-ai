use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    action::{ActionProposal, CapabilityKind, CapabilityScope, MAX_ACTION_TTL_MS, validate_id},
    canonical::{
        ActionDigest, ApprovalBindingDigest, CanonicalError, PolicyDigest, StateDigest,
        canonical_json, domain_digest,
    },
};

pub const POLICY_SCHEMA_VERSION: u16 = 1;
pub const MAX_CANONICAL_POLICY_BYTES: usize = 256 * 1024;
const MAX_TRUSTED_PROJECTS: usize = 256;
const MAX_CAPABILITY_GRANTS: usize = 1_024;
const MAX_CAPABILITY_RULES: usize = 6;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuleDecision {
    AllowWithoutApproval,
    RequireApproval { max_ttl_ms: u64 },
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EffectivePolicy {
    pub schema_version: u16,
    pub revision: String,
    pub trusted_projects: BTreeMap<String, StateDigest>,
    pub grants: BTreeSet<CapabilityScope>,
    pub rules: BTreeMap<CapabilityKind, RuleDecision>,
    pub max_approval_ttl_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecutionState {
    pub project_state: StateDigest,
    pub resource_state: StateDigest,
    pub destination_state: Option<StateDigest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRequirement {
    pub action_id: String,
    pub action_digest: ActionDigest,
    pub policy_digest: PolicyDigest,
    pub binding_digest: ApprovalBindingDigest,
    pub max_ttl_ms: u64,
    pub action_created_at_unix_ms: u64,
    pub action_expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DenialReason {
    InvalidAction,
    InvalidPolicy,
    InvalidExecutionState,
    Expired,
    UntrustedProject,
    NoCapabilityGrant,
    NoPolicyRule,
    RuleDenied,
    ProjectStateDrift,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum PolicyDecision {
    AllowedWithoutApproval {
        action_digest: ActionDigest,
        policy_digest: PolicyDigest,
        binding_digest: ApprovalBindingDigest,
    },
    ApprovalRequired {
        requirement: ApprovalRequirement,
    },
    Denied {
        reason: DenialReason,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyError {
    #[error(transparent)]
    Canonical(#[from] CanonicalError),
    #[error("invalid policy field: {0}")]
    InvalidField(&'static str),
    #[error("canonical policy exceeds {MAX_CANONICAL_POLICY_BYTES} bytes")]
    PolicyTooLarge,
}

impl EffectivePolicy {
    pub fn validate(&self) -> Result<(), PolicyError> {
        if self.schema_version != POLICY_SCHEMA_VERSION {
            return Err(PolicyError::InvalidField("schema_version"));
        }
        if validate_id(&self.revision, "revision").is_err() {
            return Err(PolicyError::InvalidField("revision"));
        }
        if self.trusted_projects.len() > MAX_TRUSTED_PROJECTS
            || self.trusted_projects.iter().any(|(project_id, digest)| {
                validate_id(project_id, "trusted_project_id").is_err() || !digest.is_valid()
            })
        {
            return Err(PolicyError::InvalidField("trusted_projects"));
        }
        if self.grants.len() > MAX_CAPABILITY_GRANTS
            || self.grants.iter().any(|grant| grant.validate().is_err())
        {
            return Err(PolicyError::InvalidField("grants"));
        }
        if self.rules.len() > MAX_CAPABILITY_RULES
            || self
                .rules
                .iter()
                .any(|(capability, decision)| match decision {
                    RuleDecision::AllowWithoutApproval => {
                        *capability != CapabilityKind::ProjectRead
                    }
                    RuleDecision::RequireApproval { max_ttl_ms } => {
                        *max_ttl_ms == 0 || *max_ttl_ms > MAX_ACTION_TTL_MS
                    }
                    RuleDecision::Deny => false,
                })
        {
            return Err(PolicyError::InvalidField("rules"));
        }
        if self.max_approval_ttl_ms == 0 || self.max_approval_ttl_ms > MAX_ACTION_TTL_MS {
            return Err(PolicyError::InvalidField("max_approval_ttl_ms"));
        }
        if canonical_json(self)?.len() > MAX_CANONICAL_POLICY_BYTES {
            return Err(PolicyError::PolicyTooLarge);
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PolicyError> {
        self.validate()?;
        Ok(canonical_json(self)?)
    }

    pub fn digest(&self) -> Result<PolicyDigest, PolicyError> {
        let canonical = self.canonical_bytes()?;
        Ok(PolicyDigest::from_raw(domain_digest(
            b"vea\0policy\0v1\0",
            &canonical,
        )))
    }
}

impl ExecutionState {
    pub fn validate_for(&self, action: &ActionProposal) -> bool {
        self.project_state.is_valid()
            && self.resource_state.is_valid()
            && self
                .destination_state
                .as_ref()
                .is_none_or(StateDigest::is_valid)
            && action.operation.requires_destination_state() == self.destination_state.is_some()
    }

    pub fn binding_canonical_bytes(
        &self,
        action_digest: &ActionDigest,
        policy_digest: &PolicyDigest,
    ) -> Result<Vec<u8>, CanonicalError> {
        canonical_json(&Binding {
            action_digest,
            policy_digest,
            project_state: &self.project_state,
            resource_state: &self.resource_state,
            destination_state: &self.destination_state,
        })
    }

    pub fn binding_digest(
        &self,
        action_digest: &ActionDigest,
        policy_digest: &PolicyDigest,
    ) -> Result<ApprovalBindingDigest, CanonicalError> {
        let canonical = self.binding_canonical_bytes(action_digest, policy_digest)?;
        Ok(ApprovalBindingDigest::from_raw(domain_digest(
            b"vea\0approval-binding\0v1\0",
            &canonical,
        )))
    }
}

pub fn evaluate_action(
    action: &ActionProposal,
    policy: &EffectivePolicy,
    state: &ExecutionState,
    now_unix_ms: u64,
) -> PolicyDecision {
    if action.validate_shape().is_err() {
        return PolicyDecision::Denied {
            reason: DenialReason::InvalidAction,
        };
    }
    if policy.validate().is_err() {
        return PolicyDecision::Denied {
            reason: DenialReason::InvalidPolicy,
        };
    }
    if !state.validate_for(action) {
        return PolicyDecision::Denied {
            reason: DenialReason::InvalidExecutionState,
        };
    }
    if now_unix_ms < action.created_at_unix_ms || now_unix_ms >= action.expires_at_unix_ms {
        return PolicyDecision::Denied {
            reason: DenialReason::Expired,
        };
    }
    let Some(trusted_state) = policy.trusted_projects.get(&action.project_id) else {
        return PolicyDecision::Denied {
            reason: DenialReason::UntrustedProject,
        };
    };
    if trusted_state != &state.project_state {
        return PolicyDecision::Denied {
            reason: DenialReason::ProjectStateDrift,
        };
    }
    if !policy.grants.iter().any(|grant| grant.authorizes(action)) {
        return PolicyDecision::Denied {
            reason: DenialReason::NoCapabilityGrant,
        };
    }
    let Some(rule) = policy.rules.get(&action.requested_capability.kind()) else {
        return PolicyDecision::Denied {
            reason: DenialReason::NoPolicyRule,
        };
    };
    if matches!(rule, RuleDecision::Deny) {
        return PolicyDecision::Denied {
            reason: DenialReason::RuleDenied,
        };
    }

    let Ok(action_digest) = action.digest() else {
        return PolicyDecision::Denied {
            reason: DenialReason::InvalidAction,
        };
    };
    let Ok(policy_digest) = policy.digest() else {
        return PolicyDecision::Denied {
            reason: DenialReason::InvalidPolicy,
        };
    };
    let Ok(binding_digest) = binding_digest(&action_digest, &policy_digest, state) else {
        return PolicyDecision::Denied {
            reason: DenialReason::InvalidExecutionState,
        };
    };

    match rule {
        RuleDecision::AllowWithoutApproval => PolicyDecision::AllowedWithoutApproval {
            action_digest,
            policy_digest,
            binding_digest,
        },
        RuleDecision::RequireApproval { max_ttl_ms } => PolicyDecision::ApprovalRequired {
            requirement: ApprovalRequirement {
                action_id: action.action_id.clone(),
                action_digest,
                policy_digest,
                binding_digest,
                max_ttl_ms: policy.max_approval_ttl_ms.min(*max_ttl_ms),
                action_created_at_unix_ms: action.created_at_unix_ms,
                action_expires_at_unix_ms: action.expires_at_unix_ms,
            },
        },
        RuleDecision::Deny => PolicyDecision::Denied {
            reason: DenialReason::RuleDenied,
        },
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct Binding<'a> {
    action_digest: &'a ActionDigest,
    policy_digest: &'a PolicyDigest,
    project_state: &'a StateDigest,
    resource_state: &'a StateDigest,
    destination_state: &'a Option<StateDigest>,
}

pub(crate) fn binding_digest(
    action_digest: &ActionDigest,
    policy_digest: &PolicyDigest,
    state: &ExecutionState,
) -> Result<ApprovalBindingDigest, CanonicalError> {
    state.binding_digest(action_digest, policy_digest)
}
