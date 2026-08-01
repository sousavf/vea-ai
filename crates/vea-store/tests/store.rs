use std::path::Path;

use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use uuid::Uuid;
use vea_store::{
    Actor, ActorKind, AuditResultClass, COMMAND_SCHEMA_VERSION, CommandEnvelope, CreateProject,
    PolicyDecision, ProjectCommand, SideEffectAuditContext, SideEffectCommand, SideEffectPhase,
    SideEffectResultClass, Store, StoreCommand, StoreError, TrustState,
};

fn database_path(directory: &TempDir) -> std::path::PathBuf {
    directory.path().join("vea.sqlite")
}

fn actor() -> Actor {
    Actor {
        kind: ActorKind::User,
        actor_id: Some("user-1".into()),
    }
}

fn envelope(
    aggregate_id: &str,
    expected_revision: u64,
    command: StoreCommand,
    occurred_at_unix_ms: u64,
) -> CommandEnvelope {
    CommandEnvelope {
        schema_version: COMMAND_SCHEMA_VERSION,
        command_id: Uuid::now_v7().to_string(),
        aggregate_id: aggregate_id.into(),
        expected_revision,
        actor: actor(),
        correlation_id: Uuid::now_v7().to_string(),
        causation_event_id: None,
        occurred_at_unix_ms,
        command,
    }
}

fn side_effect_audit_context() -> SideEffectAuditContext {
    SideEffectAuditContext {
        policy_decision: PolicyDecision::Approved,
        provider_id: Some("provider.test".into()),
        account_alias: Some("primary".into()),
        affected_paths: vec!["/repos/vea/src/main.rs".into()],
        destination: Some("https://api.example.test".into()),
    }
}

fn create_project() -> StoreCommand {
    StoreCommand::Project(ProjectCommand::Create(CreateProject {
        display_name: "Vea".into(),
        repo_root: "/repos/vea".into(),
        repo_identity: "github.com/sousavf/vea-ai".into(),
        default_branch: "main".into(),
        provider_policy: "local-first".into(),
        data_classification: "source".into(),
        weight: 1.0,
    }))
}

#[test]
fn fresh_store_commits_events_receipts_audit_and_projections_atomically() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let (store, report) = Store::open(&path).unwrap();
    assert_eq!(report.schema_version, 2);
    assert_eq!(report.applied_migrations, vec![1, 2]);
    assert_eq!(report.verification.event_count, 0);

    let project_id = Uuid::now_v7().to_string();
    let create = envelope(&project_id, 0, create_project(), 1_000);
    let receipt = store.execute(create.clone()).unwrap();
    assert_eq!(receipt.aggregate_revision, 1);
    assert!(!receipt.replayed);

    let replayed = store.execute(create.clone()).unwrap();
    assert!(replayed.replayed);
    assert_eq!(
        replayed.first_global_sequence,
        receipt.first_global_sequence
    );
    assert_eq!(store.events_after(0, 100).unwrap().len(), 1);
    assert_eq!(store.audit_after(0, 100).unwrap().len(), 1);

    let mut conflicting_retry = create;
    conflicting_retry.occurred_at_unix_ms += 1;
    assert!(matches!(
        store.execute(conflicting_retry),
        Err(StoreError::IdempotencyConflict(_))
    ));

    let rename = envelope(
        &project_id,
        1,
        StoreCommand::Project(ProjectCommand::Rename {
            display_name: "Vea AI".into(),
        }),
        2_000,
    );
    store.execute(rename).unwrap();
    let project = store.get_project(&project_id).unwrap().unwrap();
    assert_eq!(project.display_name, "Vea AI");
    assert_eq!(project.revision, 2);

    let stale = envelope(
        &project_id,
        1,
        StoreCommand::Project(ProjectCommand::SetSchedulingWeight { weight: 2.0 }),
        3_000,
    );
    assert!(matches!(
        store.execute(stale),
        Err(StoreError::RevisionConflict {
            expected: 1,
            actual: 2,
            ..
        })
    ));
    assert_eq!(store.events_after(0, 100).unwrap().len(), 2);
    assert_eq!(store.audit_after(0, 100).unwrap().len(), 2);

    let snapshot = store.project_snapshot().unwrap();
    assert_eq!(snapshot.projects.len(), 1);
    assert!(snapshot.cursor >= snapshot.projects[0].last_event_sequence);
    drop(store);

    let (reopened, reopen_report) = Store::open(&path).unwrap();
    assert!(reopen_report.applied_migrations.is_empty());
    assert_eq!(
        reopened.get_project(&project_id).unwrap().unwrap().revision,
        2
    );
}

