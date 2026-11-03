//! Per-agent API keys.
//!
//! The instance's master `TITEN_API_KEY` keeps full access. Agent keys are
//! created from the admin UI, carry a scope, and can be revoked individually.
//! Only a SHA-256 hash is stored; the plaintext (`tk_…`) is returned once.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{Result, Store, TitenError};

/// What an agent key may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyScope {
    /// Read-only: GET requests and read-only MCP tools.
    Read,
    /// Everything an operator can do except managing agent keys.
    Write,
}

impl KeyScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyScope::Read => "read",
            KeyScope::Write => "write",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "read" => Some(KeyScope::Read),
            "write" => Some(KeyScope::Write),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AgentKey {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub scope: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

pub const KEY_PREFIX: &str = "tk_";

pub fn hash_key(key: &str) -> String {
    hex::encode(Sha256::digest(key.as_bytes()))
}

impl Store {
    /// Create a key. Returns `(record, plaintext_key)`; the plaintext is never stored.
    pub async fn create_agent_key(&self, name: &str, scope: KeyScope) -> Result<(AgentKey, String)> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(TitenError::InvalidRequest(
                "name must be 1-80 characters".to_string(),
            ));
        }
        let mut buf = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut buf);
        let key = format!("{KEY_PREFIX}{}", hex::encode(buf));
        let prefix: String = key.chars().take(KEY_PREFIX.len() + 6).collect();
        let id = uuid::Uuid::now_v7().to_string();

        sqlx::query(
            "INSERT INTO agent_keys (id, name, key_hash, key_prefix, scope) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(hash_key(&key))
        .bind(&prefix)
        .bind(scope.as_str())
        .execute(self.pool())
        .await?;

        Ok((self.get_agent_key(&id).await?, key))
    }

    pub async fn get_agent_key(&self, id: &str) -> Result<AgentKey> {
        sqlx::query_as::<_, AgentKey>(
            "SELECT id, name, key_prefix, scope, created_at, last_used_at, revoked_at FROM agent_keys WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await?
        .ok_or_else(|| TitenError::InvalidRequest(format!("agent key not found: {id}")))
    }

    pub async fn list_agent_keys(&self) -> Result<Vec<AgentKey>> {
        Ok(sqlx::query_as::<_, AgentKey>(
            "SELECT id, name, key_prefix, scope, created_at, last_used_at, revoked_at FROM agent_keys ORDER BY created_at DESC",
        )
        .fetch_all(self.pool())
        .await?)
    }

    /// Revoke (soft-delete) a key. Returns false if it doesn't exist.
    pub async fn revoke_agent_key(&self, id: &str) -> Result<bool> {
        let r = sqlx::query(
            "UPDATE agent_keys SET revoked_at = strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id = ? AND revoked_at IS NULL",
        )
        .bind(id)
        .execute(self.pool())
        .await?;
        Ok(r.rows_affected() > 0)
    }

    /// Resolve a presented key to its scope (None if unknown or revoked) and
    /// record the use.
    pub async fn authenticate_agent_key(&self, key: &str) -> Result<Option<(String, KeyScope)>> {
        if !key.starts_with(KEY_PREFIX) {
            return Ok(None);
        }
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT id, scope FROM agent_keys WHERE key_hash = ? AND revoked_at IS NULL",
        )
        .bind(hash_key(key))
        .fetch_optional(self.pool())
        .await?;
        let Some((id, scope)) = row else { return Ok(None) };
        let Some(scope) = KeyScope::parse(&scope) else { return Ok(None) };
        let _ = sqlx::query(
            "UPDATE agent_keys SET last_used_at = strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id = ?",
        )
        .bind(&id)
        .execute(self.pool())
        .await;
        Ok(Some((id, scope)))
    }
}
