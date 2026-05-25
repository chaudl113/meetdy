import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { Check, Download, Trash2 } from "lucide-react";
import { commands, type ModelInfo } from "@/bindings";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { ProgressBar } from "../../shared";
import { formatModelSize } from "../../../lib/utils/format";
import {
  getTranslatedModelDescription,
  getTranslatedModelName,
} from "../../../lib/utils/modelTranslation";

interface DownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  percentage: number;
}

interface ModelStateEvent {
  event_type: string;
  model_id?: string;
  model_name?: string;
  error?: string;
}

export const ModelsSettings: React.FC = () => {
  const { t } = useTranslation();
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [currentModelId, setCurrentModelId] = useState<string>("");
  const [downloadProgress, setDownloadProgress] = useState<
    Map<string, DownloadProgress>
  >(new Map());
  const [extractingModels, setExtractingModels] = useState<Set<string>>(
    new Set(),
  );
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [busyModelId, setBusyModelId] = useState<string | null>(null);

  const loadModels = async () => {
    const result = await commands.getAvailableModels();
    if (result.status === "ok") {
      setModels(result.data);
    }
  };

  const loadCurrentModel = async () => {
    const result = await commands.getCurrentModel();
    if (result.status === "ok") {
      setCurrentModelId(result.data ?? "");
    }
  };

  useEffect(() => {
    loadModels();
    loadCurrentModel();

    const stateUnlisten = listen<ModelStateEvent>(
      "model-state-changed",
      (event) => {
        const { event_type, model_id, error } = event.payload;
        if (event_type === "loading_completed" && model_id) {
          setCurrentModelId(model_id);
          setBusyModelId(null);
          setErrorMessage(null);
        } else if (event_type === "loading_failed") {
          setErrorMessage(error || "Failed to load model");
          setBusyModelId(null);
        }
      },
    );

    const progressUnlisten = listen<DownloadProgress>(
      "model-download-progress",
      (event) => {
        const progress = event.payload;
        setDownloadProgress((prev) => {
          const next = new Map(prev);
          next.set(progress.model_id, progress);
          return next;
        });
      },
    );

    const completeUnlisten = listen<string>(
      "model-download-complete",
      (event) => {
        const modelId = event.payload;
        setDownloadProgress((prev) => {
          const next = new Map(prev);
          next.delete(modelId);
          return next;
        });
        loadModels();
      },
    );

    const extractStartUnlisten = listen<string>(
      "model-extraction-started",
      (event) => {
        setExtractingModels((prev) => new Set(prev).add(event.payload));
      },
    );

    const extractDoneUnlisten = listen<string>(
      "model-extraction-completed",
      (event) => {
        setExtractingModels((prev) => {
          const next = new Set(prev);
          next.delete(event.payload);
          return next;
        });
        loadModels();
      },
    );

    const extractFailUnlisten = listen<{ model_id: string; error: string }>(
      "model-extraction-failed",
      (event) => {
        setExtractingModels((prev) => {
          const next = new Set(prev);
          next.delete(event.payload.model_id);
          return next;
        });
        setErrorMessage(`Failed to extract model: ${event.payload.error}`);
      },
    );

    return () => {
      stateUnlisten.then((fn) => fn());
      progressUnlisten.then((fn) => fn());
      completeUnlisten.then((fn) => fn());
      extractStartUnlisten.then((fn) => fn());
      extractDoneUnlisten.then((fn) => fn());
      extractFailUnlisten.then((fn) => fn());
    };
  }, []);

  const handleSelect = async (modelId: string) => {
    if (modelId === currentModelId) return;
    setErrorMessage(null);
    setBusyModelId(modelId);
    const result = await commands.setActiveModel(modelId);
    if (result.status === "error") {
      setErrorMessage(result.error);
      setBusyModelId(null);
    } else {
      setCurrentModelId(modelId);
    }
  };

  const handleDownload = async (modelId: string) => {
    setErrorMessage(null);
    const result = await commands.downloadModel(modelId);
    if (result.status === "error") {
      setErrorMessage(result.error);
    }
  };

  const handleDelete = async (modelId: string) => {
    setErrorMessage(null);
    const result = await commands.deleteModel(modelId);
    if (result.status === "ok") {
      await loadModels();
      if (modelId === currentModelId) {
        setCurrentModelId("");
      }
    } else {
      setErrorMessage(result.error);
    }
  };

  const downloadedModels = models.filter((m) => m.is_downloaded);
  const downloadableModels = models.filter((m) => !m.is_downloaded);

  const renderDownloadedModel = (model: ModelInfo) => {
    const isActive = model.id === currentModelId;
    const isBusy = busyModelId === model.id;
    const isExtracting = extractingModels.has(model.id);

    return (
      <div
        key={model.id}
        className="flex items-center justify-between p-4 gap-4"
      >
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <p className="text-sm font-medium truncate">
              {getTranslatedModelName(model, t)}
            </p>
            {isActive && (
              <span className="inline-flex items-center gap-1 text-xs text-logo-primary bg-logo-primary/10 px-2 py-0.5 rounded">
                <Check size={12} />
                {t("modelSelector.active")}
              </span>
            )}
          </div>
          <p className="text-xs text-text/60 italic mt-0.5">
            {getTranslatedModelDescription(model, t)}
          </p>
          <p className="text-xs text-text/40 mt-1 tabular-nums">
            {formatModelSize(Number(model.size_mb))}
          </p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {!isActive && (
            <button
              type="button"
              disabled={isBusy || isExtracting}
              onClick={() => handleSelect(model.id)}
              className="text-xs px-3 py-1.5 rounded-md border border-mid-gray/30 hover:bg-mid-gray/10 transition-colors disabled:opacity-50"
            >
              {isBusy
                ? t("modelSelector.loadingGeneric")
                : t("models.activate")}
            </button>
          )}
          {!isActive && (
            <button
              type="button"
              onClick={() => handleDelete(model.id)}
              className="p-1.5 text-red-400 hover:text-red-300 hover:bg-red-500/10 rounded-md transition-colors"
              title={t("modelSelector.deleteModel", {
                modelName: getTranslatedModelName(model, t),
              })}
            >
              <Trash2 size={14} />
            </button>
          )}
        </div>
      </div>
    );
  };

  const renderDownloadableModel = (model: ModelInfo) => {
    const progress = downloadProgress.get(model.id);
    const isDownloading = !!progress;
    const isExtracting = extractingModels.has(model.id);

    return (
      <div key={model.id} className="p-4">
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium truncate">
              {getTranslatedModelName(model, t)}
            </p>
            <p className="text-xs text-text/60 italic mt-0.5">
              {getTranslatedModelDescription(model, t)}
            </p>
            <p className="text-xs text-text/40 mt-1 tabular-nums">
              {t("modelSelector.downloadSize")} ·{" "}
              {formatModelSize(Number(model.size_mb))}
            </p>
          </div>
          <button
            type="button"
            disabled={isDownloading || isExtracting}
            onClick={() => handleDownload(model.id)}
            className="shrink-0 inline-flex items-center gap-1.5 text-xs px-3 py-1.5 rounded-md bg-logo-primary/80 hover:bg-logo-primary text-white transition-colors disabled:opacity-50"
          >
            <Download size={12} />
            {isDownloading
              ? `${Math.max(0, Math.min(100, Math.round(progress.percentage)))}%`
              : isExtracting
                ? t("modelSelector.extractingGeneric")
                : t("modelSelector.download")}
          </button>
        </div>
        {isDownloading && (
          <div className="mt-3">
            <ProgressBar
              progress={[
                {
                  id: model.id,
                  percentage: progress.percentage,
                  label: getTranslatedModelName(model, t),
                },
              ]}
              size="small"
            />
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      {errorMessage && (
        <div className="px-4 py-3 rounded-md bg-red-500/10 border border-red-500/30 text-sm text-red-400">
          {errorMessage}
        </div>
      )}

      <SettingsGroup
        title={t("models.installedTitle")}
        description={t("models.installedDescription")}
      >
        {downloadedModels.length > 0 ? (
          downloadedModels.map(renderDownloadedModel)
        ) : (
          <div className="px-4 py-6 text-sm text-text/60 text-center">
            {t("models.noInstalled")}
          </div>
        )}
      </SettingsGroup>

      <SettingsGroup
        title={t("models.availableTitle")}
        description={t("models.availableDescription")}
      >
        {downloadableModels.length > 0 ? (
          downloadableModels.map(renderDownloadableModel)
        ) : (
          <div className="px-4 py-6 text-sm text-text/60 text-center">
            {t("models.noAvailable")}
          </div>
        )}
      </SettingsGroup>
    </div>
  );
};

export default ModelsSettings;