#[test]
fn audit_records_do_not_capture_project_content() {
    const CANARY: &str = "SUPER_SECRET_SOURCE_CANARY";
    let directory = TempDir::new().unwrap();
    let (store, _) = Store::open(database_path(&directory)).unwrap();
    let project_id = Uuid::now_v7().to_string();
    let command = StoreCommand::Project(ProjectCommand::Create(CreateProject {
        display_name: CANARY.into(),
        repo_root: "/repos/canary".into(),
        repo_identity: CANARY.into(),
        default_branch: "main".into(),
        provider_policy: "local-first".into(),
        data_classification: "source".into(),
        weight: 1.0,
    }));
    store
        .execute(envelope(&project_id, 0, command, 1_000))
        .unwrap();
    let serialized = serde_json::to_string(&store.audit_after(0, 100).unwrap()).unwrap();
    assert!(!serialized.contains(CANARY));
}

#[test]
fn late_projection_failure_rolls_back_event_receipt_and_audit() {
    let directory = TempDir::new().unwrap();
    let (store, _) = Store::open(database_path(&directory)).unwrap();
    let first_id = Uuid::now_v7().to_string();
    store
        .execute(envelope(&first_id, 0, create_project(), 1_000))
        .unwrap();

    let second_id = Uuid::now_v7().to_string();
    let duplicate_root = envelope(&second_id, 0, create_project(), 2_000);
    assert!(matches!(
        store.execute(duplicate_root),
        Err(StoreError::Sqlite(_))
    ));
    assert!(store.get_project(&second_id).unwrap().is_none());
    assert_eq!(store.events_after(0, 100).unwrap().len(), 1);
    assert_eq!(store.audit_after(0, 100).unwrap().len(), 1);
    assert_eq!(store.verify().unwrap().event_count, 1);
}

#[test]
fn event_and_audit_tables_reject_mutation() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let (store, _) = Store::open(&path).unwrap();
    let project_id = Uuid::now_v7().to_string();
    store
        .execute(envelope(&project_id, 0, create_project(), 1_000))
        .unwrap();

    let connection = Connection::open(&path).unwrap();
    assert!(
        connection
            .execute("UPDATE domain_events SET kind='tampered'", [])
            .is_err()
    );
    assert!(connection.execute("DELETE FROM domain_events", []).is_err());
    assert!(
        connection
            .execute("UPDATE audit_events SET action='tampered'", [])
            .is_err()
    );
    assert!(connection.execute("DELETE FROM audit_events", []).is_err());
    assert!(
        connection
            .execute("UPDATE command_receipts SET expected_revision=99", [])
            .is_err()
    );
    assert!(
        connection
            .execute("DELETE FROM command_receipts", [])
            .is_err()
    );
}

#[test]
fn receipt_metadata_corruption_fails_verification_even_if_trigger_is_restored() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    {
        let (store, _) = Store::open(&path).unwrap();
        store
            .execute(envelope(
                &Uuid::now_v7().to_string(),
                0,
                create_project(),
                1_000,
            ))
            .unwrap();
    }
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "DROP TRIGGER command_receipts_no_update;
             UPDATE command_receipts
             SET expected_revision=42, request_sha256=zeroblob(32);
             CREATE TRIGGER command_receipts_no_update
             BEFORE UPDATE ON command_receipts BEGIN
               SELECT RAISE(ABORT, 'command_receipts are immutable');
             END;",
        )
        .unwrap();
    drop(connection);
    assert!(matches!(
        Store::open(&path),
        Err(StoreError::IntegrityFailure(message))
            if message.contains("command receipt does not match events")
    ));
}

#[test]
fn projection_is_rebuilt_from_events_after_tampering() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let project_id = Uuid::now_v7().to_string();
    {
        let (store, _) = Store::open(&path).unwrap();
        store
            .execute(envelope(&project_id, 0, create_project(), 1_000))
            .unwrap();
    }
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE projects SET display_name='tampered' WHERE id=?1",
            [&project_id],
        )
        .unwrap();
    drop(connection);

    let (store, report) = Store::open(&path).unwrap();
    assert!(report.verification.projections_rebuilt);
    assert_eq!(
        store
            .get_project(&project_id)
            .unwrap()
            .unwrap()
            .display_name,
        "Vea"
    );
    assert_eq!(
        store.audit_after(0, 100).unwrap().last().unwrap().action,
        "store.projection_rebuilt"
    );
}

