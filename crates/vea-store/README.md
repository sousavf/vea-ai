# vea-store

Rust-host-owned durable storage for Vea.

## Guarantees

- SQLite uses WAL, `synchronous=FULL`, foreign keys, trusted-schema disabled, defensive mode, a bounded busy timeout, and a single-process lock.
- Commands are validated and committed atomically with their receipt, domain event, materialized projection, projection cursor, and audit record.
- UUIDv7 command IDs provide exact-retry idempotency. Reusing an ID with different command bytes fails closed.
- Aggregate revisions are optimistic concurrency tokens.
- Domain events, command receipts, and audit records are immutable at the database layer.
- Project and side-effect projections are replayed and verified on startup, then repaired from valid events when necessary.
- A side effect is persisted as `authorized -> started -> finished`; startup converts an interrupted `started` action to `unknown` without replaying it.
- Embedded migrations are checksummed, transactional, version-gated, and backed up through SQLite's online backup API before an upgrade.
- Verified backups are published without overwriting an existing file; locked restore validates the source and keeps a rollback copy until replacement succeeds.

The crate does not expose a SQLite connection or generic SQL/event append API. Future Tauri commands and broker code must use the typed `Store` facade.

## Validation

```sh
cargo test -p vea-store
cargo clippy -p vea-store --all-targets -- -D warnings
```
