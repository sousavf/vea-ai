use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use fs2::FileExt;
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior, params,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    Actor, ActorKind, AuditRecord, AuditResultClass, CommandEnvelope, CommandReceipt, EventRecord,
    OpenReport, PolicyDecision, Project, ProjectCommand, ProjectSnapshot, SideEffect,
    SideEffectCommand, SideEffectResultClass, StoreCommand, StoreError, VerificationReport,
    migrations::{
        apply_migrations, backup_to_path, configure_connection, quick_check,
        set_user_only_permissions, validate_migration_history,
    },
    projects, side_effects,
};

pub struct Store {
    connection: Mutex<Connection>,
    _lock: File,
    database_path: PathBuf,
}

impl Store {
    pub fn open(database_path: impl AsRef<Path>) -> Result<(Self, OpenReport), StoreError> {
        let database_path = database_path.as_ref().to_path_buf();
        let parent = database_path
            .parent()
            .ok_or(StoreError::InvalidInput("database_path"))?;
        let parent_was_missing = !parent.exists();
        fs::create_dir_all(parent)?;
        if parent_was_missing {
            set_user_only_directory_permissions(parent)?;
        }
        let lock_path = database_path.with_extension("lock");
        let lock = open_private_file(&lock_path)?;
        if let Err(error) = lock.try_lock_exclusive() {
            return if error.kind() == std::io::ErrorKind::WouldBlock {
                Err(StoreError::DatabaseBusy)
            } else {
                Err(StoreError::Io(error))
            };
        }

        if !database_path.exists() {
            open_private_file(&database_path)?;
        }
        let existed = fs::metadata(&database_path)?.len() > 0;
        let mut connection = Connection::open(&database_path)?;
        set_user_only_permissions(&database_path)?;
        configure_connection(&connection)?;
        if existed {
            quick_check(&connection)?;
        }
        let migrations = apply_migrations(&mut connection, &database_path)?;
        let store = Self {
            connection: Mutex::new(connection),
            _lock: lock,
            database_path,
        };
        let verification = store.verify_and_repair()?;
        let recovered_side_effects = store.recover_started_side_effects()?;
        let report = OpenReport {
            schema_version: migrations.schema_version,
            applied_migrations: migrations.applied,
            recovered_side_effects,
            verification,
            backup_path: migrations
                .backup_path
                .map(|path| path.to_string_lossy().into_owned()),
        };
        Ok((store, report))
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn execute(&self, envelope: CommandEnvelope) -> Result<CommandReceipt, StoreError> {
        envelope.validate()?;
        if matches!(
            &envelope.command,
            StoreCommand::SideEffect(SideEffectCommand::MarkUnknown)
        ) {
            return Err(StoreError::InvalidInput("startup-only recovery command"));
        }
        let request_sha256: [u8; 32] = Sha256::digest(serde_json::to_vec(&envelope)?).into();
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| StoreError::IntegrityFailure("database mutex poisoned".into()))?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        if let Some((stored_digest, mut receipt)) =
            load_receipt(&transaction, &envelope.command_id)?
        {
            if stored_digest.as_slice() != request_sha256 {
                return Err(StoreError::IdempotencyConflict(envelope.command_id));
            }
            receipt.replayed = true;
            transaction.rollback()?;
            return Ok(receipt);
        }

        let receipt = match &envelope.command {
            StoreCommand::Project(command) => {
                execute_project(&transaction, &envelope, command, &request_sha256)?
            }
            StoreCommand::SideEffect(command) => {
                execute_side_effect(&transaction, &envelope, command, &request_sha256)?
            }
        };
        transaction.commit()?;
        Ok(receipt)
    }

    pub fn get_project(&self, id: &str) -> Result<Option<Project>, StoreError> {
        let connection = self.lock_connection()?;
        projects::load_project(&connection, id)
    }

    pub fn list_projects(&self) -> Result<Vec<Project>, StoreError> {
        let connection = self.lock_connection()?;
        projects::list_projects(&connection)
    }