#[test]
fn started_side_effect_is_marked_unknown_during_recovery() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let action_id = Uuid::now_v7().to_string();
    let project_id = Uuid::now_v7().to_string();
    let run_id = Uuid::now_v7().to_string();
    {
        let (store, _) = Store::open(&path).unwrap();
        store
            .execute(envelope(
                &action_id,
                0,
                StoreCommand::SideEffect(SideEffectCommand::Authorize {
                    project_id: Some(project_id),
                    run_id,
                    capability: "process.spawn".into(),
                    action_digest: format!("sha256:{}", "a".repeat(64)),
                    approval_id: Uuid::now_v7().to_string(),
                    binding_digest: format!("sha256:{}", "b".repeat(64)),
                    audit: Box::new(side_effect_audit_context()),
                }),
                1_000,
            ))
            .unwrap();
        store
            .execute(envelope(
                &action_id,
                1,
                StoreCommand::SideEffect(SideEffectCommand::Start),
                2_000,
            ))
            .unwrap();
        assert_eq!(
            store.get_side_effect(&action_id).unwrap().unwrap().phase,
            SideEffectPhase::Started
        );
        assert!(matches!(
            store.execute(envelope(
                &action_id,
                2,
                StoreCommand::SideEffect(SideEffectCommand::MarkUnknown),
                2_500,
            )),
            Err(StoreError::InvalidInput("startup-only recovery command"))
        ));
    }

    let (store, report) = Store::open(&path).unwrap();
    assert_eq!(report.recovered_side_effects, 1);
    let effect = store.get_side_effect(&action_id).unwrap().unwrap();
    assert_eq!(effect.phase, SideEffectPhase::Unknown);
    assert_eq!(effect.revision, 3);
    let audit = store.audit_after(0, 100).unwrap();
    assert_eq!(audit[0].policy_decision, Some(PolicyDecision::Approved));
    assert_eq!(
        audit[0].approval_digest.as_deref(),
        Some(format!("sha256:{}", "b".repeat(64)).as_str())
    );
    assert_eq!(audit[0].provider_id.as_deref(), Some("provider.test"));
    assert_eq!(audit[0].account_alias.as_deref(), Some("primary"));
    assert_eq!(
        audit[0].affected_paths.as_deref(),
        Some(["/repos/vea/src/main.rs".into()].as_slice())
    );
    assert_eq!(
        audit[0].destination.as_deref(),
        Some("https://api.example.test")
    );
    assert_eq!(audit[0].result_class, AuditResultClass::Authorized);
    assert_eq!(audit[1].result_class, AuditResultClass::Started);
    assert_eq!(audit[2].result_class, AuditResultClass::Unknown);
    drop(store);

    let (store, second_report) = Store::open(&path).unwrap();
    assert_eq!(second_report.recovered_side_effects, 0);
    assert_eq!(store.events_after(0, 100).unwrap().len(), 3);
}

