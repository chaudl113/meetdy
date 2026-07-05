#![allow(dead_code)]
//! Database initialization, migrations, and CRUD operations for meeting sessions.

use anyhow::Result;
use log::{debug, info};
use r2d2::ManageConnection;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use rusqlite_migration::{Migrations, M};
use std::path::PathBuf;

use super::models::{AudioSourceType, MeetingNote, MeetingSession, MeetingStatus};

/// Custom r2d2 ManageConnection for rusqlite::Connection.
/// Avoids the version conflict between r2d2_sqlite (rusqlite 0.32) and our
/// project's rusqlite 0.37.
pub(crate) struct SqliteConnectionManager {
    path: PathBuf,
}

impl SqliteConnectionManager {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ManageConnection for SqliteConnectionManager {
    type Connection = Connection;
    type Error = rusqlite::Error;

    fn connect(&self) -> std::result::Result<Connection, rusqlite::Error> {
        Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
    }

    fn is_valid(&self, conn: &mut Connection) -> std::result::Result<(), rusqlite::Error> {
        conn.execute_batch("SELECT 1")
    }

    fn has_broken(&self, conn: &mut Connection) -> bool {
        conn.is_autocommit()
    }
}

/// Alias for the pool type used throughout the meeting module.
pub type Pool = r2d2::Pool<SqliteConnectionManager>;

/// Connection pool for the meetings SQLite database.
#[derive(Clone)]
pub struct DbPool(Pool);

impl DbPool {
    pub fn get(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        Ok(self.0.get()?)
    }
}

/// Database migrations for meeting sessions.
/// Each migration is applied in order. The library tracks which migrations
/// have been applied using SQLite's user_version pragma.
///
/// Note: This uses a separate database file from transcription history
/// to maintain complete separation between Meeting Mode and Quick Dictation.
static MIGRATIONS: &[M] = &[
    M::up(
        "CREATE TABLE IF NOT EXISTS meeting_sessions (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            duration INTEGER,
            status TEXT NOT NULL DEFAULT 'idle',
            audio_path TEXT,
            transcript_path TEXT,
            error_message TEXT
        );",
    ),
    M::up(
        "ALTER TABLE meeting_sessions ADD COLUMN audio_source TEXT NOT NULL DEFAULT 'microphone_only';",
    ),
    M::up(
        "ALTER TABLE meeting_sessions ADD COLUMN summary_path TEXT;",
    ),
    M::up(
        "ALTER TABLE meeting_sessions ADD COLUMN template_id TEXT;",
    ),
    M::up(
        "CREATE TABLE IF NOT EXISTS meeting_notes (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            timestamp_seconds INTEGER NOT NULL DEFAULT 0,
            content TEXT NOT NULL,
            author TEXT,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (session_id) REFERENCES meeting_sessions(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_meeting_notes_session_id
            ON meeting_notes(session_id);",
    ),
];

/// Initialize the meeting sessions database and return a connection pool.
///
/// This function opens (or creates) the database at the specified path,
/// applies all pending migrations, then wraps the database in an r2d2
/// connection pool for reuse across CRUD operations.
///
/// # Arguments
/// * `db_path` - Path to the SQLite database file
///
/// # Returns
/// * `Ok(DbPool)` - Connection pool for the initialized database
/// * `Err` if the database could not be opened or migrations failed
pub fn init_meeting_database(db_path: &PathBuf) -> Result<DbPool> {
    info!("Initializing meeting database at {:?}", db_path);

    let mut conn = Connection::open(db_path)?;

    // Create migrations object and run to latest version
    let migrations = Migrations::new(MIGRATIONS.to_vec());

    // Validate migrations in debug builds
    #[cfg(debug_assertions)]
    migrations.validate().expect("Invalid migrations");

    // Get current version before migration
    let version_before: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    debug!(
        "Meeting database version before migration: {}",
        version_before
    );

    // Apply any pending migrations
    migrations.to_latest(&mut conn)?;

    // Get version after migration
    let version_after: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

    if version_after > version_before {
        info!(
            "Meeting database migrated from version {} to {}",
            version_before, version_after
        );
    } else {
        debug!(
            "Meeting database already at latest version {}",
            version_after
        );
    }

    // Drop the migration connection before creating the pool
    drop(conn);

    let manager = SqliteConnectionManager::new(db_path.clone());
    let pool = r2d2::Pool::builder()
        .max_size(4)
        .build(manager)?;

    Ok(DbPool(pool))
}

/// Helper functions for database serialization/deserialization of enums.
pub(crate) fn status_to_string(status: &MeetingStatus) -> String {
    match status {
        MeetingStatus::Idle => "idle".to_string(),
        MeetingStatus::Recording => "recording".to_string(),
        MeetingStatus::Processing => "processing".to_string(),
        MeetingStatus::Completed => "completed".to_string(),
        MeetingStatus::Failed => "failed".to_string(),
        MeetingStatus::Interrupted => "interrupted".to_string(),
    }
}

pub(crate) fn string_to_status(s: &str) -> MeetingStatus {
    match s {
        "recording" => MeetingStatus::Recording,
        "processing" => MeetingStatus::Processing,
        "completed" => MeetingStatus::Completed,
        "failed" => MeetingStatus::Failed,
        "interrupted" => MeetingStatus::Interrupted,
        _ => MeetingStatus::Idle,
    }
}

pub(crate) fn audio_source_to_string(source: &AudioSourceType) -> &'static str {
    match source {
        AudioSourceType::MicrophoneOnly => "microphone_only",
        AudioSourceType::SystemOnly => "system_only",
        AudioSourceType::Mixed => "mixed",
    }
}

