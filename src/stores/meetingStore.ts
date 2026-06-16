import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import type {
  AudioSourceType,
  MeetingNote,
  MeetingSession,
  MeetingStatus,
  Participant,
} from "@/bindings";
import { commands } from "@/bindings";
import { useRecordingConfigStore } from "./recordingConfigStore";
import { useSettingsStore } from "./settingsStore";

// Patterns of known hallucinated/junk transcripts (commonly produced by Whisper
// on silence/noise — e.g. YouTube subscribe outros from Vietnamese channels).
const TRANSCRIPT_NOISE_PATTERNS: RegExp[] = [
  /ghiền\s*mì\s*gõ/i,
  /hãy\s*subscribe\s*cho\s*kênh/i,
  /đăng\s*k(ý|y)\s*kênh/i,
  /không\s*bỏ\s*lỡ\s*những\s*video/i,
  /like\s*và\s*subscribe/i,
  /nhấn\s*chuông\s*thông\s*báo/i,
  /cảm\s*ơn\s*các\s*bạn\s*đã\s*(xem|theo\s*dõi)/i,
  /hẹn\s*gặp\s*lại\s*(các\s*bạn\s*)?(ở|trong)\s*(video|clip)/i,
];

const isNoiseTranscript = (text: string): boolean => {
  const trimmed = text.trim();
  if (!trimmed) return true;
  return TRANSCRIPT_NOISE_PATTERNS.some((re) => re.test(trimmed));
};

const stripNoiseFromText = (text: string): string => {
  if (!text) return text;
  return text
    .split(/\n+/)
    .map((line) => (isNoiseTranscript(line) ? "" : line))
    .filter((line) => line.length > 0)
    .join("\n");
};

async function notifyIfEnabled(
  enabled: boolean,
  title: string,
  body: string,
): Promise<void> {
  if (!enabled) return;
  try {
    let granted = await isPermissionGranted();
    if (!granted) {
      const perm = await requestPermission();
      granted = perm === "granted";
    }
    if (granted) sendNotification({ title, body });
  } catch (err) {
    console.warn("notification failed:", err);
  }
}

/**
 * Formats a duration in seconds to HH:MM:SS format
 * @param seconds - The duration in seconds
 * @returns Formatted string in HH:MM:SS format
 */
export function formatDuration(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  const pad = (n: number) => n.toString().padStart(2, "0");

  return `${pad(hours)}:${pad(minutes)}:${pad(secs)}`;
}

/**
 * Real-time audio statistics emitted from the backend at ~10Hz while a
 * meeting is recording. `null` when no recording is active.
 */
export interface MeetingAudioStats {
  session_id: string;
  rms: number;
  peak: number;
  snr_db: number;
  noise_floor_db: number;
  audio_source: AudioSourceType;
}

export interface MeetingLiveTranscript {
  session_id: string;
  text: string;
  chunk_text: string;
  is_final: boolean;
  speaker_id: string | null;
  start_ms: number;
  end_ms: number;
}

export interface LiveTranscriptSegment {
  text: string;
  offset: number;
  startMs?: number;
  endMs?: number;
  speakerId: string | null;
}

interface MeetingStore {
  // State
  sessionStatus: MeetingStatus;
  currentSession: MeetingSession | null;
  sessions: MeetingSession[];
  recordingDuration: number;
  isPaused: boolean;
  isLoading: boolean;
  error: string | null;

  // Latest live audio statistics for the active recording.
  audioStats: MeetingAudioStats | null;

  // Incremental transcript emitted while recording.
  liveTranscript: string;
  liveTranscriptSegments: LiveTranscriptSegment[];

  // Participants for the active recording session.
  participants: Participant[];
  activeSpeakerId: string | null;

  // Notes for the currently displayed session (recording or completed).
  notes: MeetingNote[];

  // STT engine error (e.g. Soniox connection failed / bad API key).
  sttError: string | null;
  clearSttError: () => void;

