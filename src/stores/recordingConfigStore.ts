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
export type SttEngine = "whisper" | "soniox" | "funasr";

interface RecordingConfigState {
  audioSource: AudioSourceType;
  recordingQuality: RecordingQuality;
  autoSave: boolean;
  autoTranscribe: boolean;
  autoSummary: boolean;
  summaryLanguage: string;
  saveLocation: string;
  meetingTitle: string;
  participants: string;
  tags: string;
  sttEngine: SttEngine;
  sonioxApiKey: string;
  funasrBaseUrl: string;
  funasrModel: string;

  setAudioSource: (v: AudioSourceType) => void;
  setRecordingQuality: (v: RecordingQuality) => void;
  setAutoSave: (v: boolean) => void;
  setAutoTranscribe: (v: boolean) => void;
  setAutoSummary: (v: boolean) => void;
  setSummaryLanguage: (v: string) => void;
  setSaveLocation: (v: string) => void;
  setMeetingTitle: (v: string) => void;
  setParticipants: (v: string) => void;
  setTags: (v: string) => void;
  setSttEngine: (v: SttEngine) => void;
  setSonioxApiKey: (v: string) => void;
  setFunasrBaseUrl: (v: string) => void;
  setFunasrModel: (v: string) => void;
}

export const useRecordingConfigStore = create<RecordingConfigState>()((set) => ({
  audioSource: "microphone_only",
  recordingQuality: "high",
  autoSave: true,
  autoTranscribe: false,
  autoSummary: true,
  summaryLanguage: "auto",
  saveLocation: "~/Meetdy/Recordings",
  meetingTitle: "",
  participants: "",
  tags: "",
  sttEngine: "whisper",
  sonioxApiKey: "",
  funasrBaseUrl: "http://localhost:8000",
  funasrModel: "fun-asr-nano",

  setAudioSource: (audioSource) => set({ audioSource }),
  setRecordingQuality: (recordingQuality) => set({ recordingQuality }),
  setAutoSave: (autoSave) => set({ autoSave }),
  setAutoTranscribe: (autoTranscribe) => set({ autoTranscribe }),
  setAutoSummary: (autoSummary) => set({ autoSummary }),
  setSummaryLanguage: (summaryLanguage) => set({ summaryLanguage }),
  setSaveLocation: (saveLocation) => set({ saveLocation }),
  setMeetingTitle: (meetingTitle) => set({ meetingTitle }),
  setParticipants: (participants) => set({ participants }),
  setTags: (tags) => set({ tags }),
  setSttEngine: (sttEngine) => set({ sttEngine }),
  setSonioxApiKey: (sonioxApiKey) => set({ sonioxApiKey }),
  setFunasrBaseUrl: (funasrBaseUrl) => set({ funasrBaseUrl }),
  setFunasrModel: (funasrModel) => set({ funasrModel }),
}));