pub(crate) fn string_to_audio_source(s: &str) -> AudioSourceType {
    match s {
        "system_only" => AudioSourceType::SystemOnly,
        "mixed" => AudioSourceType::Mixed,
        _ => AudioSourceType::MicrophoneOnly,
    }
}

/// Converts a database row to a MeetingSession struct.
pub(crate) fn row_to_session(row: &rusqlite::Row) -> rusqlite::Result<MeetingSession> {
    let status_str: String = row.get(4)?;
    let audio_source_str: String = row.get(7)?;
    Ok(MeetingSession {
        id: row.get(0)?,
        title: row.get(1)?,
        created_at: row.get(2)?,
        duration: row.get(3)?,
        status: string_to_status(&status_str),
        audio_path: row.get(5)?,
        transcript_path: row.get(6)?,
        error_message: row.get(8)?,
        audio_source: string_to_audio_source(&audio_source_str),
        summary_path: row.get(9)?,
        template_id: row.get(10)?,
    })
}

/// Creates a new session record in the database.
pub(crate) fn insert_session(pool: &DbPool, session: &MeetingSession) -> Result<()> {
    let conn = pool.get()?;
    conn.execute(
        "INSERT INTO meeting_sessions (id, title, created_at, status, audio_source, template_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            session.id,
            session.title,
            session.created_at,
            status_to_string(&session.status),
            audio_source_to_string(&session.audio_source),
            session.template_id,
        ],
    )?;
    Ok(())
}

/// Retrieves a meeting session by its ID.
pub(crate) fn get_session(pool: &DbPool, session_id: &str) -> Result<Option<MeetingSession>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, created_at, duration, status, audio_path, transcript_path, audio_source, error_message, summary_path, template_id
         FROM meeting_sessions WHERE id = ?1",
    )?;
    let session = stmt
        .query_row(params![session_id], |row| row_to_session(row))
        .optional()?;
    Ok(session)
}

/// Updates the status of a meeting session.
pub(crate) fn update_session_status(
    pool: &DbPool,
    session_id: &str,
    status: &MeetingStatus,
) -> Result<()> {
    let conn = pool.get()?;
    let rows = conn.execute(
        "UPDATE meeting_sessions SET status = ?1 WHERE id = ?2",
        params![status_to_string(status), session_id],
    )?;
    if rows == 0 {
        return Err(anyhow::anyhow!(
            "Session not found: {}",
            session_id
        ));
    }
    Ok(())
}

/// Updates the status of a meeting session with an error message.
pub(crate) fn update_session_status_with_error(
    pool: &DbPool,
    session_id: &str,
    status: &MeetingStatus,
    error_message: &str,
) -> Result<()> {
    let conn = pool.get()?;
    let rows = conn.execute(
        "UPDATE meeting_sessions SET status = ?1, error_message = ?2 WHERE id = ?3",
        params![status_to_string(status), error_message, session_id],
    )?;
    if rows == 0 {
        return Err(anyhow::anyhow!(
            "Session not found: {}",
            session_id
        ));
    }
    Ok(())
}

/// Lists all meeting sessions, ordered by creation time (newest first).
/// Use list_sessions_paginated() for paginated queries.
pub(crate) fn list_sessions(pool: &DbPool) -> Result<Vec<MeetingSession>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, created_at, duration, status, audio_path, transcript_path, audio_source, error_message, summary_path, template_id
         FROM meeting_sessions ORDER BY created_at DESC",
    )?;
    let sessions = stmt
        .query_map([], |row| row_to_session(row))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(sessions)
}

/// Lists meeting sessions with pagination support.
pub(crate) fn list_sessions_paginated(
    pool: &DbPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<MeetingSession>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, created_at, duration, status, audio_path, transcript_path, audio_source, error_message, summary_path, template_id
         FROM meeting_sessions ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
    )?;
    let sessions = stmt
        .query_map(params![limit, offset], |row| row_to_session(row))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(sessions)
}

/// Returns the total count of meeting sessions.
pub(crate) fn count_sessions(pool: &DbPool) -> Result<i64> {
    let conn = pool.get()?;
    conn.query_row("SELECT COUNT(*) FROM meeting_sessions", [], |row| row.get(0))
        .map_err(Into::into)
}