  // Actions
  startMeeting: (
    audioSource?: AudioSourceType,
    templateId?: string,
    sttEngine?: string,
    sonioxApiKey?: string,
    funasrBaseUrl?: string,
    funasrModel?: string,
  ) => Promise<void>;
  stopMeeting: () => Promise<void>;
  pauseMeeting: () => Promise<void>;
  resumeMeeting: () => Promise<void>;
  retryTranscription: () => Promise<void>;
  updateTitle: (title: string) => Promise<void>;
  refreshStatus: () => Promise<void>;
  fetchSessions: () => Promise<void>;
  clearAllSessions: () => Promise<void>;
  clearError: () => void;

  loadParticipants: (sessionId: string) => Promise<void>;
  setActiveSpeakerId: (id: string | null) => void;
  addParticipantToStore: (participant: Participant) => void;

  // Notes actions
  loadNotes: (sessionId: string) => Promise<void>;
  addNote: (timestampSeconds: number, content: string) => Promise<void>;
  deleteNote: (noteId: string) => Promise<void>;

  // Internal setters
  setSessionStatus: (status: MeetingStatus) => void;
  setCurrentSession: (session: MeetingSession | null) => void;
  setSessions: (sessions: MeetingSession[]) => void;
  setRecordingDuration: (duration: number) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;

  // Internal timer
  _durationInterval: ReturnType<typeof setInterval> | null;
  _startDurationTimer: () => void;
  _stopDurationTimer: () => void;

  // Event listener management
  _initId: number;
  _eventUnlisteners: UnlistenFn[];
  _visibilityHandler: (() => void) | null;
  initializeEventListeners: () => Promise<void>;
  cleanupEventListeners: () => void;
}

