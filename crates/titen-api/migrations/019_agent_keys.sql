-- Per-agent API keys with a scope. Only the SHA-256 of the key is stored;
-- the plaintext is shown once at creation.
CREATE TABLE IF NOT EXISTS agent_keys (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    key_hash     TEXT NOT NULL UNIQUE,
    key_prefix   TEXT NOT NULL,
    scope        TEXT NOT NULL CHECK (scope IN ('read', 'write')),
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    last_used_at TEXT,
    revoked_at   TEXT
);
