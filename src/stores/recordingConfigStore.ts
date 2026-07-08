import { create } from "zustand";
import type { AudioSourceType } from "@/bindings";

/**
 * Shared, in-memory configuration captured on the StartMeeting screen
 * and consumed by RecordingView while a meeting is in progress.
 *
 * This state is intentionally NOT persisted — it represents the choices
 * made for the current/upcoming recording. Settings that should survive
 * across launches live in `settingsStore` / backend `AppSettings`.
 */
interface RecordingConfigState {
  audioSource: AudioSourceType;
  autoTranscribe: boolean;
  autoSummary: boolean;
  summaryLanguage: string;
  saveLocation: string;
  meetingTitle: string;
  participants: string;
  tags: string;

  setAudioSource: (v: AudioSourceType) => void;
  setAutoTranscribe: (v: boolean) => void;
  setAutoSummary: (v: boolean) => void;
  setSummaryLanguage: (v: string) => void;
  setSaveLocation: (v: string) => void;
  setMeetingTitle: (v: string) => void;
  setParticipants: (v: string) => void;
  setTags: (v: string) => void;
}

export const useRecordingConfigStore = create<RecordingConfigState>()((set) => ({
  audioSource: "microphone_only",
  autoTranscribe: false,
  autoSummary: true,
  summaryLanguage: "auto",
  saveLocation: "~/Meetdy/Recordings",
  meetingTitle: "",
  participants: "",
  tags: "",

  setAudioSource: (audioSource) => set({ audioSource }),
  setAutoTranscribe: (autoTranscribe) => set({ autoTranscribe }),
  setAutoSummary: (autoSummary) => set({ autoSummary }),
  setSummaryLanguage: (summaryLanguage) => set({ summaryLanguage }),
  setSaveLocation: (saveLocation) => set({ saveLocation }),
  setMeetingTitle: (meetingTitle) => set({ meetingTitle }),
  setParticipants: (participants) => set({ participants }),
  setTags: (tags) => set({ tags }),
}));