export const useMeetingStore = create<MeetingStore>()(
  subscribeWithSelector((set, get) => ({
    // Initial state
    sessionStatus: "idle",
    currentSession: null,
    sessions: [],
    recordingDuration: 0,
    isPaused: false,
    isLoading: false,
    error: null,
    notes: [],
    sttError: null,
    audioStats: null,
    liveTranscript: "",
    liveTranscriptSegments: [],
    participants: [],
    activeSpeakerId: null,

    // Internal timer reference
    _durationInterval: null,

    // Event listener management
    _initId: 0,
    _eventUnlisteners: [],
    _visibilityHandler: null,

    // Internal setters
    setSessionStatus: (sessionStatus) => set({ sessionStatus }),
    setCurrentSession: (currentSession) => set({ currentSession }),
    setSessions: (sessions) => set({ sessions }),
    setRecordingDuration: (recordingDuration) => set({ recordingDuration }),
    setLoading: (isLoading) => set({ isLoading }),
    setError: (error) => set({ error }),

    // Clear error
    clearError: () => set({ error: null }),

    // Start duration timer
    _startDurationTimer: () => {
      const { _stopDurationTimer } = get();
      // Stop any existing timer first
      _stopDurationTimer();

      // Reset duration to 0
      set({ recordingDuration: 0 });

      // Start new timer that increments every second
      const interval = setInterval(() => {
        set((state) =>
          state.isPaused
            ? state
            : { recordingDuration: state.recordingDuration + 1 },
        );
      }, 1000);

      set({ _durationInterval: interval });
    },

    // Stop duration timer
    _stopDurationTimer: () => {
      const { _durationInterval } = get();
      if (_durationInterval) {
        clearInterval(_durationInterval);
        set({ _durationInterval: null });
      }
    },

    // Start a new meeting session
    startMeeting: async (audioSource?: AudioSourceType, templateId?: string, sttEngine?: string, sonioxApiKey?: string, funasrBaseUrl?: string, funasrModel?: string) => {
      const {
        setLoading,
        setError,
        setSessionStatus,
        setCurrentSession,
        _startDurationTimer,
      } = get();

      setLoading(true);
      setError(null);

      try {
        const result = await commands.startMeetingSession(
          audioSource ?? null,
          templateId ?? null,
          sttEngine ?? null,
          sonioxApiKey ?? null,
          funasrBaseUrl ?? null,
          funasrModel ?? null,
        );
        if (result.status === "ok") {
          const session = result.data as MeetingSession;
          setCurrentSession(session);
          setSessionStatus("recording");
          // Fresh session — clear stale notes and any previous stt error.
          set({ notes: [], isPaused: false, sttError: null });
          _startDurationTimer();
        } else {
          setError(result.error);
        }
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to start meeting";
        setError(errorMessage);
      } finally {
        setLoading(false);
      }
    },

    // Stop the current meeting session
    stopMeeting: async () => {
      const { setLoading, setError, _stopDurationTimer } = get();

      setLoading(true);
      setError(null);

      try {
        const result = await commands.stopMeetingSession();
        if (result.status === "ok") {
          set({ isPaused: false });
          _stopDurationTimer();
        } else {
          setError(result.error);
        }
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to stop meeting";
        setError(errorMessage);
      } finally {
        setLoading(false);
      }
    },

    pauseMeeting: async () => {
      const { setError } = get();
      setError(null);
      const result = await commands.pauseMeetingSession();
      if (result.status === "ok") {
        set({ isPaused: true });
      } else {
        setError(result.error);
      }
    },

    resumeMeeting: async () => {
      const { setError } = get();
      setError(null);
      const result = await commands.resumeMeetingSession();
      if (result.status === "ok") {
        set({ isPaused: false });
      } else {
        setError(result.error);
      }
    },

    // Refresh the current meeting status from backend
    refreshStatus: async () => {
      const { setSessionStatus, setCurrentSession, setError } = get();

      try {
        // Get current meeting details
        const meetingResult = await commands.getCurrentMeeting();
        if (meetingResult.status === "ok") {
          const session = meetingResult.data as MeetingSession | null;
          if (session) {
            setCurrentSession(session);
            setSessionStatus(session.status);
          } else {
            setCurrentSession(null);
            setSessionStatus("idle");
          }
        } else {
          setError(meetingResult.error);
        }
      } catch (err) {
        const errorMessage =
          err instanceof Error
            ? err.message
            : "Failed to refresh meeting status";
        setError(errorMessage);
      }
    },

    // Retry / regenerate transcription for a meeting session with saved audio.
    retryTranscription: async () => {
      const { currentSession, setLoading, setError, setSessionStatus } = get();

      // Validate we have a current session
      if (!currentSession) {
        setError("No meeting session to retry");
        return;
      }

      if (!["failed", "completed", "interrupted"].includes(currentSession.status)) {
        setError("Can only regenerate transcript after recording has stopped");
        return;
      }

      setLoading(true);
      setError(null);

      try {
        const result = await commands.retryTranscription(currentSession.id);
        if (result.status === "ok") {
          setSessionStatus("processing");
        } else {
          setError(result.error);
        }
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to retry transcription";
        setError(errorMessage);
      } finally {
        setLoading(false);
      }
    },

    // Update the title of the current meeting session
    updateTitle: async (title: string) => {
      const { currentSession, setCurrentSession, setError } = get();

      // Validate we have a current session
      if (!currentSession) {
        setError("No meeting session to update");
        return;
      }

      // Validate title is not empty
      if (!title.trim()) {
        setError("Title cannot be empty");
        return;
      }

      try {
        const result = await commands.updateMeetingTitle(
          currentSession.id,
          title,
        );
        if (result.status === "ok") {
          // Optimistically update local state
          setCurrentSession({
            ...currentSession,
            title: title,
          });
        } else {
          setError(result.error);
        }
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to update title";
        setError(errorMessage);
      }
    },

    // Fetch all meeting sessions from backend
    fetchSessions: async () => {
      const { setSessions, setError } = get();

      try {
        const result = await commands.listMeetingSessions();
        if (result.status === "ok") {
          setSessions(result.data);
        } else {
          setError(result.error);
        }
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to fetch sessions";
        setError(errorMessage);
      }
    },

    clearAllSessions: async () => {
      const { setLoading, setError } = get();
      setLoading(true);
      setError(null);
      try {
        const result = await commands.clearAllMeetingSessions();
        if (result.status === "ok") {
          set({
            sessions: [],
            currentSession: null,
            sessionStatus: "idle",
            liveTranscript: "",
            liveTranscriptSegments: [],
            notes: [],
            participants: [],
            activeSpeakerId: null,
          });
        } else {
          setError(result.error);
        }
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to clear history";
        setError(errorMessage);
      } finally {
        setLoading(false);
      }
    },

    // --- Notes -----------------------------------------------------------

    // Loads notes for the given session into the store.
    loadNotes: async (sessionId: string) => {
      const { setError } = get();
      try {
        const result = await commands.listMeetingNotes(sessionId);
        if (result.status === "ok") {
          set({ notes: result.data });
        } else {
          setError(result.error);
        }
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to load notes";
        setError(errorMessage);
      }
    },

    // Adds a note to the current session (no-op if no session).
    addNote: async (timestampSeconds: number, content: string) => {
      const { currentSession, setError } = get();
      if (!currentSession) {
        setError("No active meeting session for note");
        return;
      }
      const trimmed = content.trim();
      if (!trimmed) return;

      try {
        const result = await commands.addMeetingNote(
          currentSession.id,
          Math.max(0, Math.floor(timestampSeconds)),
          trimmed,
        );
        if (result.status === "ok") {
          set((state) => ({ notes: [...state.notes, result.data] }));
        } else {
          setError(result.error);
        }
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to add note";
        setError(errorMessage);
      }
    },

    // Deletes a note from the current list and the backend.
    deleteNote: async (noteId: string) => {
      const { setError } = get();
      try {
        const result = await commands.deleteMeetingNote(noteId);
        if (result.status === "ok") {
          set((state) => ({ notes: state.notes.filter((n) => n.id !== noteId) }));
        } else {
          setError(result.error);
        }
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to delete note";
        setError(errorMessage);
      }
    },

    loadParticipants: async (sessionId: string) => {
      try {
        const result = await commands.listMeetingParticipants(sessionId);
        if (result.status === "ok") {
          set({ participants: result.data });
        }
      } catch (err) {
        console.warn("Failed to load participants:", err);
      }
    },

    setActiveSpeakerId: (activeSpeakerId) => set({ activeSpeakerId }),
    clearSttError: () => set({ sttError: null }),

    addParticipantToStore: (participant) =>
      set((state) => ({ participants: [...state.participants, participant] })),

    // Initialize event listeners for meeting_* events from backend
    initializeEventListeners: async () => {
      // Hydrate stt settings from persisted AppSettings so that keyboard
      // shortcuts work correctly even if StartMeeting screen was never opened.
      try {
        const settingsResult = await commands.getAppSettings();
        if (settingsResult.status === "ok") {
          const s = settingsResult.data;
          const store = useRecordingConfigStore.getState();
          if (s.meeting_stt_engine) {
            store.setSttEngine(s.meeting_stt_engine as import("./recordingConfigStore").SttEngine);
          }
          if (s.soniox_api_key) {
            store.setSonioxApiKey(s.soniox_api_key);
          }
          if (s.funasr_base_url) {
            store.setFunasrBaseUrl(s.funasr_base_url);
          }
          if (s.funasr_model) {
            store.setFunasrModel(s.funasr_model);
          }
        }
      } catch {
        // non-fatal: fall back to store defaults
      }

      const {
        setSessionStatus,
        setCurrentSession,
        setRecordingDuration,
        _startDurationTimer,
        _stopDurationTimer,
        refreshStatus,
        cleanupEventListeners,
      } = get();

      // Clean up any existing listeners first
      cleanupEventListeners();

      // Generate new init ID for abort pattern
      const initId = Date.now();
      set({ _initId: initId });

      const unlisteners: UnlistenFn[] = [];

      // Helper to check if this init is still valid
      const isValid = () => get()._initId === initId;

      try {
        // Listen for meeting_started event
        const startedUnlisten = await listen<MeetingSession>(
          "meeting_started",
          (event) => {
            if (!isValid()) return; // Abort if invalidated
            const session = event.payload;
            setCurrentSession(session);
            setSessionStatus("recording");
            // Reset notes / live stats / live transcript for the new session; load any
            // pre-existing notes.
            set({
              notes: [],
              audioStats: null,
              liveTranscript: "",
              liveTranscriptSegments: [],
              participants: [],
              activeSpeakerId: null,
            });
            get().loadNotes(session.id);
            get().loadParticipants(session.id);
            _startDurationTimer();
            // Sync duration if available
            if (session.duration !== undefined && session.duration !== null) {
              setRecordingDuration(session.duration);
            }
          },
        );

        if (!isValid()) {
          startedUnlisten(); // Cleanup if invalidated
          return;
        }
        unlisteners.push(startedUnlisten);

        // Listen for meeting_stopped event
        const stoppedUnlisten = await listen<MeetingSession>(
          "meeting_stopped",
          (event) => {
            if (!isValid()) return;
            const session = event.payload;
            setCurrentSession(session);
            _stopDurationTimer();
            // Clear live stats once recording has stopped.
            set({ audioStats: null, isPaused: false });
            // Sync duration
            if (session.duration !== undefined && session.duration !== null) {
              setRecordingDuration(session.duration);
            }
            // Status will transition to processing next
          },
        );

        if (!isValid()) {
          stoppedUnlisten();
          return;
        }
        unlisteners.push(stoppedUnlisten);

        // Listen for meeting_processing event
        const processingUnlisten = await listen<MeetingSession>(
          "meeting_processing",
          (event) => {
            if (!isValid()) return;
            const session = event.payload;
            setCurrentSession(session);
            setSessionStatus("processing");
            _stopDurationTimer();
            // Sync duration
            if (session.duration !== undefined && session.duration !== null) {
              setRecordingDuration(session.duration);
            }
          },
        );

        if (!isValid()) {
          processingUnlisten();
          return;
        }
        unlisteners.push(processingUnlisten);

        // Listen for meeting_completed event
        const completedUnlisten = await listen<MeetingSession>(
          "meeting_completed",
          (event) => {
            if (!isValid()) return;
            const session = event.payload;
            setCurrentSession(session);
            setSessionStatus("completed");
            _stopDurationTimer();
            // CRITICAL: Sync final duration from backend
            if (session.duration !== undefined && session.duration !== null) {
              setRecordingDuration(session.duration);
            }

            const settings = useSettingsStore.getState().settings;
            const autoSavePref = settings?.auto_save ?? true;
            const autoTranscribePref = settings?.auto_transcribe ?? false;
            const autoSummaryPref = settings?.auto_summary ?? true;
            const notifyCompletedPref = settings?.notify_completed ?? true;

            // Auto Save = false → discard meeting (delete persisted session + files)
            if (!autoSavePref) {
              commands
                .deleteMeetingSession(session.id)
                .catch((err) =>
                  console.warn("Auto-save off: failed to discard session", err),
                );
              return;
            }

            // Auto Transcribe = true and no transcript yet → force regenerate
            if (autoTranscribePref && !session.transcript_path) {
              commands
                .retryTranscription(session.id)
                .catch((err) =>
                  console.warn("Auto transcribe failed:", err),
                );
            }

            // Auto Summary (uses persisted setting)
            const recordingConfig = useRecordingConfigStore.getState();
            if (autoSummaryPref && session.transcript_path) {
              commands
                .generateMeetingSummary(
                  session.id,
                  recordingConfig.summaryLanguage === "auto"
                    ? null
                    : recordingConfig.summaryLanguage,
                )
                .catch((err) => {
                  console.error("Auto summary failed:", err);
                });
            }

            notifyIfEnabled(
              notifyCompletedPref,
              "Meeting completed",
              session.title || "Your recording has finished processing.",
            );
          },
        );

        if (!isValid()) {
          completedUnlisten();
          return;
        }
        unlisteners.push(completedUnlisten);

        const summaryGeneratedUnlisten = await listen<MeetingSession>(
          "meeting_summary_generated",
          (event) => {
            if (!isValid()) return;
            const currentSession = get().currentSession;
            if (currentSession?.id !== event.payload.id) return;
            setCurrentSession(event.payload);
          },
        );

        if (!isValid()) {
          summaryGeneratedUnlisten();
          return;
        }
        unlisteners.push(summaryGeneratedUnlisten);

        // Listen for meeting_failed event
        const failedUnlisten = await listen<MeetingSession>(
          "meeting_failed",
          (event) => {
            if (!isValid()) return;
            const session = event.payload;
            setCurrentSession(session);
            setSessionStatus("failed");
            _stopDurationTimer();
            // Sync partial duration if available
            if (session.duration !== undefined && session.duration !== null) {
              setRecordingDuration(session.duration);
            }

            const settings = useSettingsStore.getState().settings;
            notifyIfEnabled(
              settings?.notify_failed ?? true,
              "Meeting failed",
              session.title || "Recording or transcription failed.",
            );
          },
        );

        if (!isValid()) {
          failedUnlisten();
          return;
        }
        unlisteners.push(failedUnlisten);

        // Listen for live audio statistics. Backend emits at ~10Hz; we
        // throttle the store write to ~5Hz (every 200ms) because the UI
        // doesn't need sub-100ms updates and every set() here cascades into
        // re-renders of every component subscribed to the store.
        let lastStatsApply = 0;
        let pendingStats: MeetingAudioStats | null = null;
        let pendingStatsTimer: ReturnType<typeof setTimeout> | null = null;
        const STATS_THROTTLE_MS = 200;
        const applyPendingStats = () => {
          pendingStatsTimer = null;
          if (pendingStats && isValid()) {
            lastStatsApply = Date.now();
            set({ audioStats: pendingStats });
            pendingStats = null;
          }
        };
        const statsUnlisten = await listen<MeetingAudioStats>(
          "meeting_audio_stats",
          (event) => {
            if (!isValid()) return;
            pendingStats = event.payload;
            const now = Date.now();
            const elapsed = now - lastStatsApply;
            if (elapsed >= STATS_THROTTLE_MS) {
              if (pendingStatsTimer) {
                clearTimeout(pendingStatsTimer);
                pendingStatsTimer = null;
              }
              lastStatsApply = now;
              set({ audioStats: pendingStats });
              pendingStats = null;
            } else if (pendingStatsTimer == null) {
              pendingStatsTimer = setTimeout(
                applyPendingStats,
                STATS_THROTTLE_MS - elapsed,
              );
            }
          },
        );

        if (!isValid()) {
          statsUnlisten();
          return;
        }
        unlisteners.push(statsUnlisten);

        // Listen for live transcript chunks emitted while recording.
        const liveTranscriptUnlisten = await listen<MeetingLiveTranscript>(
          "meeting_live_transcript",
          (event) => {
            if (!isValid()) return;
            const currentSession = get().currentSession;
            if (currentSession?.id !== event.payload.session_id) return;
            const chunk = event.payload.chunk_text.trim();
            const cleanedChunk = isNoiseTranscript(chunk) ? "" : chunk;
            const cleanedFull = stripNoiseFromText(event.payload.text);
            set((state) => ({
              liveTranscript: cleanedFull,
              liveTranscriptSegments: cleanedChunk
                ? [
                    ...state.liveTranscriptSegments,
                    {
                      text: cleanedChunk,
                      offset:
                        event.payload.start_ms > 0 || event.payload.end_ms > 0
                          ? Math.max(0, Math.floor(event.payload.start_ms / 1000))
                          : state.recordingDuration,
                      startMs: event.payload.start_ms,
                      endMs: event.payload.end_ms,
                      speakerId: event.payload.speaker_id ?? null,
                    },
                  ]
                : state.liveTranscriptSegments,
            }));
          },
        );

        if (!isValid()) {
          liveTranscriptUnlisten();
          return;
        }
        unlisteners.push(liveTranscriptUnlisten);

        const pausedUnlisten = await listen<MeetingSession>(
          "meeting_paused",
          (event) => {
            if (!isValid()) return;
            const currentSession = get().currentSession;
            if (currentSession?.id !== event.payload.id) return;
            set({ isPaused: true });
          },
        );

        if (!isValid()) {
          pausedUnlisten();
          return;
        }
        unlisteners.push(pausedUnlisten);

        const resumedUnlisten = await listen<MeetingSession>(
          "meeting_resumed",
          (event) => {
            if (!isValid()) return;
            const currentSession = get().currentSession;
            if (currentSession?.id !== event.payload.id) return;
            set({ isPaused: false });
          },
        );

        if (!isValid()) {
          resumedUnlisten();
          return;
        }
        unlisteners.push(resumedUnlisten);

        // Listen for STT engine errors (e.g. Soniox bad key / connection lost)
        const sttErrorUnlisten = await listen<{ session_id: string; message: string }>(
          "meeting_stt_error",
          (event) => {
            if (!isValid()) return;
            const currentSession = get().currentSession;
            if (currentSession?.id !== event.payload.session_id) return;
            set({ sttError: event.payload.message });
          },
        );
        if (!isValid()) {
          sttErrorUnlisten();
          return;
        }
        unlisteners.push(sttErrorUnlisten);

        // Auto-add diarization participants emitted by Soniox path
        const participantAddedUnlisten = await listen<import("@/bindings").Participant>(
          "meeting_participant_added",
          (event) => {
            if (!isValid()) return;
            const currentSession = get().currentSession;
            if (currentSession?.id !== event.payload.session_id) return;
            set((state) => ({
              participants: [...state.participants, event.payload],
            }));
          },
        );
        if (!isValid()) {
          participantAddedUnlisten();
          return;
        }
        unlisteners.push(participantAddedUnlisten);

        // Global shortcut events emitted from Rust ACTION_MAP
        const startStopUnlisten = await listen("shortcut_start_stop_recording", () => {
          if (!isValid()) return;
          const state = get();
          if (state.sessionStatus === "recording") {
            state.stopMeeting();
          } else if (state.sessionStatus === "idle") {
            const { audioSource, sttEngine, sonioxApiKey, funasrBaseUrl, funasrModel } =
              useRecordingConfigStore.getState();
            state.startMeeting(
              audioSource,
              undefined,
              sttEngine,
              sonioxApiKey,
              funasrBaseUrl,
              funasrModel,
            );
          }
        });
        if (!isValid()) {
          startStopUnlisten();
          return;
        }
        unlisteners.push(startStopUnlisten);

        const pauseResumeUnlisten = await listen("shortcut_pause_resume", () => {
          if (!isValid()) return;
          const state = get();
          if (state.sessionStatus !== "recording") return;
          if (state.isPaused) state.resumeMeeting();
          else state.pauseMeeting();
        });
        if (!isValid()) {
          pauseResumeUnlisten();
          return;
        }
        unlisteners.push(pauseResumeUnlisten);

        // Set up visibility change handler for reconnection on app focus
        const handleVisibilityChange = () => {
          if (document.visibilityState === "visible") {
            // Refresh status when app becomes visible to sync state
            refreshStatus();
          }
        };
        document.addEventListener("visibilitychange", handleVisibilityChange);

        // Only commit listeners if still valid
        if (isValid()) {
          set({
            _eventUnlisteners: unlisteners,
            _visibilityHandler: handleVisibilityChange,
          });
        } else {
          // Cleanup if invalidated during setup
          unlisteners.forEach((unlisten) => unlisten());
          document.removeEventListener(
            "visibilitychange",
            handleVisibilityChange,
          );
        }
      } catch (error) {
        console.error("Failed to initialize event listeners:", error);
        // Cleanup any partially registered listeners
        unlisteners.forEach((unlisten) => unlisten());
      }
    },

    // Cleanup all event listeners
    cleanupEventListeners: () => {
      // Invalidate all pending inits
      set({ _initId: 0 });

      const { _eventUnlisteners, _visibilityHandler } = get();

      // Unsubscribe from all Tauri events
      for (const unlisten of _eventUnlisteners) {
        unlisten();
      }

      // Remove visibility change listener
      if (_visibilityHandler) {
        document.removeEventListener("visibilitychange", _visibilityHandler);
      }

      set({
        _eventUnlisteners: [],
        _visibilityHandler: null,
      });
    },
  })),
);
