use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::{
    SideEffect, SideEffectAuditContext, SideEffectCommand, SideEffectPhase, SideEffectResultClass,
    StoreError,
    projects::{from_i64, to_i64},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SideEffectAuthorized {
    project_id: Option<String>,
    run_id: String,
    capability: String,
    action_digest: String,
    approval_id: String,
    binding_digest: String,
    audit: SideEffectAuditContext,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SideEffectFinished {
    result_class: SideEffectResultClass,
}

pub(crate) struct SideEffectMutation {
    pub kind: &'static str,
    pub payload_json: String,
    pub side_effect: SideEffect,
}

pub(crate) fn build_mutation(
    action_id: &str,
    command: &SideEffectCommand,
    current: Option<&SideEffect>,
    occurred_at_unix_ms: u64,
) -> Result<SideEffectMutation, StoreError> {
    match command {
        SideEffectCommand::Authorize {
            project_id,
            run_id,
            capability,
            action_digest,
            approval_id,
            binding_digest,
            audit,
        } => {
            if current.is_some() {
                return Err(StoreError::AggregateAlreadyExists(action_id.into()));
            }
            let payload = SideEffectAuthorized {
                project_id: project_id.clone(),
                run_id: run_id.clone(),
                capability: capability.clone(),
                action_digest: action_digest.clone(),
                approval_id: approval_id.clone(),
                binding_digest: binding_digest.clone(),
                audit: audit.as_ref().clone(),
            };
            Ok(SideEffectMutation {
                kind: "side_effect.authorized",
                payload_json: serde_json::to_string(&payload)?,
                side_effect: SideEffect {
                    action_id: action_id.into(),
                    project_id: project_id.clone(),
                    run_id: run_id.clone(),
                    capability: capability.clone(),
                    action_digest: action_digest.clone(),
                    approval_id: approval_id.clone(),
                    binding_digest: binding_digest.clone(),
                    phase: SideEffectPhase::Authorized,
                    revision: 1,
                    result_class: None,
                    authorized_at_unix_ms: occurred_at_unix_ms,
                    started_at_unix_ms: None,
                    finished_at_unix_ms: None,
                    updated_at_unix_ms: occurred_at_unix_ms,
                    last_event_sequence: 0,
                },
            })
        }
        SideEffectCommand::Start => {
            let mut effect = require_current(action_id, current)?.clone();
            if effect.phase != SideEffectPhase::Authorized {
                return Err(StoreError::InvalidTransition);
            }
            effect.phase = SideEffectPhase::Started;
            effect.revision += 1;
            effect.started_at_unix_ms = Some(occurred_at_unix_ms);
            effect.updated_at_unix_ms = occurred_at_unix_ms;
            Ok(SideEffectMutation {
                kind: "side_effect.started",
                payload_json: "{}".into(),
                side_effect: effect,
            })
        }
        SideEffectCommand::Finish { result_class } => {
            let mut effect = require_current(action_id, current)?.clone();
            if effect.phase != SideEffectPhase::Started {
                return Err(StoreError::InvalidTransition);
            }
            effect.phase = SideEffectPhase::Finished;
            effect.revision += 1;
            effect.result_class = Some(*result_class);
            effect.finished_at_unix_ms = Some(occurred_at_unix_ms);
            effect.updated_at_unix_ms = occurred_at_unix_ms;
            Ok(SideEffectMutation {
                kind: "side_effect.finished",
                payload_json: serde_json::to_string(&SideEffectFinished {
                    result_class: *result_class,
                })?,
                side_effect: effect,
            })
        }
        SideEffectCommand::MarkUnknown => {
            let mut effect = require_current(action_id, current)?.clone();
            if effect.phase != SideEffectPhase::Started {
                return Err(StoreError::InvalidTransition);
            }
            effect.phase = SideEffectPhase::Unknown;
            effect.revision += 1;
            effect.updated_at_unix_ms = occurred_at_unix_ms;
            Ok(SideEffectMutation {
                kind: "side_effect.outcome_unknown",
                payload_json: "{}".into(),
                side_effect: effect,
            })
        }
    }
}

pub(crate) fn apply_projection(
    transaction: &Transaction<'_>,
    effect: &SideEffect,
    is_create: bool,
) -> Result<(), StoreError> {
    let changed = if is_create {
        transaction.execute(
            "INSERT INTO side_effects(
                action_id, project_id, run_id, capability, action_digest, approval_id,
                binding_digest, phase, revision, result_class, authorized_at_unix_ms,
                started_at_unix_ms, finished_at_unix_ms, updated_at_unix_ms,
                last_event_sequence
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                effect.action_id,
                effect.project_id,
                effect.run_id,
                effect.capability,
                effect.action_digest,
                effect.approval_id,
                effect.binding_digest,
                effect.phase.as_str(),
                to_i64(effect.revision)?,
                effect
                    .result_class
                    .as_ref()
                    .map(SideEffectResultClass::as_str),
                to_i64(effect.authorized_at_unix_ms)?,
                effect.started_at_unix_ms.map(to_i64).transpose()?,
                effect.finished_at_unix_ms.map(to_i64).transpose()?,
                to_i64(effect.updated_at_unix_ms)?,
                to_i64(effect.last_event_sequence)?,
            ],
        )?
    } else {
        transaction.execute(
            "UPDATE side_effects SET
                phase=?2, revision=?3, result_class=?4, started_at_unix_ms=?5,
                finished_at_unix_ms=?6, updated_at_unix_ms=?7, last_event_sequence=?8
             WHERE action_id=?1 AND revision=?9",
            params![
                effect.action_id,
                effect.phase.as_str(),
                to_i64(effect.revision)?,
                effect
                    .result_class
                    .as_ref()
                    .map(SideEffectResultClass::as_str),
                effect.started_at_unix_ms.map(to_i64).transpose()?,
                effect.finished_at_unix_ms.map(to_i64).transpose()?,
                to_i64(effect.updated_at_unix_ms)?,
                to_i64(effect.last_event_sequence)?,
                to_i64(effect.revision - 1)?,
            ],
        )?
    };
    if changed != 1 {
        return Err(StoreError::IntegrityFailure(
            "side-effect projection write count".into(),
        ));
    }
    Ok(())
}

pub(crate) fn load_side_effect(
    connection: &Connection,
    action_id: &str,
) -> Result<Option<SideEffect>, StoreError> {
    connection
        .query_row(
            "SELECT action_id, project_id, run_id, capability, action_digest, approval_id,
                    binding_digest, phase, revision, result_class, authorized_at_unix_ms,
                    started_at_unix_ms, finished_at_unix_ms, updated_at_unix_ms,
                    last_event_sequence
             FROM side_effects WHERE action_id=?1",
            [action_id],
            side_effect_from_row,
        )
        .optional()
        .map_err(StoreError::from)
}

pub(crate) fn list_side_effects(connection: &Connection) -> Result<Vec<SideEffect>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT action_id, project_id, run_id, capability, action_digest, approval_id,
                binding_digest, phase, revision, result_class, authorized_at_unix_ms,
                started_at_unix_ms, finished_at_unix_ms, updated_at_unix_ms,
                last_event_sequence
         FROM side_effects ORDER BY action_id",
    )?;
    statement
        .query_map([], side_effect_from_row)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