/// Deletes a meeting session record from the database.
pub(crate) fn delete_session_record(pool: &DbPool, session_id: &str) -> Result<()> {
    let conn = pool.get()?;
    conn.execute(
        "DELETE FROM meeting_sessions WHERE id = ?1",
        params![session_id],
    )?;
    Ok(())
}

/// Updates the title of a meeting session in the database.
pub(crate) fn update_session_title(
    pool: &DbPool,
    session_id: &str,
    title: &str,
) -> Result<()> {
    let conn = pool.get()?;
    let rows = conn.execute(
        "UPDATE meeting_sessions SET title = ?1 WHERE id = ?2",
        params![title, session_id],
    )?;
    if rows == 0 {
        return Err(anyhow::anyhow!("Session not found: {}", session_id));
    }
    Ok(())
}

/// Updates the template_id of a meeting session.
pub(crate) fn update_session_template_id(
    pool: &DbPool,
    session_id: &str,
    template_id: &str,
) -> Result<()> {
    let conn = pool.get()?;
    let rows = conn.execute(
        "UPDATE meeting_sessions SET template_id = ?1 WHERE id = ?2",
        params![template_id, session_id],
    )?;
    if rows == 0 {
        return Err(anyhow::anyhow!("Session not found: {}", session_id));
    }
    Ok(())
}

/// Updates the summary path of a meeting session.
pub(crate) fn update_session_summary_path(
    pool: &DbPool,
    session_id: &str,
    summary_path: &str,
) -> Result<()> {
    let conn = pool.get()?;
    let rows = conn.execute(
        "UPDATE meeting_sessions SET summary_path = ?1 WHERE id = ?2",
        params![summary_path, session_id],
    )?;
    if rows == 0 {
        return Err(anyhow::anyhow!("Session not found: {}", session_id));
    }
    Ok(())
}

/// Updates audio_path and duration for a meeting session.
pub(crate) fn update_session_audio(
    pool: &DbPool,
    session_id: &str,
    audio_path: &str,
    duration: i64,
    status: &MeetingStatus,
) -> Result<()> {
    let conn = pool.get()?;
    conn.execute(
        "UPDATE meeting_sessions SET audio_path = ?1, duration = ?2, status = ?3 WHERE id = ?4",
        params![audio_path, duration, status_to_string(status), session_id],
    )?;
    Ok(())
}

/// Updates transcript_path and status for a meeting session.
pub(crate) fn update_session_transcript(
    pool: &DbPool,
    session_id: &str,
    transcript_path: &str,
    status: &MeetingStatus,
) -> Result<()> {
    let conn = pool.get()?;
    conn.execute(
        "UPDATE meeting_sessions SET transcript_path = ?1, status = ?2 WHERE id = ?3",
        params![transcript_path, status_to_string(status), session_id],
    )?;
    Ok(())
}

/// Finds sessions in Recording or Interrupted status (for recovery on restart).
pub(crate) fn find_interrupted_sessions(pool: &DbPool) -> Result<Vec<MeetingSession>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, created_at, duration, status, audio_path, transcript_path, audio_source, error_message, summary_path, template_id
         FROM meeting_sessions WHERE status IN ('recording', 'interrupted') ORDER BY created_at DESC",
    )?;
    let sessions = stmt
        .query_map([], |row| row_to_session(row))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(sessions)
}

// --- Notes -----------------------------------------------------------------

fn row_to_note(row: &rusqlite::Row) -> rusqlite::Result<MeetingNote> {
    Ok(MeetingNote {
        id: row.get(0)?,
        session_id: row.get(1)?,
        timestamp_seconds: row.get(2)?,
        content: row.get(3)?,
        author: row.get(4)?,
        created_at: row.get(5)?,
    })
}

/// Inserts a new note for a meeting session.
pub(crate) fn insert_note(pool: &DbPool, note: &MeetingNote) -> Result<()> {
    let conn = pool.get()?;
    conn.execute(
        "INSERT INTO meeting_notes (id, session_id, timestamp_seconds, content, author, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            note.id,
            note.session_id,
            note.timestamp_seconds,
            note.content,
            note.author,
            note.created_at,
        ],
    )?;
    Ok(())
}

/// Returns the notes attached to a session, oldest first (timestamp then created_at).
pub(crate) fn list_notes_by_session(
    pool: &DbPool,
    session_id: &str,
) -> Result<Vec<MeetingNote>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, session_id, timestamp_seconds, content, author, created_at
         FROM meeting_notes
         WHERE session_id = ?1
         ORDER BY timestamp_seconds ASC, created_at ASC",
    )?;
    let notes = stmt
        .query_map(params![session_id], |row| row_to_note(row))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(notes)
}

/// Deletes a single note by id. Returns `true` if a row was removed.
pub(crate) fn delete_note(pool: &DbPool, note_id: &str) -> Result<bool> {
    let conn = pool.get()?;
    let rows = conn.execute(
        "DELETE FROM meeting_notes WHERE id = ?1",
        params![note_id],
    )?;
    Ok(rows > 0)
}