#[test]
fn side_effect_projection_and_cursor_are_rebuilt_from_events() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let action_id = Uuid::now_v7().to_string();
    {
        let (store, _) = Store::open(&path).unwrap();
        store
            .execute(envelope(
                &action_id,
                0,
                StoreCommand::SideEffect(SideEffectCommand::Authorize {
                    project_id: None,
                    run_id: Uuid::now_v7().to_string(),
                    capability: "fs.write".into(),
                    action_digest: format!("sha256:{}", "c".repeat(64)),
                    approval_id: Uuid::now_v7().to_string(),
                    binding_digest: format!("sha256:{}", "d".repeat(64)),
                    audit: Box::new(side_effect_audit_context()),
                }),
                1_000,
            ))
            .unwrap();
        store
            .execute(envelope(
                &action_id,
                1,
                StoreCommand::SideEffect(SideEffectCommand::Start),
                2_000,
            ))
            .unwrap();
        store
            .execute(envelope(
                &action_id,
                2,
                StoreCommand::SideEffect(SideEffectCommand::Finish {
                    result_class: SideEffectResultClass::Failed,
                }),
                3_000,
            ))
            .unwrap();
    }
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE side_effects SET phase='authorized', revision=1, result_class=NULL,
                started_at_unix_ms=NULL, finished_at_unix_ms=NULL,
                updated_at_unix_ms=authorized_at_unix_ms, last_event_sequence=1
             WHERE action_id=?1",
            [&action_id],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE projection_state SET last_global_sequence=0 WHERE name='side_effects'",
            [],
        )
        .unwrap();
    drop(connection);

    let (store, report) = Store::open(&path).unwrap();
    assert!(report.verification.projections_rebuilt);
    assert_eq!(report.verification.side_effect_count, 1);
    let effect = store.get_side_effect(&action_id).unwrap().unwrap();
    assert_eq!(effect.phase, SideEffectPhase::Finished);
    assert_eq!(effect.revision, 3);
    let audit = store.audit_after(0, 100).unwrap();
    let finish = audit
        .iter()
        .find(|record| record.action == "side_effect.finish")
        .unwrap();
    assert_eq!(finish.result_class, AuditResultClass::Failed);
}

#[test]
fn repo_identity_validation_matches_the_persisted_512_character_contract() {
    let directory = TempDir::new().unwrap();
    let (store, _) = Store::open(database_path(&directory)).unwrap();
    for (index, repo_identity) in ["x".repeat(512), "é".repeat(200)].into_iter().enumerate() {
        let project_id = Uuid::now_v7().to_string();
        let command = StoreCommand::Project(ProjectCommand::Create(CreateProject {
            display_name: format!("Project {index}"),
            repo_root: format!("/repos/project-{index}"),
            repo_identity: repo_identity.clone(),
            default_branch: "main".into(),
            provider_policy: "local-first".into(),
            data_classification: "source".into(),
            weight: 1.0,
        }));
        store
            .execute(envelope(&project_id, 0, command, 1_000 + index as u64))
            .unwrap();
        assert_eq!(
            store
                .get_project(&project_id)
                .unwrap()
                .unwrap()
                .repo_identity,
            repo_identity
        );
    }
}

#[test]
fn trust_lifecycle_is_enforced() {
    let directory = TempDir::new().unwrap();
    let (store, _) = Store::open(database_path(&directory)).unwrap();
    let project_id = Uuid::now_v7().to_string();
    store
        .execute(envelope(&project_id, 0, create_project(), 1_000))
        .unwrap();

    for (revision, trust) in [(1, TrustState::Trusted), (2, TrustState::Revoked)] {
        store
            .execute(envelope(
                &project_id,
                revision,
                StoreCommand::Project(ProjectCommand::SetTrust { trust_state: trust }),
                2_000 + revision,
            ))
            .unwrap();
    }
    let invalid = envelope(
        &project_id,
        3,
        StoreCommand::Project(ProjectCommand::SetTrust {
            trust_state: TrustState::Trusted,
        }),
        3_000,
    );
    assert!(matches!(
        store.execute(invalid),
        Err(StoreError::InvalidTransition)
    ));
    store
        .execute(envelope(
            &project_id,
            3,
            StoreCommand::Project(ProjectCommand::SetTrust {
                trust_state: TrustState::Untrusted,
            }),
            4_000,
        ))
        .unwrap();
    assert_eq!(
        store.get_project(&project_id).unwrap().unwrap().trust_state,
        TrustState::Untrusted
    );
}

#[test]
fn concurrent_commands_with_one_revision_have_one_winner() {
    let directory = TempDir::new().unwrap();
    let (store, _) = Store::open(database_path(&directory)).unwrap();
    let store = std::sync::Arc::new(store);
    let project_id = Uuid::now_v7().to_string();
    store
        .execute(envelope(&project_id, 0, create_project(), 1_000))
        .unwrap();

    let handles: Vec<_> = ["First", "Second"]
        .into_iter()
        .map(|name| {
            let store = std::sync::Arc::clone(&store);
            let project_id = project_id.clone();
            std::thread::spawn(move || {
                store.execute(envelope(
                    &project_id,
                    1,
                    StoreCommand::Project(ProjectCommand::Rename {
                        display_name: name.into(),
                    }),
                    2_000,
                ))
            })
        })
        .collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(StoreError::RevisionConflict { .. })))
            .count(),
        1
    );
    assert_eq!(store.events_after(0, 100).unwrap().len(), 2);
}