    pub fn project_snapshot(&self) -> Result<ProjectSnapshot, StoreError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let cursor: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(global_sequence), 0) FROM domain_events",
            [],
            |row| row.get(0),
        )?;
        let projects = projects::list_projects(&transaction)?;
        transaction.commit()?;
        Ok(ProjectSnapshot {
            cursor: projects::from_i64(cursor)?,
            projects,
        })
    }

    pub fn get_side_effect(
        &self,
        action_id: &str,
    ) -> Result<Option<crate::SideEffect>, StoreError> {
        let connection = self.lock_connection()?;
        side_effects::load_side_effect(&connection, action_id)
    }

    pub fn events_after(&self, cursor: u64, limit: u32) -> Result<Vec<EventRecord>, StoreError> {
        if !(1..=1_000).contains(&limit) {
            return Err(StoreError::InvalidInput("event limit"));
        }
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT global_sequence, event_id, aggregate_type, aggregate_id,
                    aggregate_revision, schema_version, kind, payload_json, command_id,
                    causation_event_id, correlation_id, actor_kind, actor_id,
                    occurred_at_unix_ms
             FROM domain_events WHERE global_sequence > ?1
             ORDER BY global_sequence LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![projects::to_i64(cursor)?, i64::from(limit)],
            event_from_row,
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn audit_after(&self, cursor: u64, limit: u32) -> Result<Vec<AuditRecord>, StoreError> {
        if !(1..=1_000).contains(&limit) {
            return Err(StoreError::InvalidInput("audit limit"));
        }
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT audit_sequence, audit_id, occurred_at_unix_ms, actor_kind, actor_id,
                    action, project_id, run_id, action_id, command_id, correlation_id,
                    policy_decision, approval_digest, provider_id, account_alias,
                    affected_paths_json, destination, result_class
             FROM audit_events WHERE audit_sequence > ?1
             ORDER BY audit_sequence LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![projects::to_i64(cursor)?, i64::from(limit)],
            audit_from_row,
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn verify(&self) -> Result<VerificationReport, StoreError> {
        self.verify_internal(false)
    }

    pub fn backup(&self, destination: impl AsRef<Path>) -> Result<(), StoreError> {
        let destination = destination.as_ref();
        validate_backup_destination(&self.database_path, destination)?;
        let parent = destination
            .parent()
            .ok_or(StoreError::InvalidInput("backup destination"))?;
        if !parent.is_dir() {
            return Err(StoreError::InvalidInput("backup destination parent"));
        }
        let file_name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(StoreError::InvalidInput("backup destination"))?;
        let temporary = parent.join(format!(".{file_name}.{}.tmp", Uuid::now_v7()));
        let result = (|| {
            let connection = self.lock_connection()?;
            backup_to_path(&connection, &temporary)?;
            File::open(&temporary)?.sync_all()?;
            fs::hard_link(&temporary, destination)?;
            set_user_only_permissions(destination)?;
            sync_directory(parent)?;
            Ok(())
        })();
        if temporary.exists() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    pub fn restore(
        database_path: impl AsRef<Path>,
        backup_path: impl AsRef<Path>,
    ) -> Result<(), StoreError> {
        let database_path = database_path.as_ref();
        let backup_path = backup_path.as_ref();
        if !backup_path.is_file() || paths_alias(database_path, backup_path)? {
            return Err(StoreError::InvalidInput("restore backup"));
        }
        let parent = database_path
            .parent()
            .ok_or(StoreError::InvalidInput("database_path"))?;
        if !parent.is_dir() {
            return Err(StoreError::InvalidInput("database parent"));
        }
        let lock_path = database_path.with_extension("lock");
        let lock = open_private_file(&lock_path)?;
        if let Err(error) = lock.try_lock_exclusive() {
            return if error.kind() == std::io::ErrorKind::WouldBlock {
                Err(StoreError::DatabaseBusy)
            } else {
                Err(StoreError::Io(error))
            };
        }

        let source = Connection::open_with_flags(backup_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        quick_check(&source)?;
        validate_migration_history(&source)?;

        let file_name = database_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(StoreError::InvalidInput("database_path"))?;
        let restore_id = Uuid::now_v7();
        let temporary = parent.join(format!(".{file_name}.{restore_id}.restore.tmp"));
        backup_to_path(&source, &temporary)?;
        drop(source);

        let temporary_lock = temporary.with_extension("lock");
        let validation_result = (|| {
            let (verification_store, report) = Store::open(&temporary)?;
            verification_store.verify()?;
            drop(verification_store);
            if let Some(path) = report.backup_path {
                remove_if_exists(Path::new(&path))?;
            }
            Ok(())
        })();
        let _ = fs::remove_file(&temporary_lock);
        if let Err(error) = validation_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        File::open(&temporary)?.sync_all()?;

        let rollback = parent.join(format!(".{file_name}.{restore_id}.restore.old"));
        let wal = sqlite_auxiliary_path(database_path, "-wal");
        let shm = sqlite_auxiliary_path(database_path, "-shm");
        let rollback_wal = sqlite_auxiliary_path(&rollback, "-wal");
        let rollback_shm = sqlite_auxiliary_path(&rollback, "-shm");
        let mut moved_database = false;
        let mut moved_wal = false;
        let mut moved_shm = false;
        let install_result = (|| {
            if database_path.exists() {
                fs::rename(database_path, &rollback)?;
                moved_database = true;
            }
            if wal.exists() {
                fs::rename(&wal, &rollback_wal)?;
                moved_wal = true;
            }
            if shm.exists() {
                fs::rename(&shm, &rollback_shm)?;
                moved_shm = true;
            }
            fs::rename(&temporary, database_path)?;
            set_user_only_permissions(database_path)?;
            sync_directory(parent)?;
            let restored =
                Connection::open_with_flags(database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            quick_check(&restored)?;
            validate_migration_history(&restored)?;
            Ok(())
        })();
        if let Err(error) = install_result {
            let _ = fs::remove_file(database_path);
            let rollback_result = rollback_restore(
                database_path,
                &rollback,
                &wal,
                &rollback_wal,
                &shm,
                &rollback_shm,
                moved_database,
                moved_wal,
                moved_shm,
            );
            let _ = fs::remove_file(&temporary);
            if rollback_result.is_err() {
                return Err(StoreError::IntegrityFailure(
                    "restore failed and rollback could not be completed".into(),
                ));
            }
            return Err(error);
        }
        remove_if_exists(&rollback)?;
        remove_if_exists(&rollback_wal)?;
        remove_if_exists(&rollback_shm)?;
        sync_directory(parent)?;
        Ok(())
    }

    fn verify_and_repair(&self) -> Result<VerificationReport, StoreError> {
        self.verify_internal(true)
    }

    fn verify_internal(&self, repair: bool) -> Result<VerificationReport, StoreError> {
        let mut connection = self.lock_connection()?;
        quick_check(&connection)?;
        verify_immutable_ledger(&connection)?;
        let expected = replay_derived_state(&connection)?;
        let actual_projects: BTreeMap<_, _> = projects::list_projects(&connection)?
            .into_iter()
            .map(|project| (project.id.clone(), project))
            .collect();
        let actual_side_effects: BTreeMap<_, _> = side_effects::list_side_effects(&connection)?
            .into_iter()
            .map(|effect| (effect.action_id.clone(), effect))
            .collect();
        let project_cursor = projection_cursor(&connection, "projects")?;
        let side_effect_cursor = projection_cursor(&connection, "side_effects")?;
        let projects_mismatch =
            expected.projects != actual_projects || expected.project_cursor != project_cursor;
        let side_effects_mismatch = expected.side_effects != actual_side_effects
            || expected.side_effect_cursor != side_effect_cursor;
        let mismatch = projects_mismatch || side_effects_mismatch;
        if mismatch && !repair {
            return Err(StoreError::IntegrityFailure(
                "derived projections differ from event stream".into(),
            ));
        }
        if mismatch {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if projects_mismatch {
                transaction.execute("DELETE FROM projects", [])?;
                for project in expected.projects.values() {
                    projects::apply_projection(&transaction, project, true)?;
                }
                set_projection_cursor(&transaction, "projects", expected.project_cursor)?;
            }
            if side_effects_mismatch {
                transaction.execute("DELETE FROM side_effects", [])?;
                for effect in expected.side_effects.values() {
                    side_effects::apply_projection(&transaction, effect, true)?;
                }
                set_projection_cursor(&transaction, "side_effects", expected.side_effect_cursor)?;
            }
            let timestamp = now_ms()?;
            insert_audit(
                &transaction,
                &Actor {
                    kind: ActorKind::System,
                    actor_id: Some("vea-store".into()),
                },
                "store.projection_rebuilt",
                &Uuid::now_v7().to_string(),
                &Uuid::now_v7().to_string(),
                timestamp,
                AuditMetadata::succeeded(),
            )?;
            transaction.commit()?;
        }
        Ok(VerificationReport {
            event_count: expected.event_count,
            project_count: u64::try_from(expected.projects.len())
                .map_err(|_| StoreError::IntegrityFailure("project count overflow".into()))?,
            side_effect_count: u64::try_from(expected.side_effects.len())
                .map_err(|_| StoreError::IntegrityFailure("side-effect count overflow".into()))?,
            projections_rebuilt: mismatch,
        })
    }

    fn recover_started_side_effects(&self) -> Result<u64, StoreError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let effects = side_effects::started_side_effects(&transaction)?;
        let timestamp = now_ms()?;
        for effect in &effects {
            let envelope = CommandEnvelope {
                schema_version: crate::COMMAND_SCHEMA_VERSION,
                command_id: Uuid::now_v7().to_string(),
                aggregate_id: effect.action_id.clone(),
                expected_revision: effect.revision,
                actor: Actor {
                    kind: ActorKind::System,
                    actor_id: Some("startup-recovery".into()),
                },
                correlation_id: Uuid::now_v7().to_string(),
                causation_event_id: None,
                occurred_at_unix_ms: timestamp,
                command: StoreCommand::SideEffect(SideEffectCommand::MarkUnknown),
            };
            envelope.validate()?;
            let request_sha256: [u8; 32] = Sha256::digest(serde_json::to_vec(&envelope)?).into();
            execute_side_effect(
                &transaction,
                &envelope,
                &SideEffectCommand::MarkUnknown,
                &request_sha256,
            )?;
        }
        transaction.commit()?;
        u64::try_from(effects.len())
            .map_err(|_| StoreError::IntegrityFailure("recovery count overflow".into()))
    }

    fn lock_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, StoreError> {
        self.connection
            .lock()
            .map_err(|_| StoreError::IntegrityFailure("database mutex poisoned".into()))
    }
}

fn execute_project(
    transaction: &Transaction<'_>,
    envelope: &CommandEnvelope,
    command: &ProjectCommand,
    request_sha256: &[u8; 32],
) -> Result<CommandReceipt, StoreError> {
    let current = projects::load_project(transaction, &envelope.aggregate_id)?;
    let actual_revision = current.as_ref().map_or(0, |project| project.revision);
    ensure_revision(envelope, actual_revision)?;
    let is_create = current.is_none();
    let mut mutation = projects::build_mutation(
        &envelope.aggregate_id,
        command,
        current.as_ref(),
        envelope.occurred_at_unix_ms,
    )?;
    mutation.project.validate()?;
    let sequence = insert_event(
        transaction,
        envelope,
        mutation.kind,
        &mutation.payload_json,
        mutation.project.revision,
        request_sha256,
    )?;
    mutation.project.last_event_sequence = sequence;
    let receipt = insert_receipt(
        transaction,
        envelope,
        request_sha256,
        mutation.project.revision,
        sequence,
    )?;
    projects::apply_projection(transaction, &mutation.project, is_create)?;
    update_projection_cursor(transaction, "projects", sequence)?;
    insert_audit(
        transaction,
        &envelope.actor,
        envelope.command.kind(),
        &envelope.command_id,
        &envelope.correlation_id,
        envelope.occurred_at_unix_ms,
        AuditMetadata {
            project_id: Some(&envelope.aggregate_id),
            ..AuditMetadata::succeeded()
        },
    )?;
    Ok(receipt)
}

fn execute_side_effect(
    transaction: &Transaction<'_>,
    envelope: &CommandEnvelope,
    command: &SideEffectCommand,
    request_sha256: &[u8; 32],
) -> Result<CommandReceipt, StoreError> {
    let current = side_effects::load_side_effect(transaction, &envelope.aggregate_id)?;
    let actual_revision = current.as_ref().map_or(0, |effect| effect.revision);
    ensure_revision(envelope, actual_revision)?;
    let is_create = current.is_none();
    let mut mutation = side_effects::build_mutation(
        &envelope.aggregate_id,
        command,
        current.as_ref(),
        envelope.occurred_at_unix_ms,
    )?;
    mutation.side_effect.validate()?;
    let sequence = insert_event(
        transaction,
        envelope,
        mutation.kind,
        &mutation.payload_json,
        mutation.side_effect.revision,
        request_sha256,
    )?;
    mutation.side_effect.last_event_sequence = sequence;
    let receipt = insert_receipt(
        transaction,
        envelope,
        request_sha256,
        mutation.side_effect.revision,
        sequence,
    )?;
    side_effects::apply_projection(transaction, &mutation.side_effect, is_create)?;
    update_projection_cursor(transaction, "side_effects", sequence)?;
    let mut audit = AuditMetadata {
        project_id: mutation.side_effect.project_id.as_deref(),
        run_id: Some(&mutation.side_effect.run_id),
        action_id: Some(&mutation.side_effect.action_id),
        ..AuditMetadata::succeeded()
    };
    match command {
        SideEffectCommand::Authorize {
            binding_digest,
            audit: context,
            ..
        } => {
            audit.policy_decision = Some(&context.policy_decision);
            audit.approval_digest = Some(binding_digest);
            audit.provider_id = context.provider_id.as_deref();
            audit.account_alias = context.account_alias.as_deref();
            audit.affected_paths = Some(&context.affected_paths);
            audit.destination = context.destination.as_deref();
            audit.result_class = AuditResultClass::Authorized;
        }
        SideEffectCommand::Start => audit.result_class = AuditResultClass::Started,
        SideEffectCommand::Finish { result_class } => {
            audit.result_class = match result_class {
                SideEffectResultClass::Succeeded => AuditResultClass::Succeeded,
                SideEffectResultClass::Failed => AuditResultClass::Failed,
            };
        }
        SideEffectCommand::MarkUnknown => audit.result_class = AuditResultClass::Unknown,
    }
    insert_audit(
        transaction,
        &envelope.actor,
        envelope.command.kind(),
        &envelope.command_id,
        &envelope.correlation_id,
        envelope.occurred_at_unix_ms,
        audit,
    )?;
    Ok(receipt)
}

fn ensure_revision(envelope: &CommandEnvelope, actual: u64) -> Result<(), StoreError> {
    if envelope.expected_revision != actual {
        return Err(StoreError::RevisionConflict {
            aggregate_id: envelope.aggregate_id.clone(),
            expected: envelope.expected_revision,
            actual,
        });
    }
    Ok(())
}

fn insert_event(
    transaction: &Transaction<'_>,
    envelope: &CommandEnvelope,
    kind: &str,
    payload_json: &str,
    revision: u64,
    request_sha256: &[u8; 32],
) -> Result<u64, StoreError> {
    transaction.execute(
        "INSERT INTO domain_events(
            event_id, aggregate_type, aggregate_id, aggregate_revision, schema_version,
            kind, payload_json, command_id, command_schema_version, command_kind,
            expected_revision, request_sha256, causation_event_id, correlation_id,
            actor_kind, actor_id, occurred_at_unix_ms
         ) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                   ?13, ?14, ?15, ?16)",
        params![
            Uuid::now_v7().to_string(),
            envelope.command.aggregate_type(),
            envelope.aggregate_id,
            projects::to_i64(revision)?,
            kind,
            payload_json,
            envelope.command_id,
            envelope.schema_version,
            envelope.command.kind(),
            projects::to_i64(envelope.expected_revision)?,
            request_sha256.to_vec(),
            envelope.causation_event_id,
            envelope.correlation_id,
            envelope.actor.kind.as_str(),
            envelope.actor.actor_id,
            projects::to_i64(envelope.occurred_at_unix_ms)?,
        ],
    )?;
    projects::from_i64(transaction.last_insert_rowid())
}

fn insert_receipt(
    transaction: &Transaction<'_>,
    envelope: &CommandEnvelope,
    request_sha256: &[u8; 32],
    revision: u64,
    sequence: u64,
) -> Result<CommandReceipt, StoreError> {
    transaction.execute(
        "INSERT INTO command_receipts(
            command_id, command_schema_version, command_kind, aggregate_type,
            aggregate_id, expected_revision, request_sha256, aggregate_revision,
            first_global_sequence, last_global_sequence, actor_kind, actor_id,
            correlation_id, committed_at_unix_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11, ?12, ?13)",
        params![
            envelope.command_id,
            envelope.schema_version,
            envelope.command.kind(),
            envelope.command.aggregate_type(),
            envelope.aggregate_id,
            projects::to_i64(envelope.expected_revision)?,
            request_sha256.to_vec(),
            projects::to_i64(revision)?,
            projects::to_i64(sequence)?,
            envelope.actor.kind.as_str(),
            envelope.actor.actor_id,
            envelope.correlation_id,
            projects::to_i64(envelope.occurred_at_unix_ms)?,
        ],
    )?;
    Ok(CommandReceipt {
        command_id: envelope.command_id.clone(),
        aggregate_id: envelope.aggregate_id.clone(),
        aggregate_revision: revision,
        first_global_sequence: sequence,
        last_global_sequence: sequence,
        replayed: false,
    })
}

