import { create } from "zustand";
import type { ModelInfo } from "@/bindings";

export interface DownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  percentage: number;
}

export interface DownloadStats {
  startTime: number;
  lastUpdate: number;
  totalDownloaded: number;
  speed: number;
}

export interface ModelStateEvent {
  event_type: string;
  model_id?: string;
  model_name?: string;
  error?: string;
}

interface ModelEventState {
  modelStatus: "ready" | "loading" | "downloading" | "extracting" | "error" | "unloaded" | "none";
  currentModelId: string;
  modelError: string | null;
  downloadProgress: Record<string, DownloadProgress>;
  downloadStats: Record<string, DownloadStats>;
  extractingModels: Set<string>;

  setModelStatus: (status: string) => void;
  setCurrentModelId: (id: string) => void;
  setModelError: (error: string | null) => void;
  updateDownloadProgress: (progress: DownloadProgress) => void;
  removeDownloadProgress: (modelId: string) => void;
  addExtractingModel: (modelId: string) => void;
  removeExtractingModel: (modelId: string) => void;
  reset: () => void;
}

export const useModelEventStore = create<ModelEventState>()((set) => ({
  modelStatus: "unloaded",
  currentModelId: "",
  modelError: null,
  downloadProgress: {},
  downloadStats: {},
  extractingModels: new Set(),

  setModelStatus: (status) => set({ modelStatus: status as ModelEventState["modelStatus"] }),
  setCurrentModelId: (id) => set({ currentModelId: id }),
  setModelError: (error) => set({ modelError: error }),
  updateDownloadProgress: (progress) =>
    set((state) => ({
      downloadProgress: { ...state.downloadProgress, [progress.model_id]: progress },
      modelStatus: "downloading",
    })),
  removeDownloadProgress: (modelId) =>
    set((state) => {
      const next = { ...state.downloadProgress };
      delete next[modelId];
      return { downloadProgress: next };
    }),
  addExtractingModel: (modelId) =>
    set((state) => ({
      extractingModels: new Set(state.extractingModels).add(modelId),
      modelStatus: "extracting",
    })),
  removeExtractingModel: (modelId) =>
    set((state) => {
      const next = new Set(state.extractingModels);
      next.delete(modelId);
      return { extractingModels: next };
    }),
  reset: () =>
    set({
      modelStatus: "unloaded",
      currentModelId: "",
      modelError: null,
      downloadProgress: {},
      downloadStats: {},
      extractingModels: new Set(),
    }),
}));