#[test]
fn exclusive_process_lock_and_online_backup_work() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let (store, _) = Store::open(&path).unwrap();
    assert!(matches!(Store::open(&path), Err(StoreError::DatabaseBusy)));
    assert!(matches!(
        store.backup(&path),
        Err(StoreError::InvalidInput(
            "backup destination aliases store files"
        ))
    ));
    let project_id = Uuid::now_v7().to_string();
    store
        .execute(envelope(&project_id, 0, create_project(), 1_000))
        .unwrap();

    #[cfg(unix)]
    {
        let alias = directory.path().join("database-hard-link.sqlite");
        std::fs::hard_link(&path, &alias).unwrap();
        assert!(matches!(
            store.backup(&alias),
            Err(StoreError::InvalidInput(
                "backup destination aliases store files"
            ))
        ));
        std::fs::remove_file(alias).unwrap();
    }

    let backup = directory.path().join("manual-backup.sqlite");
    store.backup(&backup).unwrap();
    let connection = Connection::open(backup).unwrap();
    let result: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(result, "ok");
}

#[test]
fn verified_restore_requires_closed_store_and_recovers_backup_state() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let backup = directory.path().join("restore-source.sqlite");
    let project_id = Uuid::now_v7().to_string();
    let (store, _) = Store::open(&path).unwrap();
    store
        .execute(envelope(&project_id, 0, create_project(), 1_000))
        .unwrap();
    store.backup(&backup).unwrap();
    store
        .execute(envelope(
            &project_id,
            1,
            StoreCommand::Project(ProjectCommand::Rename {
                display_name: "Changed after backup".into(),
            }),
            2_000,
        ))
        .unwrap();
    assert!(matches!(
        Store::restore(&path, &backup),
        Err(StoreError::DatabaseBusy)
    ));
    drop(store);

    Store::restore(&path, &backup).unwrap();
    let (store, _) = Store::open(&path).unwrap();
    assert_eq!(
        store
            .get_project(&project_id)
            .unwrap()
            .unwrap()
            .display_name,
        "Vea"
    );
    assert_eq!(store.events_after(0, 100).unwrap().len(), 1);
}

#[test]
fn restore_rejects_history_only_database_and_preserves_primary() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let project_id = Uuid::now_v7().to_string();
    {
        let (store, _) = Store::open(&path).unwrap();
        store
            .execute(envelope(&project_id, 0, create_project(), 1_000))
            .unwrap();
    }

    let bogus = directory.path().join("history-only.sqlite");
    let connection = Connection::open(&bogus).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                sha256 BLOB NOT NULL,
                applied_at_unix_ms INTEGER NOT NULL
            ) STRICT;",
        )
        .unwrap();
    for (version, name, sql) in [
        (1, "core", include_str!("../../../migrations/0001_core.sql")),
        (
            2,
            "events_audit",
            include_str!("../../../migrations/0002_events_audit.sql"),
        ),
    ] {
        let digest: [u8; 32] = Sha256::digest(sql.as_bytes()).into();
        connection
            .execute(
                "INSERT INTO schema_migrations(version, name, sha256, applied_at_unix_ms)
                 VALUES (?1, ?2, ?3, 1)",
                params![version, name, digest.to_vec()],
            )
            .unwrap();
    }
    connection.pragma_update(None, "user_version", 2).unwrap();
    drop(connection);

    assert!(matches!(
        Store::restore(&path, &bogus),
        Err(StoreError::IntegrityFailure(_))
    ));
    let (store, _) = Store::open(&path).unwrap();
    assert!(store.get_project(&project_id).unwrap().is_some());
}

#[test]
fn upgrade_creates_verified_pre_migration_backup() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    create_version_one_database(&path);

    let (store, report) = Store::open(&path).unwrap();
    assert_eq!(report.applied_migrations, vec![2]);
    assert_eq!(report.verification.project_count, 1);
    assert_eq!(store.list_projects().unwrap()[0].display_name, "Legacy");
    assert_eq!(store.list_projects().unwrap()[0].repo_identity.len(), 129);
    let backup = report.backup_path.map(std::path::PathBuf::from).unwrap();
    assert!(backup.exists());
    let connection = Connection::open(backup).unwrap();
    let result: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(result, "ok");
    let project_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
        .unwrap();
    assert_eq!(project_count, 1);
}

