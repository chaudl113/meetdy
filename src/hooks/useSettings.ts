import { useEffect } from "react";
import { useSettingsStore } from "../stores/settingsStore";
import type { AppSettings as Settings, AudioDevice } from "@/bindings";

interface UseSettingsReturn {
  // State
  settings: Settings | null;
  isLoading: boolean;
  isUpdating: (key: string) => boolean;
  audioDevices: AudioDevice[];
  outputDevices: AudioDevice[];
  audioFeedbackEnabled: boolean;
  postProcessModelOptions: Record<string, string[]>;

  // Actions
  updateSetting: <K extends keyof Settings>(
    key: K,
    value: Settings[K],
  ) => Promise<void>;
  resetSetting: (key: keyof Settings) => Promise<void>;
  refreshSettings: () => Promise<void>;
  refreshAudioDevices: () => Promise<void>;
  refreshOutputDevices: () => Promise<void>;

  // Binding-specific actions
  updateBinding: (id: string, binding: string) => Promise<void>;
  resetBinding: (id: string) => Promise<void>;

  // Convenience getters
  getSetting: <K extends keyof Settings>(key: K) => Settings[K] | undefined;

  // Post-processing helpers
  setPostProcessProvider: (providerId: string) => Promise<void>;
  updatePostProcessBaseUrl: (
    providerId: string,
    baseUrl: string,
  ) => Promise<void>;
  updatePostProcessApiKey: (
    providerId: string,
    apiKey: string,
  ) => Promise<void>;
  updatePostProcessModel: (providerId: string, model: string) => Promise<void>;
  fetchPostProcessModels: (providerId: string) => Promise<string[]>;
}

export const useSettings = (): UseSettingsReturn => {
  const settings = useSettingsStore((s) => s.settings);
  const isLoading = useSettingsStore((s) => s.isLoading);
  const audioDevices = useSettingsStore((s) => s.audioDevices);
  const outputDevices = useSettingsStore((s) => s.outputDevices);
  const postProcessModelOptions = useSettingsStore(
    (s) => s.postProcessModelOptions,
  );
  const isUpdating = useSettingsStore((s) => s.isUpdatingKey);
  const updateSetting = useSettingsStore((s) => s.updateSetting);
  const resetSetting = useSettingsStore((s) => s.resetSetting);
  const refreshSettings = useSettingsStore((s) => s.refreshSettings);
  const refreshAudioDevices = useSettingsStore((s) => s.refreshAudioDevices);
  const refreshOutputDevices = useSettingsStore((s) => s.refreshOutputDevices);
  const updateBinding = useSettingsStore((s) => s.updateBinding);
  const resetBinding = useSettingsStore((s) => s.resetBinding);
  const getSetting = useSettingsStore((s) => s.getSetting);
  const setPostProcessProvider = useSettingsStore(
    (s) => s.setPostProcessProvider,
  );
  const updatePostProcessBaseUrl = useSettingsStore(
    (s) => s.updatePostProcessBaseUrl,
  );
  const updatePostProcessApiKey = useSettingsStore(
    (s) => s.updatePostProcessApiKey,
  );
  const updatePostProcessModel = useSettingsStore(
    (s) => s.updatePostProcessModel,
  );
  const fetchPostProcessModels = useSettingsStore(
    (s) => s.fetchPostProcessModels,
  );
  const initialize = useSettingsStore((s) => s.initialize);

  // Initialize on first mount
  useEffect(() => {
    if (isLoading) {
      initialize();
    }
  }, [initialize, isLoading]);

  return {
    settings,
    isLoading,
    isUpdating,
    audioDevices,
    outputDevices,
    audioFeedbackEnabled: settings?.audio_feedback || false,
    postProcessModelOptions,
    updateSetting,
    resetSetting,
    refreshSettings,
    refreshAudioDevices,
    refreshOutputDevices,
    updateBinding,
    resetBinding,
    getSetting,
    setPostProcessProvider,
    updatePostProcessBaseUrl,
    updatePostProcessApiKey,
    updatePostProcessModel,
    fetchPostProcessModels,
  };
};
