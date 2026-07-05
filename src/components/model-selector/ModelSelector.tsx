import React, { useState, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { commands, type ModelInfo } from "@/bindings";
import { useModelEvents } from "@/hooks/useModelEvents";
import { useModelEventStore } from "@/stores/modelEventStore";
import { getTranslatedModelName } from "../../lib/utils/modelTranslation";
import ModelStatusButton from "./ModelStatusButton";
import ModelDropdown from "./ModelDropdown";
import DownloadProgressDisplay from "./DownloadProgressDisplay";

interface ModelSelectorProps {
  onError?: (error: string) => void;
}

const ModelSelector: React.FC<ModelSelectorProps> = ({ onError }) => {
  const { t } = useTranslation();

  useModelEvents();

  const [models, setModels] = useState<ModelInfo[]>([]);
  const [showModelDropdown, setShowModelDropdown] = useState(false);

  const currentModelId = useModelEventStore((s) => s.currentModelId);
  const modelStatus = useModelEventStore((s) => s.modelStatus);
  const modelError = useModelEventStore((s) => s.modelError);
  const extractingModels = useModelEventStore((s) => s.extractingModels);
  const downloadProgress = useModelEventStore((s) => s.downloadProgress);
  const downloadStats = useModelEventStore((s) => s.downloadStats);

  const downloadProgressMap = React.useMemo(
    () => new Map(Object.entries(downloadProgress)),
    [downloadProgress],
  );

  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    loadModels();
    loadCurrentModel();

    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setShowModelDropdown(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, []);

  const loadModels = async () => {
    try {
      const result = await commands.getAvailableModels();
      if (result.status === "ok") {
        setModels(result.data);
      }
    } catch (err) {
      console.error("Failed to load models:", err);
    }
  };

  const loadCurrentModel = async () => {
    const store = useModelEventStore.getState();
    try {
      const result = await commands.getCurrentModel();
      if (result.status === "ok") {
        const current = result.data;
        store.setCurrentModelId(current);

        if (current) {
          const statusResult = await commands.getTranscriptionModelStatus();
          if (statusResult.status === "ok") {
            const transcriptionStatus = statusResult.data;
            if (transcriptionStatus === current) {
              store.setModelStatus("ready");
            } else {
              store.setModelStatus("unloaded");
            }
          }
        } else {
          store.setModelStatus("none");
        }
      }
    } catch (err) {
      console.error("Failed to load current model:", err);
      store.setModelStatus("error");
      store.setModelError("Failed to check model status");
    }
  };

  const handleModelSelect = async (modelId: string) => {
    const store = useModelEventStore.getState();
    try {
      store.setCurrentModelId(modelId);
      store.setModelError(null);
      setShowModelDropdown(false);
      const result = await commands.setActiveModel(modelId);
      if (result.status === "error") {
        const errorMsg = result.error;
        store.setModelError(errorMsg);
        store.setModelStatus("error");
        onError?.(errorMsg);
      }
    } catch (err) {
      const errorMsg = `${err}`;
      store.setModelError(errorMsg);
      store.setModelStatus("error");
      onError?.(errorMsg);
    }
  };

  const handleModelDownload = async (modelId: string) => {
    const store = useModelEventStore.getState();
    try {
      store.setModelError(null);
      const result = await commands.downloadModel(modelId);
      if (result.status === "error") {
        const errorMsg = result.error;
        store.setModelError(errorMsg);
        store.setModelStatus("error");
        onError?.(errorMsg);
      }
    } catch (err) {
      const errorMsg = `${err}`;
      store.setModelError(errorMsg);
      store.setModelStatus("error");
      onError?.(errorMsg);
    }
  };

  const getCurrentModel = () => {
    return models.find((m) => m.id === currentModelId);
  };

  const getModelDisplayText = (): string => {
    if (extractingModels.size > 0) {
      if (extractingModels.size === 1) {
        const [modelId] = Array.from(extractingModels);
        const model = models.find((m) => m.id === modelId);
        const modelName = model
          ? getTranslatedModelName(model, t)
          : t("modelSelector.extractingGeneric").replace("...", "");
        return t("modelSelector.extracting", { modelName });
      } else {
        return t("modelSelector.extractingMultiple", {
          count: extractingModels.size,
        });
      }
    }

    if (downloadProgressMap.size > 0) {
      if (downloadProgressMap.size === 1) {
        const [progress] = Array.from(downloadProgressMap.values());
        const percentage = Math.max(
          0,
          Math.min(100, Math.round(progress.percentage)),
        );
        return t("modelSelector.downloading", { percentage });
      } else {
        return t("modelSelector.downloadingMultiple", {
          count: downloadProgressMap.size,
        });
      }
    }

    const currentModel = getCurrentModel();

    switch (modelStatus) {
      case "ready":
        return currentModel
          ? getTranslatedModelName(currentModel, t)
          : t("modelSelector.modelReady");
      case "loading":
        return currentModel
          ? t("modelSelector.loading", {
              modelName: getTranslatedModelName(currentModel, t),
            })
          : t("modelSelector.loadingGeneric");
      case "extracting":
        return currentModel
          ? t("modelSelector.extracting", {
              modelName: getTranslatedModelName(currentModel, t),
            })
          : t("modelSelector.extractingGeneric");
      case "error":
        return modelError || t("modelSelector.modelError");
      case "unloaded":
        return currentModel
          ? getTranslatedModelName(currentModel, t)
          : t("modelSelector.modelUnloaded");
      case "none":
        return t("modelSelector.noModelDownloadRequired");
      default:
        return currentModel
          ? getTranslatedModelName(currentModel, t)
          : t("modelSelector.modelUnloaded");
    }
  };

  const handleModelDelete = async (modelId: string) => {
    const result = await commands.deleteModel(modelId);
    if (result.status === "ok") {
      await loadModels();
      const store = useModelEventStore.getState();
      store.setModelError(null);
    }
  };

  return (
    <>
      <div className="relative" ref={dropdownRef}>
        <ModelStatusButton
          status={modelStatus}
          displayText={getModelDisplayText()}
          isDropdownOpen={showModelDropdown}
          onClick={() => setShowModelDropdown(!showModelDropdown)}
        />

        {showModelDropdown && (
          <ModelDropdown
            models={models}
            currentModelId={currentModelId}
            downloadProgress={downloadProgressMap}
            onModelSelect={handleModelSelect}
            onModelDownload={handleModelDownload}
            onModelDelete={handleModelDelete}
            onError={onError}
          />
        )}
      </div>

      <DownloadProgressDisplay
        downloadProgress={downloadProgress}
        downloadStats={downloadStats}
      />
    </>
  );
};

export default ModelSelector;
