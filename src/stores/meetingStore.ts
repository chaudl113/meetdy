import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import type { MeetingSession, MeetingStatus } from "@/bindings";
import { commands } from "@/bindings";

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
  })),
);
