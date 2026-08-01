use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    time::Duration,
};

#[cfg(unix)]
use std::fs::File;

use rusqlite::{Connection, TransactionBehavior, backup::Backup, config::DbConfig, params};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{Project, StoreError};

struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "core",
        sql: include_str!("../../../migrations/0001_core.sql"),
    },
    Migration {
        version: 2,
        name: "events_audit",
        sql: include_str!("../../../migrations/0002_events_audit.sql"),
    },
];

pub(crate) struct MigrationReport {
    pub schema_version: u32,
    pub applied: Vec<u32>,
    pub backup_path: Option<PathBuf>,
}

pub(crate) fn configure_connection(connection: &Connection) -> Result<(), StoreError> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "trusted_schema", "OFF")?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "FULL")?;
    connection.pragma_update(None, "wal_autocheckpoint", 1_000)?;
    connection.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;

    let foreign_keys: u8 = connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    let trusted_schema: u8 =
        connection.pragma_query_value(None, "trusted_schema", |row| row.get(0))?;
    let journal_mode: String =
        connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    let synchronous: u8 = connection.pragma_query_value(None, "synchronous", |row| row.get(0))?;
    let defensive = connection.db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE)?;
    if foreign_keys != 1
        || trusted_schema != 0
        || !journal_mode.eq_ignore_ascii_case("wal")
        || synchronous != 2
        || !defensive
    {
        return Err(StoreError::IntegrityFailure(
            "required SQLite safety configuration is unavailable".into(),
        ));
    }
    Ok(())
}

pub(crate) fn quick_check(connection: &Connection) -> Result<(), StoreError> {
    let result: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if result != "ok" {
        return Err(StoreError::IntegrityFailure(format!(
            "quick_check: {result}"
        )));
    }
    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    if statement.query([])?.next()?.is_some() {
        return Err(StoreError::IntegrityFailure(
            "foreign key check failed".into(),
        ));
    }
    Ok(())
}

pub(crate) fn apply_migrations(
    connection: &mut Connection,
    database_path: &Path,
) -> Result<MigrationReport, StoreError> {
    let supported = MIGRATIONS.last().map_or(0, |migration| migration.version);
    let user_version: u32 =
        connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if user_version > supported {
        return Err(StoreError::DatabaseTooNew {
            found: user_version,
            supported,
        });
    }

    let migration_table_exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_schema WHERE type='table' AND name='schema_migrations'
         )",
        [],
        |row| row.get(0),
    )?;
    if !migration_table_exists {
        let user_table_count: u32 = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )?;
        if user_version != 0 || user_table_count != 0 {
            return Err(StoreError::IntegrityFailure(
                "database has schema but no migration history".into(),
            ));
        }
    }

    let applied_version = if migration_table_exists {
        verify_applied_checksums(connection)?
    } else {
        0
    };
    if user_version != applied_version {
        return Err(StoreError::IntegrityFailure(format!(
            "schema version metadata disagrees: pragma={user_version}, migrations={applied_version}"
        )));
    }
    let pending: Vec<_> = MIGRATIONS
        .iter()
        .filter(|migration| migration.version > applied_version)
        .collect();
    if pending.is_empty() {
        return Ok(MigrationReport {
            schema_version: supported,
            applied: Vec::new(),
            backup_path: None,
        });
    }

    let backup_path = if applied_version > 0 {
        Some(backup_database(
            connection,
            database_path,
            pending[0].version,
        )?)
    } else {
        None
    };

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if !migration_table_exists {
        transaction.execute_batch(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY CHECK(version > 0),
                name TEXT NOT NULL UNIQUE,
                sha256 BLOB NOT NULL CHECK(length(sha256) = 32),
                applied_at_unix_ms INTEGER NOT NULL CHECK(applied_at_unix_ms >= 0)
            ) STRICT;",
        )?;
    }
    let now = unix_ms()?;
    let mut applied = Vec::new();
    for migration in pending {
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations(version, name, sha256, applied_at_unix_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                migration.version,
                migration.name,
                checksum(migration.sql).to_vec(),
                i64::try_from(now).map_err(|_| StoreError::InvalidInput("system clock"))?
            ],
        )?;
        transaction.pragma_update(None, "user_version", migration.version)?;
        applied.push(migration.version);
    }
    validate_imported_projects(&transaction)?;
    quick_check(&transaction)?;
    transaction.commit()?;
    quick_check(connection)?;

    Ok(MigrationReport {
        schema_version: supported,
        applied,
        backup_path,
    })
}