fn load_receipt(
    transaction: &Transaction<'_>,
    command_id: &str,
) -> Result<Option<(Vec<u8>, CommandReceipt)>, StoreError> {
    transaction
        .query_row(
            "SELECT request_sha256, aggregate_id, aggregate_revision,
                    first_global_sequence, last_global_sequence
             FROM command_receipts WHERE command_id=?1",
            [command_id],
            |row| {
                Ok((
                    row.get(0)?,
                    CommandReceipt {
                        command_id: command_id.into(),
                        aggregate_id: row.get(1)?,
                        aggregate_revision: projects::from_i64(row.get(2)?)
                            .map_err(sql_conversion)?,
                        first_global_sequence: projects::from_i64(row.get(3)?)
                            .map_err(sql_conversion)?,
                        last_global_sequence: projects::from_i64(row.get(4)?)
                            .map_err(sql_conversion)?,
                        replayed: false,
                    },
                ))
            },
        )
        .optional()
        .map_err(StoreError::from)
}

fn update_projection_cursor(
    transaction: &Transaction<'_>,
    projection: &str,
    sequence: u64,
) -> Result<(), StoreError> {
    let changed = transaction.execute(
        "UPDATE projection_state SET last_global_sequence=?2 WHERE name=?1",
        params![projection, projects::to_i64(sequence)?],
    )?;
    if changed != 1 {
        return Err(StoreError::IntegrityFailure(
            "projection cursor write count".into(),
        ));
    }
    Ok(())
}

