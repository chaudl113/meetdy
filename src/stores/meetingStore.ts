import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { MeetingSession, MeetingStatus } from "@/bindings";
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
  recordingDuration: number;
  isLoading: boolean;
  error: string | null;

  // Actions
  startMeeting: () => Promise<void>;
  stopMeeting: () => Promise<void>;
  retryTranscription: () => Promise<void>;
  updateTitle: (title: string) => Promise<void>;
  refreshStatus: () => Promise<void>;
  clearError: () => void;

  // Internal setters
  setSessionStatus: (status: MeetingStatus) => void;
  setCurrentSession: (session: MeetingSession | null) => void;
  setRecordingDuration: (duration: number) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;

  // Internal timer
  _durationInterval: ReturnType<typeof setInterval> | null;
  _startDurationTimer: () => void;
  _stopDurationTimer: () => void;

  // Event listener management
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
    recordingDuration: 0,
    isLoading: false,
    error: null,

    // Internal timer reference
    _durationInterval: null,

    // Internal setters
    setSessionStatus: (sessionStatus) => set({ sessionStatus }),
    setCurrentSession: (currentSession) => set({ currentSession }),
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
    startMeeting: async () => {
      const { setLoading, setError, setSessionStatus, setCurrentSession, _startDurationTimer } =
        get();

      setLoading(true);
      setError(null);

      try {
        const result = await commands.startMeetingSession();
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
          err instanceof Error ? err.message : "Failed to refresh meeting status";
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
        const result = await commands.updateMeetingTitle(currentSession.id, title);
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

    // Event listener management
    _eventUnlisteners: [],
    _visibilityHandler: null,

    // Initialize event listeners for meeting_* events from backend
    initializeEventListeners: async () => {
      const {
        setSessionStatus,
        setCurrentSession,
        _startDurationTimer,
        _stopDurationTimer,
        refreshStatus,
        cleanupEventListeners,
      } = get();

      // Clean up any existing listeners first
      cleanupEventListeners();

      const unlisteners: UnlistenFn[] = [];

      // Listen for meeting_started event
      const startedUnlisten = await listen<MeetingSession>(
        "meeting_started",
        (event) => {
          const session = event.payload;
          setCurrentSession(session);
          setSessionStatus("recording");
          _startDurationTimer();
        }
      );
      unlisteners.push(startedUnlisten);

      // Listen for meeting_stopped event
      const stoppedUnlisten = await listen<MeetingSession>(
        "meeting_stopped",
        (event) => {
          const session = event.payload;
          setCurrentSession(session);
          _stopDurationTimer();
          // Status will transition to processing next
        }
      );
      unlisteners.push(stoppedUnlisten);

      // Listen for meeting_processing event
      const processingUnlisten = await listen<MeetingSession>(
        "meeting_processing",
        (event) => {
          const session = event.payload;
          setCurrentSession(session);
          setSessionStatus("processing");
          _stopDurationTimer();
        }
      );
      unlisteners.push(processingUnlisten);

      // Listen for meeting_completed event
      const completedUnlisten = await listen<MeetingSession>(
        "meeting_completed",
        (event) => {
          const session = event.payload;
          setCurrentSession(session);
          setSessionStatus("completed");
          _stopDurationTimer();
        }
      );
      unlisteners.push(completedUnlisten);

      // Listen for meeting_failed event
      const failedUnlisten = await listen<MeetingSession>(
        "meeting_failed",
        (event) => {
          const session = event.payload;
          setCurrentSession(session);
          setSessionStatus("failed");
          _stopDurationTimer();
        }
      );
      unlisteners.push(failedUnlisten);

      // Set up visibility change handler for reconnection on app focus
      const handleVisibilityChange = () => {
        if (document.visibilityState === "visible") {
          // Refresh status when app becomes visible to sync state
          refreshStatus();
        }
      };
      document.addEventListener("visibilitychange", handleVisibilityChange);

      // Store the handler reference for cleanup
      set({
        _eventUnlisteners: unlisteners,
        _visibilityHandler: handleVisibilityChange,
      });
    },

    // Cleanup all event listeners
    cleanupEventListeners: () => {
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
