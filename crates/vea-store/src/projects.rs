use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::{
    Project, ProjectCommand, StoreError, TrustState,
    model::{CreateProject, valid_trust_transition},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectCreated {
    display_name: String,
    repo_root: String,
    repo_identity: String,
    default_branch: String,
    provider_policy: String,
    data_classification: String,
    weight: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectRenamed {
    display_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectTrustChanged {
    trust_state: TrustState,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectWeightChanged {
    weight: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectPolicyChanged {
    provider_policy: String,
    data_classification: String,
}

pub(crate) struct ProjectMutation {
    pub kind: &'static str,
    pub payload_json: String,
    pub project: Project,
}

pub(crate) fn build_mutation(
    aggregate_id: &str,
    command: &ProjectCommand,
    current: Option<&Project>,
    occurred_at_unix_ms: u64,
) -> Result<ProjectMutation, StoreError> {
    match command {
        ProjectCommand::Create(create) => {
            if current.is_some() {
                return Err(StoreError::AggregateAlreadyExists(aggregate_id.into()));
            }
            let payload = created_payload(create);
            Ok(ProjectMutation {
                kind: "project.created",
                payload_json: serde_json::to_string(&payload)?,
                project: Project {
                    id: aggregate_id.into(),
                    display_name: create.display_name.clone(),
                    repo_root: create.repo_root.clone(),
                    repo_identity: create.repo_identity.clone(),
                    default_branch: create.default_branch.clone(),
                    trust_state: TrustState::Untrusted,
                    weight: create.weight,
                    provider_policy: create.provider_policy.clone(),
                    data_classification: create.data_classification.clone(),
                    revision: 1,
                    created_at_unix_ms: occurred_at_unix_ms,
                    updated_at_unix_ms: occurred_at_unix_ms,
                    last_event_sequence: 0,
                },
            })
        }
        ProjectCommand::Rename { display_name } => {
            let mut project = require_current(aggregate_id, current)?.clone();
            project.display_name = display_name.clone();
            project.revision += 1;
            project.updated_at_unix_ms = occurred_at_unix_ms;
            Ok(ProjectMutation {
                kind: "project.renamed",
                payload_json: serde_json::to_string(&ProjectRenamed {
                    display_name: display_name.clone(),
                })?,
                project,
            })
        }
        ProjectCommand::SetTrust { trust_state } => {
            let mut project = require_current(aggregate_id, current)?.clone();
            if !valid_trust_transition(&project.trust_state, trust_state) {
                return Err(StoreError::InvalidTransition);
            }
            project.trust_state = trust_state.clone();
            project.revision += 1;
            project.updated_at_unix_ms = occurred_at_unix_ms;
            Ok(ProjectMutation {
                kind: "project.trust_changed",
                payload_json: serde_json::to_string(&ProjectTrustChanged {
                    trust_state: trust_state.clone(),
                })?,
                project,
            })
        }
        ProjectCommand::SetSchedulingWeight { weight } => {
            let mut project = require_current(aggregate_id, current)?.clone();
            project.weight = *weight;
            project.revision += 1;
            project.updated_at_unix_ms = occurred_at_unix_ms;
            Ok(ProjectMutation {
                kind: "project.scheduling_weight_changed",
                payload_json: serde_json::to_string(&ProjectWeightChanged { weight: *weight })?,
                project,
            })
        }
        ProjectCommand::SetPolicy {
            provider_policy,
            data_classification,
        } => {
            let mut project = require_current(aggregate_id, current)?.clone();
            project.provider_policy = provider_policy.clone();
            project.data_classification = data_classification.clone();
            project.revision += 1;
            project.updated_at_unix_ms = occurred_at_unix_ms;
            Ok(ProjectMutation {
                kind: "project.policy_changed",
                payload_json: serde_json::to_string(&ProjectPolicyChanged {
                    provider_policy: provider_policy.clone(),
                    data_classification: data_classification.clone(),
                })?,
                project,
            })
        }
    }
}

pub(crate) fn apply_projection(
    transaction: &Transaction<'_>,
    project: &Project,
    is_create: bool,
) -> Result<(), StoreError> {
    let changed = if is_create {
        transaction.execute(
            "INSERT INTO projects(
                id, display_name, repo_root, repo_identity, default_branch, trust_state,
                weight, provider_policy, data_classification, revision, created_at_unix_ms,
                updated_at_unix_ms, last_event_sequence
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                project.id,
                project.display_name,
                project.repo_root,
                project.repo_identity,
                project.default_branch,
                project.trust_state.as_str(),
                project.weight,
                project.provider_policy,
                project.data_classification,
                to_i64(project.revision)?,
                to_i64(project.created_at_unix_ms)?,
                to_i64(project.updated_at_unix_ms)?,
                to_i64(project.last_event_sequence)?,
            ],
        )?
    } else {
        transaction.execute(
            "UPDATE projects SET
                display_name=?2, repo_root=?3, repo_identity=?4, default_branch=?5,
                trust_state=?6, weight=?7, provider_policy=?8, data_classification=?9,
                revision=?10, created_at_unix_ms=?11, updated_at_unix_ms=?12,
                last_event_sequence=?13
             WHERE id=?1 AND revision=?14",
            params![
                project.id,
                project.display_name,
                project.repo_root,
                project.repo_identity,
                project.default_branch,
                project.trust_state.as_str(),
                project.weight,
                project.provider_policy,
                project.data_classification,
                to_i64(project.revision)?,
                to_i64(project.created_at_unix_ms)?,
                to_i64(project.updated_at_unix_ms)?,
                to_i64(project.last_event_sequence)?,
                to_i64(project.revision - 1)?,
            ],
        )?
    };
    if changed != 1 {
        return Err(StoreError::IntegrityFailure(
            "project projection write count".into(),
        ));
    }
    Ok(())
}

pub(crate) fn load_project(
    connection: &Connection,
    id: &str,
) -> Result<Option<Project>, StoreError> {
    connection
        .query_row(
            "SELECT id, display_name, repo_root, repo_identity, default_branch, trust_state,
                    weight, provider_policy, data_classification, revision, created_at_unix_ms,
                    updated_at_unix_ms, last_event_sequence
             FROM projects WHERE id=?1",
            [id],
            project_from_row,
        )
        .optional()
        .map_err(StoreError::from)
}

pub(crate) fn list_projects(connection: &Connection) -> Result<Vec<Project>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT id, display_name, repo_root, repo_identity, default_branch, trust_state,
                weight, provider_policy, data_classification, revision, created_at_unix_ms,
                updated_at_unix_ms, last_event_sequence
         FROM projects ORDER BY created_at_unix_ms, id",
    )?;
    let rows = statement.query_map([], project_from_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

pub(crate) fn replay_event(
    current: Option<Project>,
    aggregate_id: &str,
    kind: &str,
    payload_json: &str,
    revision: u64,
    occurred_at_unix_ms: u64,
    sequence: u64,
) -> Result<Project, StoreError> {
    let mut project = match kind {
        "project.created" => {
            if current.is_some() || revision != 1 {
                return Err(StoreError::IntegrityFailure(
                    "invalid project creation event".into(),
                ));
            }
            let payload: ProjectCreated = serde_json::from_str(payload_json)?;
            Project {
                id: aggregate_id.into(),
                display_name: payload.display_name,
                repo_root: payload.repo_root,
                repo_identity: payload.repo_identity,
                default_branch: payload.default_branch,
                trust_state: TrustState::Untrusted,
                weight: payload.weight,
                provider_policy: payload.provider_policy,
                data_classification: payload.data_classification,
                revision,
                created_at_unix_ms: occurred_at_unix_ms,
                updated_at_unix_ms: occurred_at_unix_ms,
                last_event_sequence: sequence,
            }
        }
        "project.imported" => {
            if current.is_some() {
                return Err(StoreError::IntegrityFailure(
                    "duplicate project import event".into(),
                ));
            }
            let mut project: Project = serde_json::from_str(payload_json)?;
            project.validate().map_err(|error| {
                StoreError::IntegrityFailure(format!("invalid imported project: {error}"))
            })?;
            if project.id != aggregate_id || project.revision != revision {
                return Err(StoreError::IntegrityFailure(
                    "imported project metadata mismatch".into(),
                ));
            }
            project.last_event_sequence = sequence;
            project
        }
        _ => {
            let mut project = current.ok_or_else(|| {
                StoreError::IntegrityFailure("project event without aggregate".into())
            })?;
            if revision != project.revision + 1 {
                return Err(StoreError::IntegrityFailure("project revision gap".into()));
            }
            match kind {
                "project.renamed" => {
                    let payload: ProjectRenamed = serde_json::from_str(payload_json)?;
                    project.display_name = payload.display_name;
                }
                "project.trust_changed" => {
                    let payload: ProjectTrustChanged = serde_json::from_str(payload_json)?;
                    if !valid_trust_transition(&project.trust_state, &payload.trust_state) {
                        return Err(StoreError::IntegrityFailure(
                            "invalid trust transition".into(),
                        ));
                    }
                    project.trust_state = payload.trust_state;
                }
                "project.scheduling_weight_changed" => {
                    let payload: ProjectWeightChanged = serde_json::from_str(payload_json)?;
                    project.weight = payload.weight;
                }
                "project.policy_changed" => {
                    let payload: ProjectPolicyChanged = serde_json::from_str(payload_json)?;
                    project.provider_policy = payload.provider_policy;
                    project.data_classification = payload.data_classification;
                }
                _ => {
                    return Err(StoreError::IntegrityFailure(format!(
                        "unsupported project event: {kind}"
                    )));
                }
            }
            project.revision = revision;
            project.updated_at_unix_ms = occurred_at_unix_ms;
            project.last_event_sequence = sequence;
            project
        }
    };
    project.last_event_sequence = sequence;
    project.validate().map_err(|error| {
        StoreError::IntegrityFailure(format!("invalid replayed project: {error}"))
    })?;
    Ok(project)
}

fn created_payload(project: &CreateProject) -> ProjectCreated {
    ProjectCreated {
        display_name: project.display_name.clone(),
        repo_root: project.repo_root.clone(),
        repo_identity: project.repo_identity.clone(),
        default_branch: project.default_branch.clone(),
        provider_policy: project.provider_policy.clone(),
        data_classification: project.data_classification.clone(),
        weight: project.weight,
    }
}

fn require_current<'a>(
    aggregate_id: &str,
    current: Option<&'a Project>,
) -> Result<&'a Project, StoreError> {
    current.ok_or_else(|| StoreError::AggregateNotFound(aggregate_id.into()))
}

fn project_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    let trust: String = row.get(5)?;
    Ok(Project {
        id: row.get(0)?,
        display_name: row.get(1)?,
        repo_root: row.get(2)?,
        repo_identity: row.get(3)?,
        default_branch: row.get(4)?,
        trust_state: TrustState::parse(&trust).map_err(to_sqlite_conversion_error)?,
        weight: row.get(6)?,
        provider_policy: row.get(7)?,
        data_classification: row.get(8)?,
        revision: from_i64(row.get(9)?).map_err(to_sqlite_conversion_error)?,
        created_at_unix_ms: from_i64(row.get(10)?).map_err(to_sqlite_conversion_error)?,
        updated_at_unix_ms: from_i64(row.get(11)?).map_err(to_sqlite_conversion_error)?,
        last_event_sequence: from_i64(row.get(12)?).map_err(to_sqlite_conversion_error)?,
    })
}

pub(crate) fn to_i64(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::InvalidInput("integer overflow"))
}

pub(crate) fn from_i64(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::IntegrityFailure("negative integer".into()))
}

fn to_sqlite_conversion_error(error: StoreError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}
