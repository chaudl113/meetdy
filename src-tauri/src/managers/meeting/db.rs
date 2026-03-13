#![allow(dead_code)]
//! Database initialization, migrations, and CRUD operations for meeting sessions.

use anyhow::Result;
use log::{debug, info};
use rusqlite::{params, Connection, OptionalExtension};
use rusqlite_migration::{Migrations, M};
use std::path::PathBuf;

use super::models::{AudioSourceType, MeetingSession, MeetingStatus};

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
];

/// Initialize the meeting sessions database and run any pending migrations.
///
/// This function opens (or creates) the database at the specified path and
/// applies all pending migrations. It follows the same pattern as HistoryManager.
///
/// # Arguments
/// * `db_path` - Path to the SQLite database file
///
/// # Returns
/// * `Ok(())` if the database was initialized successfully
/// * `Err` if the database could not be opened or migrations failed
pub fn init_meeting_database(db_path: &PathBuf) -> Result<()> {
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

    Ok(())
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

/// Gets a connection to the meetings database.
pub(crate) fn get_connection(db_path: &PathBuf) -> Result<Connection> {
    Ok(Connection::open(db_path)?)
}

/// Creates a new session record in the database.
pub(crate) fn insert_session(db_path: &PathBuf, session: &MeetingSession) -> Result<()> {
    let conn = get_connection(db_path)?;
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
pub(crate) fn get_session(db_path: &PathBuf, session_id: &str) -> Result<Option<MeetingSession>> {
    let conn = get_connection(db_path)?;
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
    db_path: &PathBuf,
    session_id: &str,
    status: &MeetingStatus,
) -> Result<()> {
    let conn = get_connection(db_path)?;
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
    db_path: &PathBuf,
    session_id: &str,
    status: &MeetingStatus,
    error_message: &str,
) -> Result<()> {
    let conn = get_connection(db_path)?;
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
pub(crate) fn list_sessions(db_path: &PathBuf) -> Result<Vec<MeetingSession>> {
    let conn = get_connection(db_path)?;
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

/// Deletes a meeting session record from the database.
pub(crate) fn delete_session_record(db_path: &PathBuf, session_id: &str) -> Result<()> {
    let conn = get_connection(db_path)?;
    conn.execute(
        "DELETE FROM meeting_sessions WHERE id = ?1",
        params![session_id],
    )?;
    Ok(())
}

/// Updates the title of a meeting session in the database.
pub(crate) fn update_session_title(
    db_path: &PathBuf,
    session_id: &str,
    title: &str,
) -> Result<()> {
    let conn = get_connection(db_path)?;
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
    db_path: &PathBuf,
    session_id: &str,
    template_id: &str,
) -> Result<()> {
    let conn = get_connection(db_path)?;
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
    db_path: &PathBuf,
    session_id: &str,
    summary_path: &str,
) -> Result<()> {
    let conn = get_connection(db_path)?;
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
    db_path: &PathBuf,
    session_id: &str,
    audio_path: &str,
    duration: i64,
    status: &MeetingStatus,
) -> Result<()> {
    let conn = get_connection(db_path)?;
    conn.execute(
        "UPDATE meeting_sessions SET audio_path = ?1, duration = ?2, status = ?3 WHERE id = ?4",
        params![audio_path, duration, status_to_string(status), session_id],
    )?;
    Ok(())
}

/// Updates transcript_path and status for a meeting session.
pub(crate) fn update_session_transcript(
    db_path: &PathBuf,
    session_id: &str,
    transcript_path: &str,
    status: &MeetingStatus,
) -> Result<()> {
    let conn = get_connection(db_path)?;
    conn.execute(
        "UPDATE meeting_sessions SET transcript_path = ?1, status = ?2 WHERE id = ?3",
        params![transcript_path, status_to_string(status), session_id],
    )?;
    Ok(())
}

/// Finds sessions in Recording or Interrupted status (for recovery on restart).
pub(crate) fn find_interrupted_sessions(db_path: &PathBuf) -> Result<Vec<MeetingSession>> {
    let conn = get_connection(db_path)?;
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
