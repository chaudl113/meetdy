import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AudioSourceType,
  MeetingSession,
  MeetingStatus,
} from "@/bindings";
import { commands } from "@/bindings";

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

interface MeetingStore {
  // State
  sessionStatus: MeetingStatus;
  currentSession: MeetingSession | null;
  sessions: MeetingSession[];
  recordingDuration: number;
  isLoading: boolean;
  error: string | null;

  // Actions
  startMeeting: (audioSource?: AudioSourceType) => Promise<void>;
  stopMeeting: () => Promise<void>;
  retryTranscription: () => Promise<void>;
  updateTitle: (title: string) => Promise<void>;
  refreshStatus: () => Promise<void>;
  fetchSessions: () => Promise<void>;
  clearError: () => void;

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
    isLoading: false,
    error: null,

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
        set((state) => ({
          recordingDuration: state.recordingDuration + 1,
        }));
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
    startMeeting: async (audioSource?: AudioSourceType) => {
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
        const result = await commands.startMeetingSession(audioSource ?? null);
        if (result.status === "ok") {
          const session = result.data as MeetingSession;
          setCurrentSession(session);
          setSessionStatus("recording");
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
      const { setLoading, setError, setSessionStatus, _stopDurationTimer } =
        get();

      setLoading(true);
      setError(null);

      try {
        const result = await commands.stopMeetingSession();
        if (result.status === "ok") {
          setSessionStatus("processing");
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

    // Retry transcription for a failed meeting session
    retryTranscription: async () => {
      const { currentSession, setLoading, setError, setSessionStatus } = get();

      // Validate we have a current session
      if (!currentSession) {
        setError("No meeting session to retry");
        return;
      }

      // Validate session is in Failed status
      if (currentSession.status !== "failed") {
        setError("Can only retry transcription for failed sessions");
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

    // Initialize event listeners for meeting_* events from backend
    initializeEventListeners: async () => {
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
          },
        );

        if (!isValid()) {
          completedUnlisten();
          return;
        }
        unlisteners.push(completedUnlisten);

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
          },
        );

        if (!isValid()) {
          failedUnlisten();
          return;
        }
        unlisteners.push(failedUnlisten);

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
