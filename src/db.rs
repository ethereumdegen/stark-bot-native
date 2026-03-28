use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct Database {
    conn: Connection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: i64,
    pub capability: String,
    pub agent_id: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLog {
    pub id: i64,
    pub capability: String,
    pub session_id: Option<String>,
    pub message: String,
    pub role: String,
    pub status: String,
    pub result: Option<String>,
    pub created_at: String,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        let db = Self { conn };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<(), String> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS agents (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                capability TEXT NOT NULL UNIQUE,
                agent_id TEXT NOT NULL,
                name TEXT NOT NULL DEFAULT '',
                description TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                capability TEXT NOT NULL,
                session_id TEXT,
                message TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'user',
                status TEXT NOT NULL DEFAULT 'pending',
                result TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );"
        ).map_err(|e| e.to_string())
    }

    // ── Agents ──

    pub fn list_agents(&self) -> Result<Vec<Agent>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT id, capability, agent_id, name, description, status, created_at, updated_at
             FROM agents ORDER BY capability"
        ).map_err(|e| e.to_string())?;
        let agents = stmt.query_map([], |row| {
            Ok(Agent {
                id: row.get(0)?,
                capability: row.get(1)?,
                agent_id: row.get(2)?,
                name: row.get(3)?,
                description: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        }).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
        Ok(agents)
    }

    pub fn get_agent(&self, capability: &str) -> Result<Option<Agent>, String> {
        match self.conn.query_row(
            "SELECT id, capability, agent_id, name, description, status, created_at, updated_at
             FROM agents WHERE capability = ?1",
            [capability],
            |row| Ok(Agent {
                id: row.get(0)?,
                capability: row.get(1)?,
                agent_id: row.get(2)?,
                name: row.get(3)?,
                description: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            }),
        ) {
            Ok(a) => Ok(Some(a)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn upsert_agent(
        &self,
        capability: &str,
        agent_id: &str,
        name: &str,
        description: &str,
        status: &str,
    ) -> Result<(), String> {
        self.conn.execute(
            "INSERT INTO agents (capability, agent_id, name, description, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'), datetime('now'))
             ON CONFLICT(capability) DO UPDATE SET
             agent_id = ?2, name = ?3, description = ?4, status = ?5, updated_at = datetime('now')",
            params![capability, agent_id, name, description, status],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    // ── Sessions / Chat History ──

    pub fn log_message(
        &self,
        capability: &str,
        session_id: Option<&str>,
        message: &str,
        role: &str,
    ) -> Result<i64, String> {
        self.conn.execute(
            "INSERT INTO sessions (capability, session_id, message, role, status, created_at)
             VALUES (?1, ?2, ?3, ?4, 'sent', datetime('now'))",
            params![capability, session_id, message, role],
        ).map_err(|e| e.to_string())?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn complete_message(&self, id: i64, status: &str, result: &str) -> Result<(), String> {
        self.conn.execute(
            "UPDATE sessions SET status = ?1, result = ?2 WHERE id = ?3",
            params![status, result, id],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn recent_messages(&self, capability: &str, limit: u32) -> Result<Vec<SessionLog>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT id, capability, session_id, message, role, status, result, created_at
             FROM sessions WHERE capability = ?1 ORDER BY id DESC LIMIT ?2"
        ).map_err(|e| e.to_string())?;
        let msgs = stmt.query_map(params![capability, limit], |row| {
            Ok(SessionLog {
                id: row.get(0)?,
                capability: row.get(1)?,
                session_id: row.get(2)?,
                message: row.get(3)?,
                role: row.get(4)?,
                status: row.get(5)?,
                result: row.get(6)?,
                created_at: row.get(7)?,
            })
        }).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
        Ok(msgs)
    }

    // ── Config KV ──

    pub fn get_config(&self, key: &str) -> Result<Option<String>, String> {
        match self.conn.query_row(
            "SELECT value FROM config WHERE key = ?1",
            [key],
            |row| row.get(0),
        ) {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn set_config(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn.execute(
            "INSERT INTO config (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }
}
