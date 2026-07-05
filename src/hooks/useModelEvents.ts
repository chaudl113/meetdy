import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  useModelEventStore,
  type DownloadProgress,
  type ModelStateEvent,
  type DownloadStats,
} from "@/stores/modelEventStore";

let globalUnlistenPromises: Promise<() => void>[] | null = null;
let listenerCount = 0;

function calcStats(prevStats: Record<string, DownloadStats>, progress: DownloadProgress): Record<string, DownloadStats> {
  const now = Date.now();
  const current = prevStats[progress.model_id];
  const modelId = progress.model_id;

  if (!current) {
    return { ...prevStats, [modelId]: { startTime: now, lastUpdate: now, totalDownloaded: progress.downloaded, speed: 0 } };
  }

  const timeDiff = (now - current.lastUpdate) / 1000;
  const bytesDiff = progress.downloaded - current.totalDownloaded;

  if (timeDiff <= 0.5) return prevStats;

  const currentSpeed = bytesDiff / (1024 * 1024) / timeDiff;
  const validCurrentSpeed = Math.max(0, currentSpeed);
  const smoothedSpeed =
    current.speed > 0
      ? current.speed * 0.8 + validCurrentSpeed * 0.2
      : validCurrentSpeed;

  return {
    ...prevStats,
    [modelId]: {
      startTime: current.startTime,
      lastUpdate: now,
      totalDownloaded: progress.downloaded,
      speed: Math.max(0, smoothedSpeed),
    },
  };
}

export function useModelEvents() {
  const mounted = useRef(false);

  useEffect(() => {
    if (mounted.current) return;
    mounted.current = true;
    listenerCount++;

    if (globalUnlistenPromises) return;

    const store = useModelEventStore.getState;

    const modelStateUnlisten = listen<ModelStateEvent>(
      "model-state-changed",
      (event) => {
        const { event_type, model_id, error } = event.payload;
        const state = store();

        switch (event_type) {
          case "loading_started":
            state.setModelStatus("loading");
            state.setModelError(null);
            break;
          case "loading_completed":
            state.setModelStatus("ready");
            state.setModelError(null);
            if (model_id) state.setCurrentModelId(model_id);
            break;
          case "loading_failed":
            state.setModelStatus("error");
            state.setModelError(error || "Failed to load model");
            break;
          case "unloaded":
            state.setModelStatus("unloaded");
            state.setModelError(null);
            break;
        }
      },
    );

    const downloadProgressUnlisten = listen<DownloadProgress>(
      "model-download-progress",
      (event) => {
        const state = store();
        state.updateDownloadProgress(event.payload);
        useModelEventStore.setState((s) => ({
          downloadStats: calcStats(s.downloadStats, event.payload),
        }));
      },
    );

    const downloadCompleteUnlisten = listen<string>(
      "model-download-complete",
      (event) => {
        const modelId = event.payload;
        const state = store();
        state.removeDownloadProgress(modelId);
        useModelEventStore.setState((s) => {
          const next = { ...s.downloadStats };
          delete next[modelId];
          return { downloadStats: next };
        });
      },
    );

    const extractionStartedUnlisten = listen<string>(
      "model-extraction-started",
      (event) => {
        store().addExtractingModel(event.payload);
      },
    );

    const extractionCompletedUnlisten = listen<string>(
      "model-extraction-completed",
      (event) => {
        store().removeExtractingModel(event.payload);
      },
    );

    const extractionFailedUnlisten = listen<{
      model_id: string;
      error: string;
    }>("model-extraction-failed", (event) => {
      const state = store();
      state.removeExtractingModel(event.payload.model_id);
      state.setModelError(`Failed to extract model: ${event.payload.error}`);
      state.setModelStatus("error");
    });

    globalUnlistenPromises = [
      modelStateUnlisten,
      downloadProgressUnlisten,
      downloadCompleteUnlisten,
      extractionStartedUnlisten,
      extractionCompletedUnlisten,
      extractionFailedUnlisten,
    ];

    return () => {
      listenerCount--;
      if (listenerCount === 0 && globalUnlistenPromises) {
        globalUnlistenPromises.forEach((p) => { p.then((fn) => fn()); });
        globalUnlistenPromises = null;
      }
    };
  }, []);
}
