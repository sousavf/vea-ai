use std::{collections::BTreeSet, net::IpAddr, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::canonical::{ActionDigest, CanonicalError, canonical_json, domain_digest, is_sha256};

pub const ACTION_SCHEMA_VERSION: u16 = 1;
pub const MAX_ACTION_TTL_MS: u64 = 300_000;
pub const MAX_CANONICAL_ACTION_BYTES: usize = 64 * 1024;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_ID_BYTES: usize = 128;
const MAX_STRING_BYTES: usize = 4 * 1024;
const MAX_PATHS: usize = 256;
const MAX_ARGV: usize = 128;
const MAX_PROVENANCE: usize = 32;
const MAX_DATA_CLASSES: usize = 32;
const MAX_PATCH_BYTES: u64 = 16 * 1024 * 1024;
const MAX_PROVIDER_REQUEST_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskClass {
    ReadOnly,
    LocalMutation,
    ProcessExecution,
    ExternalDisclosure,
    CredentialUse,
    GitIntegration,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Reversibility {
    NoMutation,
    WorktreeRecoverable,
    SideEffectUnknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub source_kind: String,
    pub source_id: String,
    pub content_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Destination {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PathCapability {
    Read,
    ApplyPatch,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    ProjectRead,
    ProjectApplyPatch,
    BrokerCommand,
    ProviderSubmit,
    CredentialUse,
    WorktreeIntegrate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CapabilityScope {
    ProjectPaths {
        capability: PathCapability,
        project_id: String,
        run_id: String,
        worktree_id: String,
        paths: Vec<String>,
    },
    BrokerCommand {
        project_id: String,
        run_id: String,
        worktree_id: String,
        catalog_entry_id: String,
    },
    Provider {
        project_id: String,
        run_id: String,
        provider_id: String,
        account_id: String,
        destination: Destination,
    },
    Credential {
        project_id: String,
        run_id: String,
        provider_id: String,
        account_id: String,
        credential_ref: String,
        destination: Destination,
    },
    Integration {
        project_id: String,
        run_id: String,
        worktree_id: String,
        target_ref: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionOperation {
    ReadFiles {
        worktree_id: String,
        paths: Vec<String>,
    },
    ApplyPatch {
        worktree_id: String,
        patch_digest: String,
        patch_bytes: u64,
        affected_paths: Vec<String>,
    },
    RunBrokerCommand {
        worktree_id: String,
        catalog_entry_id: String,
        catalog_entry_digest: String,
        executable_digest: String,
        cwd: String,
        argv: Vec<String>,
    },
    SubmitProviderRequest {
        provider_id: String,
        account_id: String,
        destination: Destination,
        request_digest: String,
        request_bytes: u64,
        data_classes: Vec<String>,
    },
    UseCredential {
        provider_id: String,
        account_id: String,
        credential_ref: String,
        destination: Destination,
    },
    IntegrateWorktree {
        worktree_id: String,
        source_ref: String,
        source_oid: String,
        target_ref: String,
        expected_target_oid: String,
        diff_digest: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionProposal {
    pub schema_version: u16,
    pub action_id: String,
    pub run_id: String,
    pub project_id: String,
    pub requested_capability: CapabilityScope,
    pub operation: ActionOperation,
    pub provenance: Vec<Provenance>,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActionError {
    #[error(transparent)]
    Canonical(#[from] CanonicalError),
    #[error("unsupported action schema version")]
    UnsupportedSchema,
    #[error("required action field is invalid: {0}")]
    InvalidField(&'static str),
    #[error("action collection is invalid: {0}")]
    InvalidCollection(&'static str),
    #[error("action digest field is invalid: {0}")]
    InvalidDigest(&'static str),
    #[error("action capability request does not match its operation")]
    CapabilityMismatch,
    #[error("action lifetime is invalid")]
    InvalidLifetime,
    #[error("canonical action exceeds {MAX_CANONICAL_ACTION_BYTES} bytes")]
    ActionTooLarge,
}

impl ActionProposal {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ActionError> {
        self.validate_shape()?;
        let bytes = canonical_json(self)?;
        if bytes.len() > MAX_CANONICAL_ACTION_BYTES {
            return Err(ActionError::ActionTooLarge);
        }
        Ok(bytes)
    }

    pub fn digest(&self) -> Result<ActionDigest, ActionError> {
        let bytes = self.canonical_bytes()?;
        Ok(ActionDigest::from_raw(domain_digest(
            b"vea\0action\0v1\0",
            &bytes,
        )))
    }

    pub fn risk_class(&self) -> RiskClass {
        match self.operation {
            ActionOperation::ReadFiles { .. } => RiskClass::ReadOnly,
            ActionOperation::ApplyPatch { .. } => RiskClass::LocalMutation,
            ActionOperation::RunBrokerCommand { .. } => RiskClass::ProcessExecution,
            ActionOperation::SubmitProviderRequest { .. } => RiskClass::ExternalDisclosure,
            ActionOperation::UseCredential { .. } => RiskClass::CredentialUse,
            ActionOperation::IntegrateWorktree { .. } => RiskClass::GitIntegration,
        }
    }

    pub fn reversibility(&self) -> Reversibility {
        match self.operation {
            ActionOperation::ReadFiles { .. } => Reversibility::NoMutation,
            ActionOperation::ApplyPatch { .. } | ActionOperation::IntegrateWorktree { .. } => {
                Reversibility::WorktreeRecoverable
            }
            ActionOperation::RunBrokerCommand { .. }
            | ActionOperation::SubmitProviderRequest { .. }
            | ActionOperation::UseCredential { .. } => Reversibility::SideEffectUnknown,
        }
    }

    pub fn validate_shape(&self) -> Result<(), ActionError> {
        if self.schema_version != ACTION_SCHEMA_VERSION {
            return Err(ActionError::UnsupportedSchema);
        }
        validate_id(&self.action_id, "action_id")?;
        validate_id(&self.run_id, "run_id")?;
        validate_id(&self.project_id, "project_id")?;
        if self.created_at_unix_ms > MAX_SAFE_JSON_INTEGER
            || self.expires_at_unix_ms > MAX_SAFE_JSON_INTEGER
            || self.expires_at_unix_ms <= self.created_at_unix_ms
            || self.expires_at_unix_ms - self.created_at_unix_ms > MAX_ACTION_TTL_MS
        {
            return Err(ActionError::InvalidLifetime);
        }
        validate_provenance(&self.provenance)?;
        self.requested_capability.validate()?;
        self.operation.validate()?;
        if !self.requested_capability.matches_action(self) {
            return Err(ActionError::CapabilityMismatch);
        }
        Ok(())
    }
}

impl CapabilityScope {
    pub fn kind(&self) -> CapabilityKind {
        match self {
            Self::ProjectPaths {
                capability: PathCapability::Read,
                ..
            } => CapabilityKind::ProjectRead,
            Self::ProjectPaths {
                capability: PathCapability::ApplyPatch,
                ..
            } => CapabilityKind::ProjectApplyPatch,
            Self::BrokerCommand { .. } => CapabilityKind::BrokerCommand,
            Self::Provider { .. } => CapabilityKind::ProviderSubmit,
            Self::Credential { .. } => CapabilityKind::CredentialUse,
            Self::Integration { .. } => CapabilityKind::WorktreeIntegrate,
        }
    }

    pub fn authorizes(&self, action: &ActionProposal) -> bool {
        self.validate().is_ok() && self.matches_action(action)
    }

    pub(crate) fn validate(&self) -> Result<(), ActionError> {
        match self {
            Self::ProjectPaths {
                project_id,
                run_id,
                worktree_id,
                paths,
                ..
            } => {
                validate_id(project_id, "capability.project_id")?;
                validate_id(run_id, "capability.run_id")?;
                validate_id(worktree_id, "capability.worktree_id")?;
                validate_paths(paths, "capability.paths")
            }
            Self::BrokerCommand {
                project_id,
                run_id,
                worktree_id,
                catalog_entry_id,
            } => {
                validate_id(project_id, "capability.project_id")?;
                validate_id(run_id, "capability.run_id")?;
                validate_id(worktree_id, "capability.worktree_id")?;
                validate_id(catalog_entry_id, "capability.catalog_entry_id")
            }
            Self::Provider {
                project_id,
                run_id,
                provider_id,
                account_id,
                destination,
            } => {
                validate_id(project_id, "capability.project_id")?;
                validate_id(run_id, "capability.run_id")?;
                validate_id(provider_id, "capability.provider_id")?;
                validate_id(account_id, "capability.account_id")?;
                destination.validate()
            }
            Self::Credential {
                project_id,
                run_id,
                provider_id,
                account_id,
                credential_ref,
                destination,
            } => {
                validate_id(project_id, "capability.project_id")?;
                validate_id(run_id, "capability.run_id")?;
                validate_id(provider_id, "capability.provider_id")?;
                validate_id(account_id, "capability.account_id")?;
                validate_id(credential_ref, "capability.credential_ref")?;
                destination.validate()
            }
            Self::Integration {
                project_id,
                run_id,
                worktree_id,
                target_ref,
            } => {
                validate_id(project_id, "capability.project_id")?;
                validate_id(run_id, "capability.run_id")?;
                validate_id(worktree_id, "capability.worktree_id")?;
                validate_ref(target_ref, "capability.target_ref")
            }
        }
    }

    fn matches_action(&self, action: &ActionProposal) -> bool {
        match (self, &action.operation) {
            (
                Self::ProjectPaths {
                    capability,
                    project_id,
                    run_id,
                    worktree_id,
                    paths,
                },
                ActionOperation::ReadFiles {
                    worktree_id: operation_worktree,
                    paths: operation_paths,
                },
            ) => {
                *capability == PathCapability::Read
                    && project_id == &action.project_id
                    && run_id == &action.run_id
                    && worktree_id == operation_worktree
                    && operation_paths.iter().all(|path| paths.contains(path))
            }
            (
                Self::ProjectPaths {
                    capability,
                    project_id,
                    run_id,
                    worktree_id,
                    paths,
                },
                ActionOperation::ApplyPatch {
                    worktree_id: operation_worktree,
                    affected_paths,
                    ..
                },
            ) => {
                *capability == PathCapability::ApplyPatch
                    && project_id == &action.project_id
                    && run_id == &action.run_id
                    && worktree_id == operation_worktree
                    && affected_paths.iter().all(|path| paths.contains(path))
            }
            (
                Self::BrokerCommand {
                    project_id,
                    run_id,
                    worktree_id,
                    catalog_entry_id,
                },
                ActionOperation::RunBrokerCommand {
                    worktree_id: operation_worktree,
                    catalog_entry_id: operation_entry,
                    ..
                },
            ) => {
                project_id == &action.project_id
                    && run_id == &action.run_id
                    && worktree_id == operation_worktree
                    && catalog_entry_id == operation_entry
            }
            (
                Self::Provider {
                    project_id,
                    run_id,
                    provider_id,
                    account_id,
                    destination,
                },
                ActionOperation::SubmitProviderRequest {
                    provider_id: operation_provider,
                    account_id: operation_account,
                    destination: operation_destination,
                    ..
                },
            ) => {
                project_id == &action.project_id
                    && run_id == &action.run_id
                    && provider_id == operation_provider
                    && account_id == operation_account
                    && destination == operation_destination
            }
            (
                Self::Credential {
                    project_id,
                    run_id,
                    provider_id,
                    account_id,
                    credential_ref,
                    destination,
                },
                ActionOperation::UseCredential {
                    provider_id: operation_provider,
                    account_id: operation_account,
                    credential_ref: operation_credential,
                    destination: operation_destination,
                },
            ) => {
                project_id == &action.project_id
                    && run_id == &action.run_id
                    && provider_id == operation_provider
                    && account_id == operation_account
                    && credential_ref == operation_credential
                    && destination == operation_destination
            }
            (
                Self::Integration {
                    project_id,
                    run_id,
                    worktree_id,
                    target_ref,
                },
                ActionOperation::IntegrateWorktree {
                    worktree_id: operation_worktree,
                    target_ref: operation_target,
                    ..
                },
            ) => {
                project_id == &action.project_id
                    && run_id == &action.run_id
                    && worktree_id == operation_worktree
                    && target_ref == operation_target
            }
            _ => false,
        }
    }
}

impl ActionOperation {
    fn validate(&self) -> Result<(), ActionError> {
        match self {
            Self::ReadFiles { worktree_id, paths } => {
                validate_id(worktree_id, "operation.worktree_id")?;
                validate_paths(paths, "operation.paths")
            }
            Self::ApplyPatch {
                worktree_id,
                patch_digest,
                patch_bytes,
                affected_paths,
            } => {
                validate_id(worktree_id, "operation.worktree_id")?;
                validate_digest(patch_digest, "operation.patch_digest")?;
                validate_count(*patch_bytes, MAX_PATCH_BYTES, "operation.patch_bytes")?;
                validate_paths(affected_paths, "operation.affected_paths")
            }
            Self::RunBrokerCommand {
                worktree_id,
                catalog_entry_id,
                catalog_entry_digest,
                executable_digest,
                cwd,
                argv,
            } => {
                validate_id(worktree_id, "operation.worktree_id")?;
                validate_id(catalog_entry_id, "operation.catalog_entry_id")?;
                validate_digest(catalog_entry_digest, "operation.catalog_entry_digest")?;
                validate_digest(executable_digest, "operation.executable_digest")?;
                validate_path(cwd, "operation.cwd")?;
                if argv.len() > MAX_ARGV || argv.iter().any(|value| !is_bounded_string(value)) {
                    return Err(ActionError::InvalidCollection("operation.argv"));
                }
                Ok(())
            }
            Self::SubmitProviderRequest {
                provider_id,
                account_id,
                destination,
                request_digest,
                request_bytes,
                data_classes,
            } => {
                validate_id(provider_id, "operation.provider_id")?;
                validate_id(account_id, "operation.account_id")?;
                destination.validate()?;
                validate_digest(request_digest, "operation.request_digest")?;
                validate_count(
                    *request_bytes,
                    MAX_PROVIDER_REQUEST_BYTES,
                    "operation.request_bytes",
                )?;
                validate_sorted_strings(data_classes, MAX_DATA_CLASSES, "operation.data_classes")
            }
            Self::UseCredential {
                provider_id,
                account_id,
                credential_ref,
                destination,
            } => {
                validate_id(provider_id, "operation.provider_id")?;
                validate_id(account_id, "operation.account_id")?;
                validate_id(credential_ref, "operation.credential_ref")?;
                destination.validate()
            }
            Self::IntegrateWorktree {
                worktree_id,
                source_ref,
                source_oid,
                target_ref,
                expected_target_oid,
                diff_digest,
            } => {
                validate_id(worktree_id, "operation.worktree_id")?;
                validate_ref(source_ref, "operation.source_ref")?;
                validate_oid(source_oid, "operation.source_oid")?;
                validate_ref(target_ref, "operation.target_ref")?;
                validate_oid(expected_target_oid, "operation.expected_target_oid")?;
                validate_digest(diff_digest, "operation.diff_digest")
            }
        }
    }

    pub(crate) fn requires_destination_state(&self) -> bool {
        matches!(
            self,
            Self::SubmitProviderRequest { .. } | Self::UseCredential { .. }
        )
    }
}

impl Destination {
    pub fn validate(&self) -> Result<(), ActionError> {
        if self.scheme != "https" {
            return Err(ActionError::InvalidField("destination.scheme"));
        }
        let valid_labels = self.host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        });
        if self.host.is_empty()
            || self.host.len() > 253
            || !self.host.is_ascii()
            || self.host != self.host.to_ascii_lowercase()
            || self.host.ends_with('.')
            || !self.host.contains('.')
            || !valid_labels
            || IpAddr::from_str(&self.host).is_ok()
        {
            return Err(ActionError::InvalidField("destination.host"));
        }
        if self.port == 0 {
            return Err(ActionError::InvalidField("destination.port"));
        }
        if self.path.is_empty()
            || self.path.len() > MAX_STRING_BYTES
            || !self.path.starts_with('/')
            || !self.path.is_ascii()
            || self.path.contains('?')
            || self.path.contains('#')
            || self.path.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(ActionError::InvalidField("destination.path"));
        }
        Ok(())
    }
}

fn validate_provenance(provenance: &[Provenance]) -> Result<(), ActionError> {
    if provenance.is_empty() || provenance.len() > MAX_PROVENANCE {
        return Err(ActionError::InvalidCollection("provenance"));
    }
    for value in provenance {
        validate_id(&value.source_kind, "provenance.source_kind")?;
        validate_id(&value.source_id, "provenance.source_id")?;
        validate_digest(&value.content_digest, "provenance.content_digest")?;
    }
    Ok(())
}

pub(crate) fn validate_id(value: &str, field: &'static str) -> Result<(), ActionError> {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err(ActionError::InvalidField(field));
    };
    if value.len() > MAX_ID_BYTES
        || !first.is_ascii_alphanumeric()
        || !bytes
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
    {
        return Err(ActionError::InvalidField(field));
    }
    Ok(())
}

fn validate_digest(value: &str, field: &'static str) -> Result<(), ActionError> {
    if !is_sha256(value) {
        return Err(ActionError::InvalidDigest(field));
    }
    Ok(())
}

fn validate_count(value: u64, maximum: u64, field: &'static str) -> Result<(), ActionError> {
    if value == 0 || value > maximum || value > MAX_SAFE_JSON_INTEGER {
        return Err(ActionError::InvalidField(field));
    }
    Ok(())
}

fn validate_ref(value: &str, field: &'static str) -> Result<(), ActionError> {
    if !is_bounded_string(value)
        || value.starts_with('-')
        || value.contains("..")
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(ActionError::InvalidField(field));
    }
    Ok(())
}

fn validate_oid(value: &str, field: &'static str) -> Result<(), ActionError> {
    if !matches!(value.len(), 40 | 64)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ActionError::InvalidField(field));
    }
    Ok(())
}

fn validate_paths(paths: &[String], field: &'static str) -> Result<(), ActionError> {
    if paths.is_empty() || paths.len() > MAX_PATHS {
        return Err(ActionError::InvalidCollection(field));
    }
    let mut seen = BTreeSet::new();
    let mut previous: Option<&str> = None;
    for path in paths {
        validate_path(path, field)?;
        if previous.is_some_and(|value| value >= path.as_str()) || !seen.insert(path) {
            return Err(ActionError::InvalidCollection(field));
        }
        previous = Some(path);
    }
    Ok(())
}

fn validate_path(path: &str, field: &'static str) -> Result<(), ActionError> {
    if !is_bounded_string(path)
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.bytes().any(|byte| byte.is_ascii_control())
        || path
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".." | ".git"))
    {
        return Err(ActionError::InvalidField(field));
    }
    Ok(())
}

fn validate_sorted_strings(
    values: &[String],
    max: usize,
    field: &'static str,
) -> Result<(), ActionError> {
    if values.is_empty() || values.len() > max {
        return Err(ActionError::InvalidCollection(field));
    }
    let mut previous: Option<&str> = None;
    for value in values {
        if !is_bounded_string(value) || previous.is_some_and(|previous| previous >= value) {
            return Err(ActionError::InvalidCollection(field));
        }
        previous = Some(value);
    }
    Ok(())
}

fn is_bounded_string(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_STRING_BYTES && !value.bytes().any(|byte| byte == 0)
}