fn projection_cursor(connection: &Connection, projection: &str) -> Result<u64, StoreError> {
    let cursor = connection
        .query_row(
            "SELECT last_global_sequence FROM projection_state WHERE name=?1",
            [projection],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .ok_or_else(|| {
            StoreError::IntegrityFailure(format!("missing projection state: {projection}"))
        })?;
    projects::from_i64(cursor)
}

fn set_projection_cursor(
    transaction: &Transaction<'_>,
    projection: &str,
    sequence: u64,
) -> Result<(), StoreError> {
    let changed = transaction.execute(
        "UPDATE projection_state SET last_global_sequence=?2,
            rebuilt_at_unix_ms=?3 WHERE name=?1",
        params![
            projection,
            projects::to_i64(sequence)?,
            projects::to_i64(now_ms()?)?
        ],
    )?;
    if changed != 1 {
        return Err(StoreError::IntegrityFailure(
            "projection rebuild cursor write count".into(),
        ));
    }
    Ok(())
}

struct AuditMetadata<'a> {
    project_id: Option<&'a str>,
    run_id: Option<&'a str>,
    action_id: Option<&'a str>,
    policy_decision: Option<&'a PolicyDecision>,
    approval_digest: Option<&'a str>,
    provider_id: Option<&'a str>,
    account_alias: Option<&'a str>,
    affected_paths: Option<&'a [String]>,
    destination: Option<&'a str>,
    result_class: AuditResultClass,
}

