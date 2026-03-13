//! Meeting session management for Meeting Mode.
//!
//! This module provides the core data structures and manager for meeting sessions,
//! which are completely separate from the existing Quick Dictation functionality.
//!
//! ## Module Structure
//! - `models` - Data types: MeetingStatus, AudioSourceType, MeetingSession
//! - `wav_writer` - Thread-safe WAV file writer with timeout-based finalization
//! - `db` - Database initialization, migrations, and CRUD operations
//! - `manager` - Core MeetingSessionManager implementation (recording, transcription, lifecycle)

// Private internal modules (db is pub(crate) so tests can access it)
pub(crate) mod db;
mod manager;
mod models;
mod wav_writer;

// Re-export public types
pub use models::{AudioSourceType, MeetingSession, MeetingStatus};

// Re-export the manager
pub use manager::MeetingSessionManager;

// Re-export internal types needed by other modules (may not all be used yet)
#[allow(unused_imports)]
pub(crate) use models::MeetingManagerState;
#[allow(unused_imports)]
pub(crate) use wav_writer::WavWriterHandle;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