fn validate_imported_projects(connection: &Connection) -> Result<(), StoreError> {
    let event_table_exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_schema WHERE type='table' AND name='domain_events'
         )",
        [],
        |row| row.get(0),
    )?;
    if !event_table_exists {
        return Ok(());
    }
    let mut statement = connection.prepare(
        "SELECT aggregate_id, aggregate_revision, payload_json
         FROM domain_events WHERE kind='project.imported'",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let aggregate_id: String = row.get(0)?;
        let revision: i64 = row.get(1)?;
        let payload_json: String = row.get(2)?;
        let project: Project = serde_json::from_str(&payload_json)?;
        project.validate().map_err(|error| {
            StoreError::IntegrityFailure(format!("invalid migrated project: {error}"))
        })?;
        if project.id != aggregate_id
            || i64::try_from(project.revision)
                .map_err(|_| StoreError::IntegrityFailure("project revision overflow".into()))?
                != revision
        {
            return Err(StoreError::IntegrityFailure(
                "migrated project event metadata mismatch".into(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_migration_history(connection: &Connection) -> Result<u32, StoreError> {
    let supported = MIGRATIONS.last().map_or(0, |migration| migration.version);
    let user_version: u32 =
        connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if user_version > supported {
        return Err(StoreError::DatabaseTooNew {
            found: user_version,
            supported,
        });
    }
    let migration_table_exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_schema WHERE type='table' AND name='schema_migrations'
         )",
        [],
        |row| row.get(0),
    )?;
    if !migration_table_exists {
        return Err(StoreError::IntegrityFailure(
            "database has no migration history".into(),
        ));
    }
    let applied_version = verify_applied_checksums(connection)?;
    if user_version != applied_version {
        return Err(StoreError::IntegrityFailure(format!(
            "schema version metadata disagrees: pragma={user_version}, migrations={applied_version}"
        )));
    }
    Ok(applied_version)
}

fn verify_applied_checksums(connection: &Connection) -> Result<u32, StoreError> {
    let mut statement = connection
        .prepare("SELECT version, name, sha256 FROM schema_migrations ORDER BY version")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, u32>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Vec<u8>>(2)?,
        ))
    })?;
    let mut expected_version = 1;
    let mut latest_version = 0;
    for row in rows {
        let (version, stored_name, stored_checksum) = row?;
        if version != expected_version {
            return Err(StoreError::IntegrityFailure(format!(
                "non-contiguous migration history at version {version}"
            )));
        }
        let Some(migration) = MIGRATIONS
            .iter()
            .find(|migration| migration.version == version)
        else {
            return Err(StoreError::DatabaseTooNew {
                found: version,
                supported: MIGRATIONS.last().map_or(0, |migration| migration.version),
            });
        };
        if stored_name != migration.name || stored_checksum.as_slice() != checksum(migration.sql) {
            return Err(StoreError::MigrationChecksumMismatch(version));
        }
        latest_version = version;
        expected_version += 1;
    }
    Ok(latest_version)
}

fn backup_database(
    source: &Connection,
    database_path: &Path,
    next_version: u32,
) -> Result<PathBuf, StoreError> {
    let file_name = database_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("vea.sqlite");
    let backup_path = database_path.with_file_name(format!(
        "{file_name}.pre-v{next_version}.{}.bak",
        Uuid::now_v7()
    ));
    create_private_file(&backup_path)?;
    let mut destination = Connection::open(&backup_path)?;
    let backup = Backup::new(source, &mut destination)?;
    backup.run_to_completion(16, Duration::from_millis(10), None)?;
    drop(backup);
    quick_check(&destination)?;
    drop(destination);
    set_user_only_permissions(&backup_path)?;
    sync_file(&backup_path)?;
    sync_parent_directory(&backup_path)?;
    Ok(backup_path)
}

fn checksum(sql: &str) -> [u8; 32] {
    Sha256::digest(sql.as_bytes()).into()
}

fn unix_ms() -> Result<u64, StoreError> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| StoreError::InvalidInput("system clock"))?;
    u64::try_from(duration.as_millis()).map_err(|_| StoreError::InvalidInput("system clock"))
}

pub(crate) fn set_user_only_permissions(_path: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(_path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub(crate) fn backup_to_path(
    source: &Connection,
    destination_path: &Path,
) -> Result<(), StoreError> {
    if destination_path.exists() {
        return Err(StoreError::InvalidInput("backup destination exists"));
    }
    create_private_file(destination_path)?;
    let backup_result = (|| {
        let mut destination = Connection::open(destination_path)?;
        let backup = Backup::new(source, &mut destination)?;
        backup.run_to_completion(16, Duration::from_millis(10), None)?;
        drop(backup);
        quick_check(&destination)?;
        drop(destination);
        set_user_only_permissions(destination_path)
    })();
    if backup_result.is_err() {
        let _ = fs::remove_file(destination_path);
    }
    backup_result
}

fn create_private_file(path: &Path) -> Result<(), StoreError> {
    let mut options = OpenOptions::new();
    options.create_new(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?;
    Ok(())
}

fn sync_file(path: &Path) -> Result<(), StoreError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?
        .sync_all()?;
    Ok(())
}

fn sync_parent_directory(_path: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    if let Some(parent) = _path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}
