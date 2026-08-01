CREATE TABLE command_receipts (
  command_id TEXT PRIMARY KEY,
  command_schema_version INTEGER NOT NULL CHECK(command_schema_version > 0),
  command_kind TEXT NOT NULL,
  aggregate_type TEXT NOT NULL,
  aggregate_id TEXT NOT NULL,
  expected_revision INTEGER NOT NULL CHECK(expected_revision >= 0),
  request_sha256 BLOB NOT NULL CHECK(length(request_sha256) = 32),
  aggregate_revision INTEGER NOT NULL CHECK(aggregate_revision > 0),
  first_global_sequence INTEGER NOT NULL CHECK(first_global_sequence > 0),
  last_global_sequence INTEGER NOT NULL CHECK(last_global_sequence >= first_global_sequence),
  actor_kind TEXT NOT NULL,
  actor_id TEXT,
  correlation_id TEXT NOT NULL,
  committed_at_unix_ms INTEGER NOT NULL CHECK(committed_at_unix_ms >= 0)
) STRICT;

CREATE TABLE domain_events (
  global_sequence INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE,
  aggregate_type TEXT NOT NULL,
  aggregate_id TEXT NOT NULL,
  aggregate_revision INTEGER NOT NULL CHECK(aggregate_revision > 0),
  schema_version INTEGER NOT NULL CHECK(schema_version > 0),
  kind TEXT NOT NULL,
  payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),
  command_id TEXT NOT NULL,
  command_schema_version INTEGER NOT NULL CHECK(command_schema_version > 0),
  command_kind TEXT NOT NULL,
  expected_revision INTEGER NOT NULL CHECK(expected_revision >= 0),
  request_sha256 BLOB NOT NULL CHECK(length(request_sha256) = 32),
  causation_event_id TEXT,
  correlation_id TEXT NOT NULL,
  actor_kind TEXT NOT NULL,
  actor_id TEXT,
  occurred_at_unix_ms INTEGER NOT NULL CHECK(occurred_at_unix_ms >= 0),
  UNIQUE(aggregate_type, aggregate_id, aggregate_revision),
  FOREIGN KEY(command_id) REFERENCES command_receipts(command_id)
    DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE INDEX domain_events_aggregate_idx
  ON domain_events(aggregate_type, aggregate_id, aggregate_revision);
CREATE INDEX domain_events_correlation_idx ON domain_events(correlation_id);

CREATE TRIGGER domain_events_no_update
BEFORE UPDATE ON domain_events BEGIN
  SELECT RAISE(ABORT, 'domain_events are append-only');
END;
CREATE TRIGGER domain_events_no_delete
BEFORE DELETE ON domain_events BEGIN
  SELECT RAISE(ABORT, 'domain_events are append-only');
END;

CREATE TABLE audit_events (
  audit_sequence INTEGER PRIMARY KEY AUTOINCREMENT,
  audit_id TEXT NOT NULL UNIQUE,
  occurred_at_unix_ms INTEGER NOT NULL CHECK(occurred_at_unix_ms >= 0),
  actor_kind TEXT NOT NULL,
  actor_id TEXT,
  action TEXT NOT NULL,
  project_id TEXT,
  run_id TEXT,
  action_id TEXT,
  command_id TEXT NOT NULL,
  correlation_id TEXT NOT NULL,
  policy_decision TEXT CHECK(policy_decision IS NULL OR policy_decision IN ('allowed', 'approved')),
  approval_digest TEXT,
  provider_id TEXT,
  account_alias TEXT,
  affected_paths_json TEXT CHECK(affected_paths_json IS NULL OR json_valid(affected_paths_json)),
  destination TEXT,
  result_class TEXT NOT NULL CHECK(result_class IN ('succeeded', 'failed', 'authorized', 'started', 'unknown'))
) STRICT;

CREATE INDEX audit_events_project_idx ON audit_events(project_id, audit_sequence);
CREATE INDEX audit_events_correlation_idx ON audit_events(correlation_id);

