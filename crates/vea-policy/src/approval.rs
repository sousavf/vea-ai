use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ActionProposal, EffectivePolicy, ExecutionState,
    canonical::{ActionDigest, ApprovalBindingDigest, PolicyDigest},
    engine::{ApprovalRequirement, PolicyDecision, evaluate_action},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approved,
    Denied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Unused,
    Consumed,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApprovalGrant {
    pub approval_id: String,
    pub action_id: String,
    pub actor_id: String,
    pub decision: ApprovalDecision,
    pub status: ApprovalStatus,
    pub action_digest: ActionDigest,
    pub policy_digest: PolicyDigest,
    pub binding_digest: ApprovalBindingDigest,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthorizedAction {
    pub approval_id: String,
    pub action_id: String,
    pub binding_digest: ApprovalBindingDigest,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ApprovalError {
    #[error("approval identifier or actor is invalid")]
    InvalidIdentity,
    #[error("approval lifetime is invalid")]
    InvalidLifetime,
    #[error("approval was denied")]
    Denied,
    #[error("approval has already been consumed or revoked")]
    NotUnused,
    #[error("action no longer requires this approval")]
    RequirementChanged,
    #[error("approval is not currently valid")]
    InvalidTimeWindow,
}

impl ApprovalGrant {
    pub fn approve(
        requirement: &ApprovalRequirement,
        approval_id: impl Into<String>,
        actor_id: impl Into<String>,
        issued_at_unix_ms: u64,
        ttl_ms: u64,
    ) -> Result<Self, ApprovalError> {
        let approval_id = approval_id.into();
        let actor_id = actor_id.into();
        if !valid_identity(&approval_id) || !valid_identity(&actor_id) {
            return Err(ApprovalError::InvalidIdentity);
        }
        let Some(expires_at_unix_ms) = issued_at_unix_ms.checked_add(ttl_ms) else {
            return Err(ApprovalError::InvalidLifetime);
        };
        if ttl_ms == 0
            || ttl_ms > requirement.max_ttl_ms
            || issued_at_unix_ms < requirement.action_created_at_unix_ms
            || issued_at_unix_ms >= requirement.action_expires_at_unix_ms
            || expires_at_unix_ms > requirement.action_expires_at_unix_ms
        {
            return Err(ApprovalError::InvalidLifetime);
        }
        Ok(Self {
            approval_id,
            action_id: requirement.action_id.clone(),
            actor_id,
            decision: ApprovalDecision::Approved,
            status: ApprovalStatus::Unused,
            action_digest: requirement.action_digest.clone(),
            policy_digest: requirement.policy_digest.clone(),
            binding_digest: requirement.binding_digest.clone(),
            issued_at_unix_ms,
            expires_at_unix_ms,
        })
    }
}

pub fn verify_approval(
    action: &ActionProposal,
    policy: &EffectivePolicy,
    fresh_state: &ExecutionState,
    approval: &ApprovalGrant,
    now_unix_ms: u64,
) -> Result<AuthorizedAction, ApprovalError> {
    if approval.decision != ApprovalDecision::Approved {
        return Err(ApprovalError::Denied);
    }
    if !valid_identity(&approval.approval_id) || !valid_identity(&approval.actor_id) {
        return Err(ApprovalError::InvalidIdentity);
    }
    if approval.status != ApprovalStatus::Unused {
        return Err(ApprovalError::NotUnused);
    }
    if now_unix_ms < approval.issued_at_unix_ms || now_unix_ms >= approval.expires_at_unix_ms {
        return Err(ApprovalError::InvalidTimeWindow);
    }
    let PolicyDecision::ApprovalRequired { requirement } =
        evaluate_action(action, policy, fresh_state, now_unix_ms)
    else {
        return Err(ApprovalError::RequirementChanged);
    };
    if approval.expires_at_unix_ms <= approval.issued_at_unix_ms
        || approval.expires_at_unix_ms - approval.issued_at_unix_ms > requirement.max_ttl_ms
        || approval.issued_at_unix_ms < requirement.action_created_at_unix_ms
        || approval.expires_at_unix_ms > requirement.action_expires_at_unix_ms
        || approval.action_id != requirement.action_id
        || approval.action_digest != requirement.action_digest
        || approval.policy_digest != requirement.policy_digest
        || approval.binding_digest != requirement.binding_digest
        || !approval.action_digest.is_valid()
        || !approval.policy_digest.is_valid()
        || !approval.binding_digest.is_valid()
    {
        return Err(ApprovalError::RequirementChanged);
    }
    Ok(AuthorizedAction {
        approval_id: approval.approval_id.clone(),
        action_id: approval.action_id.clone(),
        binding_digest: approval.binding_digest.clone(),
    })
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().enumerate().all(|(index, byte)| {
            if index == 0 {
                byte.is_ascii_alphanumeric()
            } else {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
            }
        })
}
