CREATE TABLE projection_state (
  name TEXT PRIMARY KEY,
  schema_version INTEGER NOT NULL CHECK(schema_version > 0),
  last_global_sequence INTEGER NOT NULL DEFAULT 0 CHECK(last_global_sequence >= 0),
  rebuilt_at_unix_ms INTEGER
) STRICT;

CREATE TABLE projects (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL CHECK(length(display_name) BETWEEN 1 AND 200),
  repo_root TEXT NOT NULL COLLATE BINARY UNIQUE,
  repo_identity TEXT NOT NULL CHECK(length(repo_identity) BETWEEN 1 AND 512),
  default_branch TEXT NOT NULL CHECK(length(default_branch) BETWEEN 1 AND 255),
  trust_state TEXT NOT NULL CHECK(trust_state IN ('untrusted', 'trusted', 'revoked')),
  weight REAL NOT NULL CHECK(weight BETWEEN 0.1 AND 100.0),
  provider_policy TEXT NOT NULL CHECK(length(provider_policy) BETWEEN 1 AND 128),
  data_classification TEXT NOT NULL CHECK(length(data_classification) BETWEEN 1 AND 128),
  revision INTEGER NOT NULL CHECK(revision > 0),
  created_at_unix_ms INTEGER NOT NULL CHECK(created_at_unix_ms >= 0),
  updated_at_unix_ms INTEGER NOT NULL CHECK(updated_at_unix_ms >= created_at_unix_ms),
  last_event_sequence INTEGER NOT NULL CHECK(last_event_sequence > 0)
) STRICT;

CREATE INDEX projects_repo_identity_idx ON projects(repo_identity);
INSERT INTO projection_state(name, schema_version, last_global_sequence)
VALUES ('projects', 1, 0);