#[test]
fn upgrade_preserves_sql_valid_unicode_repo_identity() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let identity = "é".repeat(300);
    create_version_one_database_with_values(&path, "/legacy", &identity);
    let (store, _) = Store::open(&path).unwrap();
    assert_eq!(store.list_projects().unwrap()[0].repo_identity, identity);
}

#[test]
fn failed_upgrade_rolls_back_schema_data_and_version() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    create_version_one_database_with_root(&path, "relative/repository");

    assert!(matches!(
        Store::open(&path),
        Err(StoreError::IntegrityFailure(_))
    ));
    let connection = Connection::open(&path).unwrap();
    let user_version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(user_version, 1);
    let migration_count: u32 = connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(migration_count, 1);
    let event_table_exists: bool = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_schema WHERE type='table' AND name='domain_events'
             )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!event_table_exists);
    let project_count: u32 = connection
        .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
        .unwrap();
    assert_eq!(project_count, 1);
    assert!(
        directory
            .path()
            .read_dir()
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().contains("pre-v2"))
    );
}

#[test]
fn rejects_newer_and_checksum_mismatched_databases() {
    let directory = TempDir::new().unwrap();
    let too_new = database_path(&directory);
    let connection = Connection::open(&too_new).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);
    assert!(matches!(
        Store::open(&too_new),
        Err(StoreError::DatabaseTooNew {
            found: 99,
            supported: 2
        })
    ));

    let second_directory = TempDir::new().unwrap();
    let mismatched = database_path(&second_directory);
    create_version_one_database(&mismatched);
    let connection = Connection::open(&mismatched).unwrap();
    connection
        .execute(
            "UPDATE schema_migrations SET sha256=?1 WHERE version=1",
            [vec![0_u8; 32]],
        )
        .unwrap();
    drop(connection);
    assert!(matches!(
        Store::open(&mismatched),
        Err(StoreError::MigrationChecksumMismatch(1))
    ));

    let third_directory = TempDir::new().unwrap();
    let renamed = database_path(&third_directory);
    create_version_one_database(&renamed);
    let connection = Connection::open(&renamed).unwrap();
    connection
        .execute(
            "UPDATE schema_migrations SET name='renamed' WHERE version=1",
            [],
        )
        .unwrap();
    drop(connection);
    assert!(matches!(
        Store::open(&renamed),
        Err(StoreError::MigrationChecksumMismatch(1))
    ));
}

fn create_version_one_database(path: &Path) {
    create_version_one_database_with_values(path, "/legacy", &"x".repeat(129));
}

fn create_version_one_database_with_root(path: &Path, repo_root: &str) {
    create_version_one_database_with_values(path, repo_root, &"x".repeat(129));
}

fn create_version_one_database_with_values(path: &Path, repo_root: &str, repo_identity: &str) {
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch(include_str!("../../../migrations/0001_core.sql"))
        .unwrap();
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY CHECK(version > 0),
                name TEXT NOT NULL UNIQUE,
                sha256 BLOB NOT NULL CHECK(length(sha256) = 32),
                applied_at_unix_ms INTEGER NOT NULL CHECK(applied_at_unix_ms >= 0)
            ) STRICT;",
        )
        .unwrap();
    let sql = include_str!("../../../migrations/0001_core.sql");
    let digest: [u8; 32] = Sha256::digest(sql.as_bytes()).into();
    connection
        .execute(
            "INSERT INTO schema_migrations(version, name, sha256, applied_at_unix_ms)
             VALUES (1, 'core', ?1, 1)",
            [digest.to_vec()],
        )
        .unwrap();
    connection.pragma_update(None, "user_version", 1).unwrap();
    connection
        .execute(
            "INSERT INTO projects(
                id, display_name, repo_root, repo_identity, default_branch, trust_state,
                weight, provider_policy, data_classification, revision, created_at_unix_ms,
                updated_at_unix_ms, last_event_sequence
             ) VALUES (?1, 'Legacy', ?2, ?3, 'main', 'untrusted',
                       1.0, 'local', 'source', 1, 1, 1, 1)",
            params![Uuid::now_v7().to_string(), repo_root, repo_identity],
        )
        .unwrap();
}