pub(crate) fn started_side_effects(connection: &Connection) -> Result<Vec<SideEffect>, StoreError> {
    Ok(list_side_effects(connection)?
        .into_iter()
        .filter(|effect| effect.phase == SideEffectPhase::Started)
        .collect())
}

pub(crate) fn replay_event(
    current: Option<SideEffect>,
    action_id: &str,
    kind: &str,
    payload_json: &str,
    revision: u64,
    occurred_at_unix_ms: u64,
    sequence: u64,
) -> Result<SideEffect, StoreError> {
    match kind {
        "side_effect.authorized" => {
            if current.is_some() || revision != 1 {
                return Err(StoreError::IntegrityFailure(
                    "invalid side-effect authorization event".into(),
                ));
            }
            let payload: SideEffectAuthorized = serde_json::from_str(payload_json)?;
            payload.audit.validate().map_err(invalid_replayed_effect)?;
            let effect = SideEffect {
                action_id: action_id.into(),
                project_id: payload.project_id,
                run_id: payload.run_id,
                capability: payload.capability,
                action_digest: payload.action_digest,
                approval_id: payload.approval_id,
                binding_digest: payload.binding_digest,
                phase: SideEffectPhase::Authorized,
                revision,
                result_class: None,
                authorized_at_unix_ms: occurred_at_unix_ms,
                started_at_unix_ms: None,
                finished_at_unix_ms: None,
                updated_at_unix_ms: occurred_at_unix_ms,
                last_event_sequence: sequence,
            };
            effect.validate().map_err(invalid_replayed_effect)?;
            Ok(effect)
        }
        _ => {
            let mut effect = current.ok_or_else(|| {
                StoreError::IntegrityFailure("side-effect event without aggregate".into())
            })?;
            if revision != effect.revision + 1 {
                return Err(StoreError::IntegrityFailure(
                    "side-effect revision gap".into(),
                ));
            }
            match kind {
                "side_effect.started" if effect.phase == SideEffectPhase::Authorized => {
                    let _: EmptyPayload = serde_json::from_str(payload_json)?;
                    effect.phase = SideEffectPhase::Started;
                    effect.started_at_unix_ms = Some(occurred_at_unix_ms);
                }
                "side_effect.finished" if effect.phase == SideEffectPhase::Started => {
                    let payload: SideEffectFinished = serde_json::from_str(payload_json)?;
                    effect.phase = SideEffectPhase::Finished;
                    effect.result_class = Some(payload.result_class);
                    effect.finished_at_unix_ms = Some(occurred_at_unix_ms);
                }
                "side_effect.outcome_unknown" if effect.phase == SideEffectPhase::Started => {
                    let _: EmptyPayload = serde_json::from_str(payload_json)?;
                    effect.phase = SideEffectPhase::Unknown;
                }
                _ => {
                    return Err(StoreError::IntegrityFailure(format!(
                        "invalid side-effect event: {kind}"
                    )));
                }
            }
            effect.revision = revision;
            effect.updated_at_unix_ms = occurred_at_unix_ms;
            effect.last_event_sequence = sequence;
            effect.validate().map_err(invalid_replayed_effect)?;
            Ok(effect)
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyPayload {}

fn side_effect_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SideEffect> {
    let phase: String = row.get(7)?;
    let result_class: Option<String> = row.get(9)?;
    Ok(SideEffect {
        action_id: row.get(0)?,
        project_id: row.get(1)?,
        run_id: row.get(2)?,
        capability: row.get(3)?,
        action_digest: row.get(4)?,
        approval_id: row.get(5)?,
        binding_digest: row.get(6)?,
        phase: SideEffectPhase::parse(&phase).map_err(to_sqlite_conversion_error)?,
        revision: from_i64(row.get(8)?).map_err(to_sqlite_conversion_error)?,
        result_class: result_class
            .map(|value| SideEffectResultClass::parse(&value))
            .transpose()
            .map_err(to_sqlite_conversion_error)?,
        authorized_at_unix_ms: from_i64(row.get(10)?).map_err(to_sqlite_conversion_error)?,
        started_at_unix_ms: row
            .get::<_, Option<i64>>(11)?
            .map(from_i64)
            .transpose()
            .map_err(to_sqlite_conversion_error)?,
        finished_at_unix_ms: row
            .get::<_, Option<i64>>(12)?
            .map(from_i64)
            .transpose()
            .map_err(to_sqlite_conversion_error)?,
        updated_at_unix_ms: from_i64(row.get(13)?).map_err(to_sqlite_conversion_error)?,
        last_event_sequence: from_i64(row.get(14)?).map_err(to_sqlite_conversion_error)?,
    })
}

fn require_current<'a>(
    action_id: &str,
    current: Option<&'a SideEffect>,
) -> Result<&'a SideEffect, StoreError> {
    current.ok_or_else(|| StoreError::AggregateNotFound(action_id.into()))
}

fn invalid_replayed_effect(error: StoreError) -> StoreError {
    StoreError::IntegrityFailure(format!("invalid replayed side effect: {error}"))
}

fn to_sqlite_conversion_error(error: StoreError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}