impl AuditMetadata<'_> {
    fn succeeded() -> Self {
        Self {
            project_id: None,
            run_id: None,
            action_id: None,
            policy_decision: None,
            approval_digest: None,
            provider_id: None,
            account_alias: None,
            affected_paths: None,
            destination: None,
            result_class: AuditResultClass::Succeeded,
        }
    }
}

fn insert_audit(
    transaction: &Transaction<'_>,
    actor: &Actor,
    action: &str,
    command_id: &str,
    correlation_id: &str,
    occurred_at_unix_ms: u64,
    metadata: AuditMetadata<'_>,
) -> Result<(), StoreError> {
    let affected_paths_json = metadata
        .affected_paths
        .map(serde_json::to_string)
        .transpose()?;
    transaction.execute(
        "INSERT INTO audit_events(
            audit_id, occurred_at_unix_ms, actor_kind, actor_id, action, project_id,
            run_id, action_id, command_id, correlation_id, policy_decision,
            approval_digest, provider_id, account_alias, affected_paths_json,
            destination, result_class
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                   ?13, ?14, ?15, ?16, ?17)",
        params![
            Uuid::now_v7().to_string(),
            projects::to_i64(occurred_at_unix_ms)?,
            actor.kind.as_str(),
            actor.actor_id,
            action,
            metadata.project_id,
            metadata.run_id,
            metadata.action_id,
            command_id,
            correlation_id,
            metadata.policy_decision.map(PolicyDecision::as_str),
            metadata.approval_digest,
            metadata.provider_id,
            metadata.account_alias,
            affected_paths_json,
            metadata.destination,
            metadata.result_class.as_str(),
        ],
    )?;
    Ok(())
}

