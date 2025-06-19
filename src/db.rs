use crate::models::{ChatSession, Message, Role};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Row};
use std::path::Path;

pub fn get_connection(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    setup_database(&conn)?;
    Ok(conn)
}

fn setup_database(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "BEGIN;
        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id INTEGER NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions (id)
        );
        CREATE TABLE IF NOT EXISTS config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        COMMIT;",
    )?;
    Ok(())
}

pub fn get_next_session_id(conn: &Connection) -> Result<i64> {
    let id: i64 = conn.query_row("SELECT ifnull(max(id), 0) + 1 FROM sessions", [], |row| {
        row.get(0)
    })?;
    Ok(id)
}

pub fn save_config(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO config (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

pub fn load_config(conn: &Connection, key: &str) -> Result<Option<String>> {
    let value = conn.query_row("SELECT value FROM config WHERE key = ?1", params![key], |row| {
        row.get(0)
    });
    Ok(value.ok())
}

pub fn save_session(conn: &Connection, session: &mut ChatSession) -> Result<()> {
    conn.execute(
        "INSERT INTO sessions (name, created_at) VALUES (?1, ?2)",
        params![session.name, session.created_at.to_rfc3339()],
    )?;
    session.id = conn.last_insert_rowid();
    Ok(())
}

pub fn save_message(conn: &Connection, session_id: i64, message: &Message) -> Result<()> {
    let role_str = match message.role {
        Role::User => "user",
        Role::Assistant => "assistant",
    };
    conn.execute(
        "INSERT INTO messages (session_id, role, content) VALUES (?1, ?2, ?3)",
        params![session_id, role_str, message.content],
    )?;
    Ok(())
}

pub fn clear_messages_for_session(conn: &Connection, session_id: i64) -> Result<()> {
    conn.execute("DELETE FROM messages WHERE session_id = ?1", params![session_id])?;
    Ok(())
}

pub fn load_sessions(conn: &Connection) -> Result<Vec<ChatSession>> {
    let mut stmt =
        conn.prepare("SELECT id, name, created_at FROM sessions ORDER BY created_at ASC")?;
    let session_iter = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let name: String = row.get(1)?;
        let created_at_str: String = row.get(2)?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(ChatSession {
            id,
            name,
            messages: Vec::new(),
            created_at,
        })
    })?;

    let mut sessions = Vec::new();
    for session_result in session_iter {
        let mut session = session_result?;
        session.messages = load_messages_for_session(conn, session.id)?;
        sessions.push(session);
    }
    Ok(sessions)
}

fn load_messages_for_session(conn: &Connection, session_id: i64) -> Result<Vec<Message>> {
    let mut stmt = conn
        .prepare("SELECT role, content FROM messages WHERE session_id = ?1 ORDER BY id ASC")?;
    let message_iter = stmt.query_map(params![session_id], |row: &Row| {
        let role_str: String = row.get(0)?;
        let content: String = row.get(1)?;
        let role = if role_str == "user" {
            Role::User
        } else {
            Role::Assistant
        };
        Ok(Message { role, content })
    })?;

    let mut messages = Vec::new();
    for message_result in message_iter {
        messages.push(message_result?);
    }
    Ok(messages)
}

