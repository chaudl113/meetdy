//! Database initialization, migrations, and CRUD operations for meeting sessions.

use anyhow::Result;
use log::{debug, info};
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
use std::path::PathBuf;

use super::models::{
    ActionItem, ActionItemStatus, KeyPoint, MeetingNote, Participant, Tag, TranscriptSegment,
};

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
    // Action items extracted from a meeting (or added manually).
    M::up(
        "CREATE TABLE IF NOT EXISTS meeting_action_items (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            task TEXT NOT NULL,
            assignee TEXT,
            due_date TEXT,
            status TEXT NOT NULL DEFAULT 'todo',
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (session_id) REFERENCES meeting_sessions(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_meeting_action_items_session_id
            ON meeting_action_items(session_id);",
    ),
    // Key points / discussion topics. `category` groups bullets under a
    // heading (e.g. 'Regulatory/TSC approval'). NULL category = ungrouped.
    M::up(
        "CREATE TABLE IF NOT EXISTS meeting_key_points (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            category TEXT,
            content TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (session_id) REFERENCES meeting_sessions(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_meeting_key_points_session_id
            ON meeting_key_points(session_id);",
    ),
    // Meeting participants. `role` is free-form (e.g. 'Marketing', 'Sales').
    M::up(
        "CREATE TABLE IF NOT EXISTS meeting_participants (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            name TEXT NOT NULL,
            role TEXT,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (session_id) REFERENCES meeting_sessions(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_meeting_participants_session_id
            ON meeting_participants(session_id);",
    ),
    // Tags for filtering/organizing meetings.
    M::up(
        "CREATE TABLE IF NOT EXISTS meeting_tags (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            label TEXT NOT NULL,
            color TEXT,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (session_id) REFERENCES meeting_sessions(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_meeting_tags_session_id
            ON meeting_tags(session_id);
        CREATE INDEX IF NOT EXISTS idx_meeting_tags_label
            ON meeting_tags(label);",
    ),
    // Transcript segments for speaker diarization.
    M::up(
        "CREATE TABLE IF NOT EXISTS transcript_segments (
            id TEXT PRIMARY KEY,
            meeting_id TEXT NOT NULL,
            start_ms INTEGER NOT NULL,
            end_ms INTEGER NOT NULL,
            text TEXT NOT NULL,
            speaker_id TEXT,
            sequence INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (meeting_id) REFERENCES meeting_sessions(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_transcript_segments_meeting_id
            ON transcript_segments(meeting_id);
        CREATE INDEX IF NOT EXISTS idx_transcript_segments_sequence
            ON transcript_segments(meeting_id, sequence);",
    ),
    // Speaker color assignment for participants.
    M::up(
        "ALTER TABLE meeting_participants ADD COLUMN color_index INTEGER NOT NULL DEFAULT -1;",
    ),
    // STT metadata is pinned to each session so retry/regeneration does not
    // depend on global settings changed after the recording.
    M::up(
        "ALTER TABLE meeting_sessions ADD COLUMN stt_engine TEXT NOT NULL DEFAULT 'whisper';
         ALTER TABLE meeting_sessions ADD COLUMN funasr_base_url TEXT;
         ALTER TABLE meeting_sessions ADD COLUMN funasr_model TEXT;
         ALTER TABLE meeting_sessions ADD COLUMN transcription_language TEXT;",
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
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;

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

/// Gets a connection to the meetings database.
pub(crate) fn get_connection(db_path: &PathBuf) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
    Ok(conn)
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
pub(crate) fn insert_note(db_path: &PathBuf, note: &MeetingNote) -> Result<()> {
    let conn = get_connection(db_path)?;
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
    db_path: &PathBuf,
    session_id: &str,
) -> Result<Vec<MeetingNote>> {
    let conn = get_connection(db_path)?;
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
pub(crate) fn delete_note(db_path: &PathBuf, note_id: &str) -> Result<bool> {
    let conn = get_connection(db_path)?;
    let rows = conn.execute("DELETE FROM meeting_notes WHERE id = ?1", params![note_id])?;
    Ok(rows > 0)
}

// --- Action Items ----------------------------------------------------------

pub(crate) fn action_item_status_to_string(s: &ActionItemStatus) -> &'static str {
    match s {
        ActionItemStatus::Todo => "todo",
        ActionItemStatus::InProgress => "in_progress",
        ActionItemStatus::Done => "done",
        ActionItemStatus::Blocked => "blocked",
    }
}

pub(crate) fn string_to_action_item_status(s: &str) -> ActionItemStatus {
    match s {
        "in_progress" => ActionItemStatus::InProgress,
        "done" => ActionItemStatus::Done,
        "blocked" => ActionItemStatus::Blocked,
        _ => ActionItemStatus::Todo,
    }
}

fn row_to_action_item(row: &rusqlite::Row) -> rusqlite::Result<ActionItem> {
    let status_str: String = row.get(5)?;
    Ok(ActionItem {
        id: row.get(0)?,
        session_id: row.get(1)?,
        task: row.get(2)?,
        assignee: row.get(3)?,
        due_date: row.get(4)?,
        status: string_to_action_item_status(&status_str),
        sort_order: row.get(6)?,
        created_at: row.get(7)?,
    })
}

pub(crate) fn insert_action_item(db_path: &PathBuf, item: &ActionItem) -> Result<()> {
    let conn = get_connection(db_path)?;
    conn.execute(
        "INSERT INTO meeting_action_items
            (id, session_id, task, assignee, due_date, status, sort_order, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            item.id,
            item.session_id,
            item.task,
            item.assignee,
            item.due_date,
            action_item_status_to_string(&item.status),
            item.sort_order,
            item.created_at,
        ],
    )?;
    Ok(())
}

pub(crate) fn list_action_items_by_session(
    db_path: &PathBuf,
    session_id: &str,
) -> Result<Vec<ActionItem>> {
    let conn = get_connection(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, session_id, task, assignee, due_date, status, sort_order, created_at
         FROM meeting_action_items
         WHERE session_id = ?1
         ORDER BY sort_order ASC, created_at ASC",
    )?;
    let items = stmt
        .query_map(params![session_id], row_to_action_item)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(items)
}

pub(crate) fn update_action_item(db_path: &PathBuf, item: &ActionItem) -> Result<()> {
    let conn = get_connection(db_path)?;
    let rows = conn.execute(
        "UPDATE meeting_action_items
         SET task = ?1, assignee = ?2, due_date = ?3, status = ?4, sort_order = ?5
         WHERE id = ?6",
        params![
            item.task,
            item.assignee,
            item.due_date,
            action_item_status_to_string(&item.status),
            item.sort_order,
            item.id,
        ],
    )?;
    if rows == 0 {
        return Err(anyhow::anyhow!("Action item not found: {}", item.id));
    }
    Ok(())
}

pub(crate) fn delete_action_item(db_path: &PathBuf, id: &str) -> Result<bool> {
    let conn = get_connection(db_path)?;
    let rows = conn.execute(
        "DELETE FROM meeting_action_items WHERE id = ?1",
        params![id],
    )?;
    Ok(rows > 0)
}

pub(crate) fn delete_action_items_by_session(db_path: &PathBuf, session_id: &str) -> Result<()> {
    let conn = get_connection(db_path)?;
    conn.execute(
        "DELETE FROM meeting_action_items WHERE session_id = ?1",
        params![session_id],
    )?;
    Ok(())
}

// --- Key Points ------------------------------------------------------------

fn row_to_key_point(row: &rusqlite::Row) -> rusqlite::Result<KeyPoint> {
    Ok(KeyPoint {
        id: row.get(0)?,
        session_id: row.get(1)?,
        category: row.get(2)?,
        content: row.get(3)?,
        sort_order: row.get(4)?,
        created_at: row.get(5)?,
    })
}

pub(crate) fn insert_key_point(db_path: &PathBuf, kp: &KeyPoint) -> Result<()> {
    let conn = get_connection(db_path)?;
    conn.execute(
        "INSERT INTO meeting_key_points
            (id, session_id, category, content, sort_order, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            kp.id,
            kp.session_id,
            kp.category,
            kp.content,
            kp.sort_order,
            kp.created_at,
        ],
    )?;
    Ok(())
}

pub(crate) fn list_key_points_by_session(
    db_path: &PathBuf,
    session_id: &str,
) -> Result<Vec<KeyPoint>> {
    let conn = get_connection(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, session_id, category, content, sort_order, created_at
         FROM meeting_key_points
         WHERE session_id = ?1
         ORDER BY sort_order ASC, created_at ASC",
    )?;
    let items = stmt
        .query_map(params![session_id], row_to_key_point)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(items)
}

pub(crate) fn delete_key_points_by_session(db_path: &PathBuf, session_id: &str) -> Result<()> {
    let conn = get_connection(db_path)?;
    conn.execute(
        "DELETE FROM meeting_key_points WHERE session_id = ?1",
        params![session_id],
    )?;
    Ok(())
}

// --- Participants ----------------------------------------------------------

fn row_to_participant(row: &rusqlite::Row) -> rusqlite::Result<Participant> {
    Ok(Participant {
        id: row.get(0)?,
        session_id: row.get(1)?,
        name: row.get(2)?,
        role: row.get(3)?,
        sort_order: row.get(4)?,
        created_at: row.get(5)?,
        color_index: -1,
    })
}

pub(crate) fn insert_participant(db_path: &PathBuf, p: &Participant) -> Result<()> {
    let conn = get_connection(db_path)?;
    conn.execute(
        "INSERT INTO meeting_participants
            (id, session_id, name, role, sort_order, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            p.id,
            p.session_id,
            p.name,
            p.role,
            p.sort_order,
            p.created_at,
        ],
    )?;
    Ok(())
}

pub(crate) fn list_participants_by_session(
    db_path: &PathBuf,
    session_id: &str,
) -> Result<Vec<Participant>> {
    let conn = get_connection(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, session_id, name, role, sort_order, created_at
         FROM meeting_participants
         WHERE session_id = ?1
         ORDER BY sort_order ASC, created_at ASC",
    )?;
    let items = stmt
        .query_map(params![session_id], row_to_participant)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(items)
}

pub(crate) fn update_participant(db_path: &PathBuf, p: &Participant) -> Result<()> {
    let conn = get_connection(db_path)?;
    let rows = conn.execute(
        "UPDATE meeting_participants
         SET name = ?1, role = ?2, sort_order = ?3
         WHERE id = ?4",
        params![p.name, p.role, p.sort_order, p.id],
    )?;
    if rows == 0 {
        return Err(anyhow::anyhow!("Participant not found: {}", p.id));
    }
    Ok(())
}

pub(crate) fn delete_participant(db_path: &PathBuf, id: &str) -> Result<bool> {
    let conn = get_connection(db_path)?;
    let rows = conn.execute(
        "DELETE FROM meeting_participants WHERE id = ?1",
        params![id],
    )?;
    Ok(rows > 0)
}

pub(crate) fn delete_participants_by_session(db_path: &PathBuf, session_id: &str) -> Result<()> {
    let conn = get_connection(db_path)?;
    conn.execute(
        "DELETE FROM meeting_participants WHERE session_id = ?1",
        params![session_id],
    )?;
    Ok(())
}

// --- Tags ------------------------------------------------------------------

fn row_to_tag(row: &rusqlite::Row) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: row.get(0)?,
        session_id: row.get(1)?,
        label: row.get(2)?,
        color: row.get(3)?,
        created_at: row.get(4)?,
    })
}

pub(crate) fn insert_tag(db_path: &PathBuf, t: &Tag) -> Result<()> {
    let conn = get_connection(db_path)?;
    conn.execute(
        "INSERT INTO meeting_tags (id, session_id, label, color, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![t.id, t.session_id, t.label, t.color, t.created_at],
    )?;
    Ok(())
}

pub(crate) fn list_tags_by_session(db_path: &PathBuf, session_id: &str) -> Result<Vec<Tag>> {
    let conn = get_connection(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, session_id, label, color, created_at
         FROM meeting_tags
         WHERE session_id = ?1
         ORDER BY label ASC",
    )?;
    let items = stmt
        .query_map(params![session_id], row_to_tag)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(items)
}

pub(crate) fn delete_tag(db_path: &PathBuf, id: &str) -> Result<bool> {
    let conn = get_connection(db_path)?;
    let rows = conn.execute("DELETE FROM meeting_tags WHERE id = ?1", params![id])?;
    Ok(rows > 0)
}

/// Returns all distinct tag labels across all sessions (for filter chips).
pub(crate) fn list_all_tag_labels(db_path: &PathBuf) -> Result<Vec<String>> {
    let conn = get_connection(db_path)?;
    let mut stmt = conn.prepare("SELECT DISTINCT label FROM meeting_tags ORDER BY label ASC")?;
    let labels = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(labels)
}

// --- Transcript Segments ---------------------------------------------------

/// Insert a transcript segment into the database.
pub fn insert_transcript_segment(conn: &Connection, segment: &TranscriptSegment) -> Result<()> {
    conn.execute(
        "INSERT INTO transcript_segments (id, meeting_id, start_ms, end_ms, text, speaker_id, sequence, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            segment.id,
            segment.meeting_id,
            segment.start_ms,
            segment.end_ms,
            segment.text,
            segment.speaker_id,
            segment.sequence,
            segment.created_at,
        ],
    )?;
    Ok(())
}

/// List all transcript segments for a meeting, ordered by sequence.
pub fn list_transcript_segments(
    conn: &Connection,
    meeting_id: &str,
) -> Result<Vec<TranscriptSegment>> {
    let mut stmt = conn.prepare(
        "SELECT id, meeting_id, start_ms, end_ms, text, speaker_id, sequence, created_at
         FROM transcript_segments
         WHERE meeting_id = ?1
         ORDER BY sequence ASC",
    )?;
    let segments = stmt
        .query_map(params![meeting_id], |row| {
            Ok(TranscriptSegment {
                id: row.get(0)?,
                meeting_id: row.get(1)?,
                start_ms: row.get(2)?,
                end_ms: row.get(3)?,
                text: row.get(4)?,
                speaker_id: row.get(5)?,
                sequence: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(segments)
}

/// Update the speaker_id of a transcript segment.
pub fn update_segment_speaker(
    conn: &Connection,
    segment_id: &str,
    speaker_id: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE transcript_segments SET speaker_id = ?1 WHERE id = ?2",
        params![speaker_id, segment_id],
    )?;
    Ok(())
}

/// Update the color_index of a participant.
pub fn update_participant_color_index(
    conn: &Connection,
    participant_id: &str,
    color_index: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE meeting_participants SET color_index = ?1 WHERE id = ?2",
        params![color_index, participant_id],
    )?;
    Ok(())
}
