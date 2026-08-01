use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("invalid store input: {0}")]
    InvalidInput(&'static str),
    #[error("database is already open by another Vea host")]
    DatabaseBusy,
    #[error("aggregate already exists: {0}")]
    AggregateAlreadyExists(String),
    #[error("aggregate not found: {0}")]
    AggregateNotFound(String),
    #[error("revision conflict for {aggregate_id}: expected {expected}, actual {actual}")]
    RevisionConflict {
        aggregate_id: String,
        expected: u64,
        actual: u64,
    },
    #[error("command id was reused with different content: {0}")]
    IdempotencyConflict(String),
    #[error("unsupported command schema: {0}")]
    UnsupportedCommandSchema(u16),
    #[error("unsupported event {kind} at schema version {schema_version}")]
    UnsupportedEvent { kind: String, schema_version: u16 },
    #[error("migration checksum mismatch at version {0}")]
    MigrationChecksumMismatch(u32),
    #[error("database schema version {found} is newer than supported {supported}")]
    DatabaseTooNew { found: u32, supported: u32 },
    #[error("store integrity failure: {0}")]
    IntegrityFailure(String),
    #[error("invalid state transition")]
    InvalidTransition,
    #[error("database operation failed")]
    Sqlite(#[source] rusqlite::Error),
    #[error("filesystem operation failed")]
    Io(#[source] std::io::Error),
    #[error("serialization failed")]
    Serialization(#[source] serde_json::Error),
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}