-- Migration 1 could persist project projections before the event log existed. Import
-- each such projection as a self-contained baseline event before normal writes begin.
INSERT INTO domain_events(
  event_id, aggregate_type, aggregate_id, aggregate_revision, schema_version,
  kind, payload_json, command_id, command_schema_version, command_kind,
  expected_revision, request_sha256, correlation_id, actor_kind, actor_id,
  occurred_at_unix_ms
)
SELECT
  id, 'project', id, revision, 1, 'project.imported',
  json_object(
    'id', id,
    'display_name', display_name,
    'repo_root', repo_root,
    'repo_identity', repo_identity,
    'default_branch', default_branch,
    'trust_state', trust_state,
    'weight', weight,
    'provider_policy', provider_policy,
    'data_classification', data_classification,
    'revision', revision,
    'created_at_unix_ms', created_at_unix_ms,
    'updated_at_unix_ms', updated_at_unix_ms,
    'last_event_sequence', 0
  ),
  id, 1, 'project.import', 0, zeroblob(32), id, 'system', 'migration-v2',
  updated_at_unix_ms
FROM projects;

INSERT INTO command_receipts(
  command_id, command_schema_version, command_kind, aggregate_type, aggregate_id,
  expected_revision, request_sha256, aggregate_revision, first_global_sequence,
  last_global_sequence, actor_kind, actor_id, correlation_id, committed_at_unix_ms
)
SELECT
  event.command_id, 1, 'project.import', 'project', event.aggregate_id, 0,
  zeroblob(32), event.aggregate_revision, event.global_sequence,
  event.global_sequence, 'system', 'migration-v2', event.correlation_id,
  event.occurred_at_unix_ms
FROM domain_events AS event
WHERE event.kind = 'project.imported';

INSERT INTO audit_events(
  audit_id, occurred_at_unix_ms, actor_kind, actor_id, action, project_id,
  command_id, correlation_id, result_class
)
SELECT
  event.event_id, event.occurred_at_unix_ms, 'system', 'migration-v2',
  'project.import', event.aggregate_id, event.command_id, event.correlation_id,
  'succeeded'
FROM domain_events AS event
WHERE event.kind = 'project.imported';

UPDATE projects
SET last_event_sequence = (
  SELECT event.global_sequence
  FROM domain_events AS event
  WHERE event.aggregate_type = 'project' AND event.aggregate_id = projects.id
);
UPDATE projection_state
SET last_global_sequence = COALESCE((SELECT MAX(global_sequence) FROM domain_events), 0)
WHERE name = 'projects';

CREATE TRIGGER command_receipts_no_update
BEFORE UPDATE ON command_receipts BEGIN
  SELECT RAISE(ABORT, 'command_receipts are immutable');
END;
CREATE TRIGGER command_receipts_no_delete
BEFORE DELETE ON command_receipts BEGIN
  SELECT RAISE(ABORT, 'command_receipts are immutable');
END;
CREATE TRIGGER audit_events_no_update
BEFORE UPDATE ON audit_events BEGIN
  SELECT RAISE(ABORT, 'audit_events cannot be rewritten');
END;
CREATE TRIGGER audit_events_no_delete
BEFORE DELETE ON audit_events BEGIN
  SELECT RAISE(ABORT, 'audit_events cannot be rewritten');
END;

CREATE TABLE side_effects (
  action_id TEXT PRIMARY KEY,
  project_id TEXT,
  run_id TEXT NOT NULL,
  capability TEXT NOT NULL,
  action_digest TEXT NOT NULL,
  approval_id TEXT NOT NULL,
  binding_digest TEXT NOT NULL,
  phase TEXT NOT NULL CHECK(phase IN ('authorized', 'started', 'finished', 'unknown')),
  revision INTEGER NOT NULL CHECK(revision > 0),
  result_class TEXT CHECK(result_class IS NULL OR result_class IN ('succeeded', 'failed')),
  authorized_at_unix_ms INTEGER NOT NULL CHECK(authorized_at_unix_ms >= 0),
  started_at_unix_ms INTEGER,
  finished_at_unix_ms INTEGER,
  updated_at_unix_ms INTEGER NOT NULL CHECK(updated_at_unix_ms >= authorized_at_unix_ms),
  last_event_sequence INTEGER NOT NULL CHECK(last_event_sequence > 0),
  CHECK((phase = 'authorized' AND started_at_unix_ms IS NULL AND finished_at_unix_ms IS NULL)
     OR (phase IN ('started', 'unknown') AND started_at_unix_ms IS NOT NULL AND finished_at_unix_ms IS NULL)
     OR (phase = 'finished' AND started_at_unix_ms IS NOT NULL AND finished_at_unix_ms IS NOT NULL))
) STRICT;

INSERT INTO projection_state(name, schema_version, last_global_sequence)
VALUES ('side_effects', 1, 0);
