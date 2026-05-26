import { create } from "zustand";
import type { AudioSourceType } from "@/bindings";

/**
 * Recording quality presets used by the StartMeeting form.
 * Only "high" is currently honoured by the backend (16 kHz mono WAV);
 * the value is kept to display the selected preset in RecordingView.
 */
export type RecordingQuality = "low" | "medium" | "high";

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
  recordingQuality: RecordingQuality;
  autoTranscribe: boolean;
  autoSummary: boolean;
  saveLocation: string;
  meetingTitle: string;
  participants: string;
  tags: string;

  setAudioSource: (v: AudioSourceType) => void;
  setRecordingQuality: (v: RecordingQuality) => void;
  setAutoTranscribe: (v: boolean) => void;
  setAutoSummary: (v: boolean) => void;
  setSaveLocation: (v: string) => void;
  setMeetingTitle: (v: string) => void;
  setParticipants: (v: string) => void;
  setTags: (v: string) => void;
}

export const useRecordingConfigStore = create<RecordingConfigState>()((set) => ({
  audioSource: "system_only",
  recordingQuality: "high",
  autoTranscribe: true,
  autoSummary: true,
  saveLocation: "~/Meetdy/Recordings",
  meetingTitle: "",
  participants: "",
  tags: "",

  setAudioSource: (audioSource) => set({ audioSource }),
  setRecordingQuality: (recordingQuality) => set({ recordingQuality }),
  setAutoTranscribe: (autoTranscribe) => set({ autoTranscribe }),
  setAutoSummary: (autoSummary) => set({ autoSummary }),
  setSaveLocation: (saveLocation) => set({ saveLocation }),
  setMeetingTitle: (meetingTitle) => set({ meetingTitle }),
  setParticipants: (participants) => set({ participants }),
  setTags: (tags) => set({ tags }),
}));
