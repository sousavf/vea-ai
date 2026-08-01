use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use vea_policy::{
    ACTION_SCHEMA_VERSION, ActionOperation, ActionProposal, ApprovalError, ApprovalGrant,
    ApprovalStatus, CapabilityKind, CapabilityScope, DenialReason, Destination, EffectivePolicy,
    ExecutionState, MAX_ACTION_TTL_MS, POLICY_SCHEMA_VERSION, PathCapability, PolicyDecision,
    Provenance, Reversibility, RiskClass, RuleDecision, StateDigest, evaluate_action,
    verify_approval,
};

fn digest(character: char) -> String {
    format!("sha256:{}", character.to_string().repeat(64))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoldenVector {
    schema: String,
    action: ActionProposal,
    canonical_action: String,
    action_digest: String,
    policy: EffectivePolicy,
    canonical_policy: String,
    policy_digest: String,
    execution_state: ExecutionState,
    canonical_binding: String,
    binding_digest: String,
}

fn state_digest(character: char) -> StateDigest {
    StateDigest::parse(digest(character)).unwrap()
}

fn read_scope(run_id: &str) -> CapabilityScope {
    CapabilityScope::ProjectPaths {
        capability: PathCapability::Read,
        project_id: "project-1".into(),
        run_id: run_id.into(),
        worktree_id: "worktree-1".into(),
        paths: vec!["src/policy.rs".into()],
    }
}

fn patch_scope() -> CapabilityScope {
    CapabilityScope::ProjectPaths {
        capability: PathCapability::ApplyPatch,
        project_id: "project-1".into(),
        run_id: "run-1".into(),
        worktree_id: "worktree-1".into(),
        paths: vec!["src/policy.rs".into()],
    }
}

fn read_action() -> ActionProposal {
    ActionProposal {
        schema_version: ACTION_SCHEMA_VERSION,
        action_id: "action-1".into(),
        run_id: "run-1".into(),
        project_id: "project-1".into(),
        requested_capability: read_scope("run-1"),
        operation: ActionOperation::ReadFiles {
            worktree_id: "worktree-1".into(),
            paths: vec!["src/policy.rs".into()],
        },
        provenance: vec![Provenance {
            source_kind: "user_prompt".into(),
            source_id: "message-1".into(),
            content_digest: digest('a'),
        }],
        created_at_unix_ms: 1_000,
        expires_at_unix_ms: 1_000 + MAX_ACTION_TTL_MS,
    }
}

fn patch_action() -> ActionProposal {
    let mut action = read_action();
    action.requested_capability = patch_scope();
    action.operation = ActionOperation::ApplyPatch {
        worktree_id: "worktree-1".into(),
        patch_digest: digest('b'),
        patch_bytes: 128,
        affected_paths: vec!["src/policy.rs".into()],
    };
    action
}

fn execution_state() -> ExecutionState {
    ExecutionState {
        project_state: state_digest('c'),
        resource_state: state_digest('d'),
        destination_state: None,
    }
}

fn policy_with(grant: CapabilityScope) -> EffectivePolicy {
    let kind = grant.kind();
    let decision = if kind == CapabilityKind::ProjectRead {
        RuleDecision::AllowWithoutApproval
    } else {
        RuleDecision::RequireApproval { max_ttl_ms: 60_000 }
    };
    EffectivePolicy {
        schema_version: POLICY_SCHEMA_VERSION,
        revision: "revision-1".into(),
        trusted_projects: BTreeMap::from([("project-1".into(), state_digest('c'))]),
        grants: BTreeSet::from([grant]),
        rules: BTreeMap::from([(kind, decision)]),
        max_approval_ttl_ms: 60_000,
    }
}

#[test]
fn action_vector_is_stable_and_language_neutral() {
    let vector: GoldenVector =
        serde_json::from_str(include_str!("../../../tests/fixtures/policy-v1.json")).unwrap();
    assert_eq!(vector.schema, "vea.policy-vector.v1");
    assert_eq!(
        vector.action.canonical_bytes().unwrap(),
        vector.canonical_action.as_bytes()
    );
    let action_digest = vector.action.digest().unwrap();
    assert_eq!(action_digest.as_str(), vector.action_digest);
    assert_eq!(
        vector.policy.canonical_bytes().unwrap(),
        vector.canonical_policy.as_bytes()
    );
    let policy_digest = vector.policy.digest().unwrap();
    assert_eq!(policy_digest.as_str(), vector.policy_digest);
    assert_eq!(
        vector
            .execution_state
            .binding_canonical_bytes(&action_digest, &policy_digest)
            .unwrap(),
        vector.canonical_binding.as_bytes()
    );
    assert_eq!(
        vector
            .execution_state
            .binding_digest(&action_digest, &policy_digest)
            .unwrap()
            .as_str(),
        vector.binding_digest
    );
}

#[test]
fn action_digest_is_domain_separated_and_mutation_sensitive() {
    let action = patch_action();
    let first = action.digest().unwrap();
    assert!(first.is_valid());
    assert_eq!(first, action.digest().unwrap());

    let mut mutated = action.clone();
    if let ActionOperation::ApplyPatch { patch_bytes, .. } = &mut mutated.operation {
        *patch_bytes += 1;
    }
    assert_ne!(first, mutated.digest().unwrap());
    assert_eq!(action.risk_class(), RiskClass::LocalMutation);
    assert_eq!(action.reversibility(), Reversibility::WorktreeRecoverable);
}

#[test]
fn proposal_cannot_self_grant_read_authority() {
    let action = read_action();
    let policy = policy_with(read_scope("different-run"));
    assert_eq!(
        evaluate_action(&action, &policy, &execution_state(), 2_000),
        PolicyDecision::Denied {
            reason: DenialReason::NoCapabilityGrant
        }
    );

    let policy = policy_with(read_scope("run-1"));
    assert!(matches!(
        evaluate_action(&action, &policy, &execution_state(), 2_000),
        PolicyDecision::AllowedWithoutApproval { .. }
    ));
}

#[test]
fn missing_rules_and_empty_grants_are_valid_default_deny_policy() {
    let action = read_action();
    let mut policy = policy_with(read_scope("run-1"));
    policy.rules.clear();
    assert!(policy.validate().is_ok());
    assert_eq!(
        evaluate_action(&action, &policy, &execution_state(), 2_000),
        PolicyDecision::Denied {
            reason: DenialReason::NoPolicyRule
        }
    );

    policy.grants.clear();
    assert!(policy.validate().is_ok());
    assert_eq!(
        evaluate_action(&action, &policy, &execution_state(), 2_000),
        PolicyDecision::Denied {
            reason: DenialReason::NoCapabilityGrant
        }
    );
    policy.trusted_projects.clear();
    assert!(policy.validate().is_ok());
}

#[test]
fn complete_effective_policy_is_digest_bound() {
    let policy = policy_with(patch_scope());
    let original = policy.digest().unwrap();

    let mut changed_grant = policy.clone();
    changed_grant.grants = BTreeSet::from([read_scope("run-1")]);
    assert_ne!(original, changed_grant.digest().unwrap());

    let mut changed_trust = policy.clone();
    changed_trust
        .trusted_projects
        .insert("project-1".into(), state_digest('e'));
    assert_ne!(original, changed_trust.digest().unwrap());

    let mut changed_limit = policy.clone();
    changed_limit.max_approval_ttl_ms = 30_000;
    assert_ne!(original, changed_limit.digest().unwrap());

    let mut changed_rule = policy.clone();
    changed_rule.rules.insert(
        CapabilityKind::ProjectApplyPatch,
        RuleDecision::RequireApproval { max_ttl_ms: 15_000 },
    );
    assert_ne!(original, changed_rule.digest().unwrap());
}

#[test]
fn approval_is_bound_to_action_policy_and_fresh_broker_state() {
    let action = patch_action();
    let policy = policy_with(patch_scope());
    let state = execution_state();
    let PolicyDecision::ApprovalRequired { requirement } =
        evaluate_action(&action, &policy, &state, 2_000)
    else {
        panic!("patch should require approval");
    };
    let approval =
        ApprovalGrant::approve(&requirement, "approval-1", "local-user", 2_000, 30_000).unwrap();
    let authorized = verify_approval(&action, &policy, &state, &approval, 3_000).unwrap();
    assert_eq!(authorized.action_id, action.action_id);

    let mut changed_action = action.clone();
    if let ActionOperation::ApplyPatch { patch_bytes, .. } = &mut changed_action.operation {
        *patch_bytes += 1;
    }
    assert_eq!(
        verify_approval(&changed_action, &policy, &state, &approval, 3_000),
        Err(ApprovalError::RequirementChanged)
    );

    let mut changed_policy = policy.clone();
    changed_policy.max_approval_ttl_ms = 30_000;
    assert_eq!(
        verify_approval(&action, &changed_policy, &state, &approval, 3_000),
        Err(ApprovalError::RequirementChanged)
    );

    let drifted_state = ExecutionState {
        resource_state: state_digest('f'),
        ..state.clone()
    };
    assert_eq!(
        verify_approval(&action, &policy, &drifted_state, &approval, 3_000),
        Err(ApprovalError::RequirementChanged)
    );
}

#[test]
fn approvals_are_short_lived_and_single_use_by_contract() {
    let action = patch_action();
    let policy = policy_with(patch_scope());
    let state = execution_state();
    let PolicyDecision::ApprovalRequired { requirement } =
        evaluate_action(&action, &policy, &state, 2_000)
    else {
        panic!("patch should require approval");
    };
    assert_eq!(
        ApprovalGrant::approve(&requirement, "approval-1", "local-user", 2_000, 0),
        Err(ApprovalError::InvalidLifetime)
    );
    assert_eq!(
        ApprovalGrant::approve(&requirement, "approval-1", "local-user", 2_000, 60_001),
        Err(ApprovalError::InvalidLifetime)
    );

    let mut approval =
        ApprovalGrant::approve(&requirement, "approval-1", "local-user", 2_000, 1_000).unwrap();
    assert_eq!(
        verify_approval(&action, &policy, &state, &approval, 3_000),
        Err(ApprovalError::InvalidTimeWindow)
    );
    approval.status = ApprovalStatus::Consumed;
    assert_eq!(
        verify_approval(&action, &policy, &state, &approval, 2_500),
        Err(ApprovalError::NotUnused)
    );
}

#[test]
fn required_state_guards_cannot_be_omitted() {
    let destination = Destination {
        scheme: "https".into(),
        host: "api.example.com".into(),
        port: 443,
        path: "/v1/messages".into(),
    };
    let mut action = read_action();
    action.requested_capability = CapabilityScope::Provider {
        project_id: "project-1".into(),
        run_id: "run-1".into(),
        provider_id: "provider-1".into(),
        account_id: "account-1".into(),
        destination: destination.clone(),
    };
    action.operation = ActionOperation::SubmitProviderRequest {
        provider_id: "provider-1".into(),
        account_id: "account-1".into(),
        destination,
        request_digest: digest('f'),
        request_bytes: 42,
        data_classes: vec!["source_code".into()],
    };
    let policy = policy_with(action.requested_capability.clone());
    assert_eq!(
        evaluate_action(&action, &policy, &execution_state(), 2_000),
        PolicyDecision::Denied {
            reason: DenialReason::InvalidExecutionState
        }
    );
}

#[test]
fn invalid_paths_destinations_bounds_and_unknown_fields_fail_closed() {
    for invalid_path in [
        "/absolute",
        "../escape",
        "src\\file",
        ".git/config",
        "src//file",
    ] {
        let mut action = read_action();
        action.requested_capability = CapabilityScope::ProjectPaths {
            capability: PathCapability::Read,
            project_id: "project-1".into(),
            run_id: "run-1".into(),
            worktree_id: "worktree-1".into(),
            paths: vec![invalid_path.into()],
        };
        action.operation = ActionOperation::ReadFiles {
            worktree_id: "worktree-1".into(),
            paths: vec![invalid_path.into()],
        };
        assert!(action.validate_shape().is_err(), "accepted {invalid_path}");
    }

    let invalid_destination = Destination {
        scheme: "https".into(),
        host: "api..example.com".into(),
        port: 443,
        path: format!("/{}", "a".repeat(4_096)),
    };
    assert!(invalid_destination.validate().is_err());

    let mut policy = policy_with(patch_scope());
    policy.rules.insert(
        CapabilityKind::ProjectApplyPatch,
        RuleDecision::AllowWithoutApproval,
    );
    assert!(policy.validate().is_err());

    let mut value = serde_json::to_value(read_action()).unwrap();
    value["unknown"] = serde_json::json!(true);
    assert!(serde_json::from_value::<ActionProposal>(value).is_err());

    let mut expired = read_action();
    expired.expires_at_unix_ms = expired.created_at_unix_ms + MAX_ACTION_TTL_MS + 1;
    assert!(expired.validate_shape().is_err());
}