fn normalize_sql(sql: &str) -> String {
    sql.chars()
        .filter(|character| !character.is_whitespace() && *character != ';')
        .flat_map(char::to_lowercase)
        .collect()
}

fn verify_immutable_ledger(connection: &Connection) -> Result<(), StoreError> {
    const REQUIRED_TRIGGERS: [(&str, &str); 6] = [
        (
            "domain_events_no_update",
            "CREATE TRIGGER domain_events_no_update BEFORE UPDATE ON domain_events BEGIN
             SELECT RAISE(ABORT, 'domain_events are append-only'); END",
        ),
        (
            "domain_events_no_delete",
            "CREATE TRIGGER domain_events_no_delete BEFORE DELETE ON domain_events BEGIN
             SELECT RAISE(ABORT, 'domain_events are append-only'); END",
        ),
        (
            "command_receipts_no_update",
            "CREATE TRIGGER command_receipts_no_update BEFORE UPDATE ON command_receipts BEGIN
             SELECT RAISE(ABORT, 'command_receipts are immutable'); END",
        ),
        (
            "command_receipts_no_delete",
            "CREATE TRIGGER command_receipts_no_delete BEFORE DELETE ON command_receipts BEGIN
             SELECT RAISE(ABORT, 'command_receipts are immutable'); END",
        ),
        (
            "audit_events_no_update",
            "CREATE TRIGGER audit_events_no_update BEFORE UPDATE ON audit_events BEGIN
             SELECT RAISE(ABORT, 'audit_events cannot be rewritten'); END",
        ),
        (
            "audit_events_no_delete",
            "CREATE TRIGGER audit_events_no_delete BEFORE DELETE ON audit_events BEGIN
             SELECT RAISE(ABORT, 'audit_events cannot be rewritten'); END",
        ),
    ];
    for (trigger, expected_sql) in REQUIRED_TRIGGERS {
        let actual_sql: Option<String> = connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type='trigger' AND name=?1",
                [trigger],
                |row| row.get(0),
            )
            .optional()?;
        if actual_sql
            .as_deref()
            .is_none_or(|sql| normalize_sql(sql) != normalize_sql(expected_sql))
        {
            return Err(StoreError::IntegrityFailure(format!(
                "missing or altered immutable-ledger trigger: {trigger}"
            )));
        }
    }

    let invalid_receipt: Option<String> = connection
        .query_row(
            "SELECT receipt.command_id
             FROM command_receipts AS receipt
             LEFT JOIN domain_events AS event ON event.command_id = receipt.command_id
             GROUP BY receipt.command_id
             HAVING COUNT(event.global_sequence) !=
                        receipt.last_global_sequence - receipt.first_global_sequence + 1
                OR MIN(event.global_sequence) != receipt.first_global_sequence
                OR MAX(event.global_sequence) != receipt.last_global_sequence
                OR MIN(event.aggregate_id) != receipt.aggregate_id
                OR MAX(event.aggregate_id) != receipt.aggregate_id
                OR MIN(event.aggregate_type) != receipt.aggregate_type
                OR MAX(event.aggregate_type) != receipt.aggregate_type
                OR MAX(event.aggregate_revision) != receipt.aggregate_revision
                OR MIN(event.command_schema_version) != receipt.command_schema_version
                OR MAX(event.command_schema_version) != receipt.command_schema_version
                OR MIN(event.command_kind) != receipt.command_kind
                OR MAX(event.command_kind) != receipt.command_kind
                OR MIN(event.expected_revision) != receipt.expected_revision
                OR MAX(event.expected_revision) != receipt.expected_revision
                OR MIN(event.request_sha256) != receipt.request_sha256
                OR MAX(event.request_sha256) != receipt.request_sha256
                OR MIN(event.actor_kind) != receipt.actor_kind
                OR MAX(event.actor_kind) != receipt.actor_kind
                OR MIN(event.actor_id) IS NOT receipt.actor_id
                OR MAX(event.actor_id) IS NOT receipt.actor_id
                OR MIN(event.correlation_id) != receipt.correlation_id
                OR MAX(event.correlation_id) != receipt.correlation_id
                OR MIN(event.occurred_at_unix_ms) != receipt.committed_at_unix_ms
                OR MAX(event.occurred_at_unix_ms) != receipt.committed_at_unix_ms
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(command_id) = invalid_receipt {
        return Err(StoreError::IntegrityFailure(format!(
            "command receipt does not match events: {command_id}"
        )));
    }
    Ok(())
}

