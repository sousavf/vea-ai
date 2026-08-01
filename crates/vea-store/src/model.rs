use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::StoreError;

pub const COMMAND_SCHEMA_VERSION: u16 = 1;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    User,
    System,
}

impl ActorKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::System => "system",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub kind: ActorKind,
    pub actor_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    Allowed,
    Approved,
}

impl PolicyDecision {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Approved => "approved",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "allowed" => Ok(Self::Allowed),
            "approved" => Ok(Self::Approved),
            _ => Err(StoreError::IntegrityFailure(
                "invalid audit policy decision".into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditResultClass {
    Succeeded,
    Failed,
    Authorized,
    Started,
    Unknown,
}

impl AuditResultClass {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Authorized => "authorized",
            Self::Started => "started",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "authorized" => Ok(Self::Authorized),
            "started" => Ok(Self::Started),
            "unknown" => Ok(Self::Unknown),
            _ => Err(StoreError::IntegrityFailure(
                "invalid audit result class".into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectResultClass {
    Succeeded,
    Failed,
}

impl SideEffectResultClass {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            _ => Err(StoreError::IntegrityFailure(
                "invalid side-effect result class".into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SideEffectAuditContext {
    pub policy_decision: PolicyDecision,
    pub provider_id: Option<String>,
    pub account_alias: Option<String>,
    pub affected_paths: Vec<String>,
    pub destination: Option<String>,
}

impl SideEffectAuditContext {
    pub(crate) fn validate(&self) -> Result<(), StoreError> {
        if let Some(provider_id) = &self.provider_id {
            validate_identifier(provider_id, "audit.provider_id")?;
        }
        if let Some(account_alias) = &self.account_alias {
            validate_identifier(account_alias, "audit.account_alias")?;
        }
        if self.affected_paths.len() > 64 {
            return Err(StoreError::InvalidInput("audit.affected_paths"));
        }
        for path in &self.affected_paths {
            validate_repo_root(path)?;
        }
        if let Some(destination) = &self.destination {
            validate_bounded(destination, 2_048, "audit.destination")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrustState {
    Untrusted,
    Trusted,
    Revoked,
}

impl TrustState {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Untrusted => "untrusted",
            Self::Trusted => "trusted",
            Self::Revoked => "revoked",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "untrusted" => Ok(Self::Untrusted),
            "trusted" => Ok(Self::Trusted),
            "revoked" => Ok(Self::Revoked),
            _ => Err(StoreError::IntegrityFailure(
                "invalid project trust state".into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub id: String,
    pub display_name: String,
    pub repo_root: String,
    pub repo_identity: String,
    pub default_branch: String,
    pub trust_state: TrustState,
    pub weight: f64,
    pub provider_policy: String,
    pub data_classification: String,
    pub revision: u64,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
    pub last_event_sequence: u64,
}

impl Project {
    pub(crate) fn validate(&self) -> Result<(), StoreError> {
        validate_uuid_v7(&self.id, "project.id")?;
        validate_display_name(&self.display_name)?;
        validate_repo_root(&self.repo_root)?;
        validate_bounded(&self.repo_identity, 512, "repo_identity")?;
        validate_branch(&self.default_branch)?;
        validate_identifier(&self.provider_policy, "provider_policy")?;
        validate_identifier(&self.data_classification, "data_classification")?;
        validate_weight(self.weight)?;
        if self.revision == 0 || self.updated_at_unix_ms < self.created_at_unix_ms {
            return Err(StoreError::InvalidInput("project metadata"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CreateProject {
    pub display_name: String,
    pub repo_root: String,
    pub repo_identity: String,
    pub default_branch: String,
    pub provider_policy: String,
    pub data_classification: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectCommand {
    Create(CreateProject),
    Rename {
        display_name: String,
    },
    SetTrust {
        trust_state: TrustState,
    },
    SetSchedulingWeight {
        weight: f64,
    },
    SetPolicy {
        provider_policy: String,
        data_classification: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectPhase {
    Authorized,
    Started,
    Finished,
    Unknown,
}

impl SideEffectPhase {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Authorized => "authorized",
            Self::Started => "started",
            Self::Finished => "finished",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "authorized" => Ok(Self::Authorized),
            "started" => Ok(Self::Started),
            "finished" => Ok(Self::Finished),
            "unknown" => Ok(Self::Unknown),
            _ => Err(StoreError::IntegrityFailure(
                "invalid side-effect phase".into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SideEffect {
    pub action_id: String,
    pub project_id: Option<String>,
    pub run_id: String,
    pub capability: String,
    pub action_digest: String,
    pub approval_id: String,
    pub binding_digest: String,
    pub phase: SideEffectPhase,
    pub revision: u64,
    pub result_class: Option<SideEffectResultClass>,
    pub authorized_at_unix_ms: u64,
    pub started_at_unix_ms: Option<u64>,
    pub finished_at_unix_ms: Option<u64>,
    pub updated_at_unix_ms: u64,
    pub last_event_sequence: u64,
}

impl SideEffect {
    pub(crate) fn validate(&self) -> Result<(), StoreError> {
        validate_uuid_v7(&self.action_id, "side_effect.action_id")?;
        if let Some(project_id) = &self.project_id {
            validate_uuid_v7(project_id, "side_effect.project_id")?;
        }
        validate_uuid_v7(&self.run_id, "side_effect.run_id")?;
        validate_identifier(&self.capability, "side_effect.capability")?;
        validate_sha256(&self.action_digest, "side_effect.action_digest")?;
        validate_uuid_v7(&self.approval_id, "side_effect.approval_id")?;
        validate_sha256(&self.binding_digest, "side_effect.binding_digest")?;
        if self.revision == 0 || self.updated_at_unix_ms < self.authorized_at_unix_ms {
            return Err(StoreError::InvalidInput("side-effect metadata"));
        }
        let timestamps_match = match self.phase {
            SideEffectPhase::Authorized => {
                self.started_at_unix_ms.is_none()
                    && self.finished_at_unix_ms.is_none()
                    && self.result_class.is_none()
            }
            SideEffectPhase::Started | SideEffectPhase::Unknown => {
                self.started_at_unix_ms
                    .is_some_and(|started| started >= self.authorized_at_unix_ms)
                    && self.finished_at_unix_ms.is_none()
                    && self.result_class.is_none()
            }
            SideEffectPhase::Finished => {
                self.started_at_unix_ms.is_some_and(|started| {
                    started >= self.authorized_at_unix_ms
                        && self
                            .finished_at_unix_ms
                            .is_some_and(|finished| finished >= started)
                }) && self.result_class.is_some()
            }
        };
        if !timestamps_match {
            return Err(StoreError::InvalidInput("side-effect timestamps"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SideEffectCommand {
    Authorize {
        project_id: Option<String>,
        run_id: String,
        capability: String,
        action_digest: String,
        approval_id: String,
        binding_digest: String,
        audit: Box<SideEffectAuditContext>,
    },
    Start,
    Finish {
        result_class: SideEffectResultClass,
    },
    MarkUnknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "aggregate", rename_all = "snake_case", deny_unknown_fields)]
pub enum StoreCommand {
    Project(ProjectCommand),
    SideEffect(SideEffectCommand),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CommandEnvelope {
    pub schema_version: u16,
    pub command_id: String,
    pub aggregate_id: String,
    pub expected_revision: u64,
    pub actor: Actor,
    pub correlation_id: String,
    pub causation_event_id: Option<String>,
    pub occurred_at_unix_ms: u64,
    pub command: StoreCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CommandReceipt {
    pub command_id: String,
    pub aggregate_id: String,
    pub aggregate_revision: u64,
    pub first_global_sequence: u64,
    pub last_global_sequence: u64,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EventRecord {
    pub global_sequence: u64,
    pub event_id: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub aggregate_revision: u64,
    pub schema_version: u16,
    pub kind: String,
    pub payload: serde_json::Value,
    pub command_id: String,
    pub causation_event_id: Option<String>,
    pub correlation_id: String,
    pub actor: Actor,
    pub occurred_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuditRecord {
    pub audit_sequence: u64,
    pub audit_id: String,
    pub occurred_at_unix_ms: u64,
    pub actor: Actor,
    pub action: String,
    pub project_id: Option<String>,
    pub run_id: Option<String>,
    pub action_id: Option<String>,
    pub command_id: String,
    pub correlation_id: String,
    pub policy_decision: Option<PolicyDecision>,
    pub approval_digest: Option<String>,
    pub provider_id: Option<String>,
    pub account_alias: Option<String>,
    pub affected_paths: Option<Vec<String>>,
    pub destination: Option<String>,
    pub result_class: AuditResultClass,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectSnapshot {
    pub cursor: u64,
    pub projects: Vec<Project>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VerificationReport {
    pub event_count: u64,
    pub project_count: u64,
    pub side_effect_count: u64,
    pub projections_rebuilt: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OpenReport {
    pub schema_version: u32,
    pub applied_migrations: Vec<u32>,
    pub recovered_side_effects: u64,
    pub verification: VerificationReport,
    pub backup_path: Option<String>,
}

impl CommandEnvelope {
    pub(crate) fn validate(&self) -> Result<(), StoreError> {
        if self.schema_version != COMMAND_SCHEMA_VERSION {
            return Err(StoreError::UnsupportedCommandSchema(self.schema_version));
        }
        validate_uuid_v7(&self.command_id, "command_id")?;
        validate_uuid_v7(&self.aggregate_id, "aggregate_id")?;
        validate_uuid_v7(&self.correlation_id, "correlation_id")?;
        if let Some(causation) = &self.causation_event_id {
            validate_uuid_v7(causation, "causation_event_id")?;
        }
        if self.expected_revision > MAX_SAFE_JSON_INTEGER
            || self.occurred_at_unix_ms > MAX_SAFE_JSON_INTEGER
        {
            return Err(StoreError::InvalidInput("numeric field"));
        }
        self.actor.validate()?;
        self.command.validate()
    }
}

impl Actor {
    pub(crate) fn validate(&self) -> Result<(), StoreError> {
        match self.kind {
            ActorKind::User => validate_identifier(
                self.actor_id
                    .as_deref()
                    .ok_or(StoreError::InvalidInput("actor_id"))?,
                "actor_id",
            ),
            ActorKind::System => {
                if let Some(actor_id) = &self.actor_id {
                    validate_identifier(actor_id, "actor_id")?;
                }
                Ok(())
            }
        }
    }
}

impl StoreCommand {
    pub(crate) fn aggregate_type(&self) -> &'static str {
        match self {
            Self::Project(_) => "project",
            Self::SideEffect(_) => "side_effect",
        }
    }

    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Project(ProjectCommand::Create(_)) => "project.create",
            Self::Project(ProjectCommand::Rename { .. }) => "project.rename",
            Self::Project(ProjectCommand::SetTrust { .. }) => "project.set_trust",
            Self::Project(ProjectCommand::SetSchedulingWeight { .. }) => {
                "project.set_scheduling_weight"
            }
            Self::Project(ProjectCommand::SetPolicy { .. }) => "project.set_policy",
            Self::SideEffect(SideEffectCommand::Authorize { .. }) => "side_effect.authorize",
            Self::SideEffect(SideEffectCommand::Start) => "side_effect.start",
            Self::SideEffect(SideEffectCommand::Finish { .. }) => "side_effect.finish",
            Self::SideEffect(SideEffectCommand::MarkUnknown) => "side_effect.mark_unknown",
        }
    }

    fn validate(&self) -> Result<(), StoreError> {
        match self {
            Self::Project(command) => command.validate(),
            Self::SideEffect(command) => command.validate(),
        }
    }
}

impl ProjectCommand {
    fn validate(&self) -> Result<(), StoreError> {
        match self {
            Self::Create(project) => project.validate(),
            Self::Rename { display_name } => validate_display_name(display_name),
            Self::SetTrust { .. } => Ok(()),
            Self::SetSchedulingWeight { weight } => validate_weight(*weight),
            Self::SetPolicy {
                provider_policy,
                data_classification,
            } => {
                validate_identifier(provider_policy, "provider_policy")?;
                validate_identifier(data_classification, "data_classification")
            }
        }
    }
}

impl CreateProject {
    fn validate(&self) -> Result<(), StoreError> {
        validate_display_name(&self.display_name)?;
        validate_repo_root(&self.repo_root)?;
        validate_bounded(&self.repo_identity, 512, "repo_identity")?;
        validate_branch(&self.default_branch)?;
        validate_identifier(&self.provider_policy, "provider_policy")?;
        validate_identifier(&self.data_classification, "data_classification")?;
        validate_weight(self.weight)
    }
}

impl SideEffectCommand {
    fn validate(&self) -> Result<(), StoreError> {
        match self {
            Self::Authorize {
                project_id,
                run_id,
                capability,
                action_digest,
                approval_id,
                binding_digest,
                audit,
            } => {
                if let Some(project_id) = project_id {
                    validate_uuid_v7(project_id, "project_id")?;
                }
                validate_uuid_v7(run_id, "run_id")?;
                validate_identifier(capability, "capability")?;
                validate_sha256(action_digest, "action_digest")?;
                validate_uuid_v7(approval_id, "approval_id")?;
                validate_sha256(binding_digest, "binding_digest")?;
                audit.validate()
            }
            Self::Start | Self::MarkUnknown | Self::Finish { .. } => Ok(()),
        }
    }
}

pub(crate) fn validate_uuid_v7(value: &str, field: &'static str) -> Result<(), StoreError> {
    let uuid = Uuid::parse_str(value).map_err(|_| StoreError::InvalidInput(field))?;
    if uuid.get_version_num() != 7 || uuid.to_string() != value {
        return Err(StoreError::InvalidInput(field));
    }
    Ok(())
}

pub(crate) fn validate_identifier(value: &str, field: &'static str) -> Result<(), StoreError> {
    if value.is_empty()
        || value.len() > 128
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(StoreError::InvalidInput(field));
    }
    Ok(())
}

fn validate_display_name(value: &str) -> Result<(), StoreError> {
    validate_bounded(value, 200, "display_name")
}

fn validate_bounded(value: &str, max: usize, field: &'static str) -> Result<(), StoreError> {
    if value.trim().is_empty()
        || value.chars().count() > max
        || value
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
    {
        return Err(StoreError::InvalidInput(field));
    }
    Ok(())
}

fn validate_repo_root(value: &str) -> Result<(), StoreError> {
    validate_bounded(value, 4_096, "repo_root")?;
    let path = Path::new(value);
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(StoreError::InvalidInput("repo_root"));
    }
    Ok(())
}

fn validate_branch(value: &str) -> Result<(), StoreError> {
    validate_bounded(value, 255, "default_branch")?;
    if value.starts_with('-') || value.contains("..") || value.ends_with('.') {
        return Err(StoreError::InvalidInput("default_branch"));
    }
    Ok(())
}

fn validate_weight(value: f64) -> Result<(), StoreError> {
    if !value.is_finite() || !(0.1..=100.0).contains(&value) {
        return Err(StoreError::InvalidInput("weight"));
    }
    Ok(())
}

fn validate_sha256(value: &str, field: &'static str) -> Result<(), StoreError> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(StoreError::InvalidInput(field));
    }
    Ok(())
}

pub(crate) fn valid_trust_transition(from: &TrustState, to: &TrustState) -> bool {
    matches!(
        (from, to),
        (
            TrustState::Untrusted,
            TrustState::Trusted | TrustState::Revoked
        ) | (
            TrustState::Trusted,
            TrustState::Untrusted | TrustState::Revoked
        ) | (TrustState::Revoked, TrustState::Untrusted)
    ) || from == to
}