struct ReplayedState {
    projects: BTreeMap<String, Project>,
    side_effects: BTreeMap<String, SideEffect>,
    event_count: u64,
    project_cursor: u64,
    side_effect_cursor: u64,
}

fn replay_derived_state(connection: &Connection) -> Result<ReplayedState, StoreError> {
    let mut statement = connection.prepare(
        "SELECT aggregate_type, aggregate_id, schema_version, kind, payload_json,
                aggregate_revision, occurred_at_unix_ms, global_sequence, event_id,
                command_id, causation_event_id, correlation_id, actor_kind, actor_id
         FROM domain_events ORDER BY global_sequence",
    )?;
    let mut rows = statement.query([])?;
    let mut state = ReplayedState {
        projects: BTreeMap::new(),
        side_effects: BTreeMap::new(),
        event_count: 0,
        project_cursor: 0,
        side_effect_cursor: 0,
    };
    while let Some(row) = rows.next()? {
        let aggregate_type: String = row.get(0)?;
        let aggregate_id: String = row.get(1)?;
        let schema_version: u16 = row.get(2)?;
        let kind: String = row.get(3)?;
        let payload_json: String = row.get(4)?;
        let revision = projects::from_i64(row.get(5)?)?;
        let occurred_at_unix_ms = projects::from_i64(row.get(6)?)?;
        let sequence = projects::from_i64(row.get(7)?)?;
        let event_id: String = row.get(8)?;
        let command_id: String = row.get(9)?;
        let causation_event_id: Option<String> = row.get(10)?;
        let correlation_id: String = row.get(11)?;
        let actor_kind: String = row.get(12)?;
        let actor = Actor {
            kind: parse_actor_kind(&actor_kind)?,
            actor_id: row.get(13)?,
        };
        validate_replayed_event_ids(
            &aggregate_id,
            &event_id,
            &command_id,
            causation_event_id.as_deref(),
            &correlation_id,
        )?;
        actor.validate().map_err(|error| {
            StoreError::IntegrityFailure(format!("invalid event actor: {error}"))
        })?;
        if schema_version != 1 || !supported_event(&aggregate_type, &kind) {
            return Err(StoreError::UnsupportedEvent {
                kind,
                schema_version,
            });
        }
        match aggregate_type.as_str() {
            "project" => {
                let current = state.projects.remove(&aggregate_id);
                let project = projects::replay_event(
                    current,
                    &aggregate_id,
                    &kind,
                    &payload_json,
                    revision,
                    occurred_at_unix_ms,
                    sequence,
                )?;
                state.projects.insert(aggregate_id, project);
                state.project_cursor = sequence;
            }
            "side_effect" => {
                let current = state.side_effects.remove(&aggregate_id);
                let effect = side_effects::replay_event(
                    current,
                    &aggregate_id,
                    &kind,
                    &payload_json,
                    revision,
                    occurred_at_unix_ms,
                    sequence,
                )?;
                state.side_effects.insert(aggregate_id, effect);
                state.side_effect_cursor = sequence;
            }
            _ => unreachable!("supported_event rejects unknown aggregate types"),
        }
        state.event_count += 1;
    }
    Ok(state)
}

fn validate_replayed_event_ids(
    aggregate_id: &str,
    event_id: &str,
    command_id: &str,
    causation_event_id: Option<&str>,
    correlation_id: &str,
) -> Result<(), StoreError> {
    for (value, field) in [
        (aggregate_id, "event.aggregate_id"),
        (event_id, "event.event_id"),
        (command_id, "event.command_id"),
        (correlation_id, "event.correlation_id"),
    ] {
        crate::model::validate_uuid_v7(value, field).map_err(|error| {
            StoreError::IntegrityFailure(format!("invalid event metadata: {error}"))
        })?;
    }
    if let Some(causation_event_id) = causation_event_id {
        crate::model::validate_uuid_v7(causation_event_id, "event.causation_event_id").map_err(
            |error| StoreError::IntegrityFailure(format!("invalid event metadata: {error}")),
        )?;
    }
    Ok(())
}

fn supported_event(aggregate_type: &str, kind: &str) -> bool {
    match aggregate_type {
        "project" => matches!(
            kind,
            "project.created"
                | "project.imported"
                | "project.renamed"
                | "project.trust_changed"
                | "project.scheduling_weight_changed"
                | "project.policy_changed"
        ),
        "side_effect" => matches!(
            kind,
            "side_effect.authorized"
                | "side_effect.started"
                | "side_effect.finished"
                | "side_effect.outcome_unknown"
        ),
        _ => false,
    }
}

fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRecord> {
    let actor_kind: String = row.get(11)?;
    let payload_json: String = row.get(7)?;
    Ok(EventRecord {
        global_sequence: projects::from_i64(row.get(0)?).map_err(sql_conversion)?,
        event_id: row.get(1)?,
        aggregate_type: row.get(2)?,
        aggregate_id: row.get(3)?,
        aggregate_revision: projects::from_i64(row.get(4)?).map_err(sql_conversion)?,
        schema_version: row.get(5)?,
        kind: row.get(6)?,
        payload: serde_json::from_str(&payload_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                7,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        command_id: row.get(8)?,
        causation_event_id: row.get(9)?,
        correlation_id: row.get(10)?,
        actor: Actor {
            kind: parse_actor_kind(&actor_kind).map_err(sql_conversion)?,
            actor_id: row.get(12)?,
        },
        occurred_at_unix_ms: projects::from_i64(row.get(13)?).map_err(sql_conversion)?,
    })
}

fn audit_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuditRecord> {
    let actor_kind: String = row.get(3)?;
    let policy_decision: Option<String> = row.get(11)?;
    let result_class: String = row.get(17)?;
    let affected_paths_json: Option<String> = row.get(15)?;
    let affected_paths = affected_paths_json
        .map(|json| serde_json::from_str(&json))
        .transpose()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                15,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    Ok(AuditRecord {
        audit_sequence: projects::from_i64(row.get(0)?).map_err(sql_conversion)?,
        audit_id: row.get(1)?,
        occurred_at_unix_ms: projects::from_i64(row.get(2)?).map_err(sql_conversion)?,
        actor: Actor {
            kind: parse_actor_kind(&actor_kind).map_err(sql_conversion)?,
            actor_id: row.get(4)?,
        },
        action: row.get(5)?,
        project_id: row.get(6)?,
        run_id: row.get(7)?,
        action_id: row.get(8)?,
        command_id: row.get(9)?,
        correlation_id: row.get(10)?,
        policy_decision: policy_decision
            .map(|value| PolicyDecision::parse(&value))
            .transpose()
            .map_err(sql_conversion)?,
        approval_digest: row.get(12)?,
        provider_id: row.get(13)?,
        account_alias: row.get(14)?,
        affected_paths,
        destination: row.get(16)?,
        result_class: AuditResultClass::parse(&result_class).map_err(sql_conversion)?,
    })
}

fn parse_actor_kind(value: &str) -> Result<ActorKind, StoreError> {
    match value {
        "user" => Ok(ActorKind::User),
        "system" => Ok(ActorKind::System),
        _ => Err(StoreError::IntegrityFailure("invalid actor kind".into())),
    }
}

fn sql_conversion(error: StoreError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

#[allow(clippy::too_many_arguments)]
fn rollback_restore(
    database_path: &Path,
    rollback: &Path,
    wal: &Path,
    rollback_wal: &Path,
    shm: &Path,
    rollback_shm: &Path,
    moved_database: bool,
    moved_wal: bool,
    moved_shm: bool,
) -> Result<(), StoreError> {
    if moved_shm {
        fs::rename(rollback_shm, shm)?;
    }
    if moved_wal {
        fs::rename(rollback_wal, wal)?;
    }
    if moved_database {
        fs::rename(rollback, database_path)?;
    }
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<(), StoreError> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn sqlite_auxiliary_path(database_path: &Path, suffix: &str) -> PathBuf {
    let mut name = database_path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn paths_alias(left: &Path, right: &Path) -> Result<bool, StoreError> {
    Ok(
        normalized_absolute_path(left)? == normalized_absolute_path(right)?
            || (left.exists() && right.exists() && same_file::is_same_file(left, right)?),
    )
}

fn validate_backup_destination(database_path: &Path, destination: &Path) -> Result<(), StoreError> {
    let lock_path = database_path.with_extension("lock");
    let mut wal_name = database_path.as_os_str().to_os_string();
    wal_name.push("-wal");
    let mut shm_name = database_path.as_os_str().to_os_string();
    shm_name.push("-shm");
    let forbidden = [
        database_path.to_path_buf(),
        lock_path,
        PathBuf::from(wal_name),
        PathBuf::from(shm_name),
    ];
    let normalized_destination = normalized_absolute_path(destination)?;
    for path in &forbidden {
        if normalized_destination == normalized_absolute_path(path)?
            || (destination.exists()
                && path.exists()
                && same_file::is_same_file(destination, path)?)
        {
            return Err(StoreError::InvalidInput(
                "backup destination aliases store files",
            ));
        }
    }
    if destination.exists() {
        return Err(StoreError::InvalidInput("backup destination exists"));
    }
    Ok(())
}

fn normalized_absolute_path(path: &Path) -> Result<PathBuf, StoreError> {
    if path.exists() {
        return Ok(fs::canonicalize(path)?);
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    Ok(normalized)
}

fn sync_directory(_path: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    File::open(_path)?.sync_all()?;
    Ok(())
}

fn open_private_file(path: &Path) -> Result<File, StoreError> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}

fn set_user_only_directory_permissions(_path: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(_path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn now_ms() -> Result<u64, StoreError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StoreError::InvalidInput("system clock"))?;
    u64::try_from(duration.as_millis()).map_err(|_| StoreError::InvalidInput("system clock"))
}
