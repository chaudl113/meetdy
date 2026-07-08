import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Download, Trash2 } from "lucide-react";
import { commands, type ModelInfo } from "@/bindings";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { ProgressBar } from "../../shared";
import { formatModelSize } from "../../../lib/utils/format";
import {
  getTranslatedModelDescription,
  getTranslatedModelName,
} from "../../../lib/utils/modelTranslation";
import { useModelEvents } from "@/hooks/useModelEvents";
import { type DownloadProgress, useModelEventStore } from "@/stores/modelEventStore";

export const ModelsSettings: React.FC = () => {
  const { t } = useTranslation();

  useModelEvents();

  const [models, setModels] = useState<ModelInfo[]>([]);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [busyModelId, setBusyModelId] = useState<string | null>(null);

  const currentModelId = useModelEventStore((s) => s.currentModelId);
  const downloadProgress = useModelEventStore((s) => s.downloadProgress);
  const extractingModels = useModelEventStore((s) => s.extractingModels);

  const loadModels = async () => {
    const result = await commands.getAvailableModels();
    if (result.status === "ok") {
      setModels(result.data);
    }
  };

  const loadCurrentModel = async () => {
    const result = await commands.getCurrentModel();
    if (result.status === "ok") {
      useModelEventStore.getState().setCurrentModelId(result.data ?? "");
    }
  };

  useEffect(() => {
    loadModels();
    loadCurrentModel();
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
      useModelEventStore.getState().setCurrentModelId(modelId);
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
        useModelEventStore.getState().setCurrentModelId("");
      }
    } else {
      setErrorMessage(result.error);
    }
  };

  const downloadedModels = models.filter((m) => m.is_downloaded && m.engine_type !== "Diarization");
  const recommendedDownloaded = downloadedModels.filter((m) => m.is_recommended);
  const downloadableModels = models.filter((m) => !m.is_downloaded && m.engine_type !== "Diarization");
  const diarizationModels = models.filter((m) => m.engine_type === "Diarization");

  const getModelCategory = (model: ModelInfo): "vi" | "multi" | "en" => {
    const langs = model.supported_languages;
    if (langs.includes("vi")) return "vi";
    if (langs.length > 1) return "multi";
    return "en";
  };

  const groupModels = (list: ModelInfo[]) => ({
    vi: list.filter((m) => getModelCategory(m) === "vi"),
    multi: list.filter((m) => getModelCategory(m) === "multi"),
    en: list.filter((m) => getModelCategory(m) === "en"),
  });

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
          <div className="flex items-center gap-2 flex-wrap">
            <p className="text-sm font-medium truncate">
              {getTranslatedModelName(model, t)}
            </p>
            {isActive && (
              <span className="inline-flex items-center gap-1 text-xs text-logo-primary bg-logo-primary/10 px-2 py-0.5 rounded">
                <Check size={12} />
                {t("modelSelector.active")}
              </span>
            )}
            {model.is_recommended && (
              <span className="text-xs bg-logo-primary/10 text-logo-primary px-1.5 py-0.5 rounded">
                {t("modelSelector.recommended", "Recommended")}
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
    const progress = downloadProgress[model.id];
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
        {downloadedModels.length > 0 ? (() => {
          const nonRecommended = downloadedModels.filter((m) => !m.is_recommended);
          const groups = groupModels(nonRecommended);
          return (
            <>
              {recommendedDownloaded.length > 0 && (
                <>
                  <div className="px-4 pt-3 pb-1 text-xs text-text/40 font-medium">
                    ⭐ {t("modelSelector.recommended", "Recommended")}
                  </div>
                  {recommendedDownloaded.map(renderDownloadedModel)}
                </>
              )}
              {groups.vi.length > 0 && (
                <>
                  <div className="px-4 pt-3 pb-1 text-xs text-text/40">
                    🇻🇳 {t("modelSelector.groupVietnamese", "Tiếng Việt")}
                  </div>
                  {groups.vi.map(renderDownloadedModel)}
                </>
              )}
              {groups.multi.length > 0 && (
                <>
                  <div className="px-4 pt-3 pb-1 text-xs text-text/40">
                    🌐 {t("modelSelector.groupMultilingual", "Đa ngôn ngữ")}
                  </div>
                  {groups.multi.map(renderDownloadedModel)}
                </>
              )}
              {groups.en.length > 0 && (
                <>
                  <div className="px-4 pt-3 pb-1 text-xs text-text/40">
                    🇬🇧 {t("modelSelector.groupEnglish", "English only")}
                  </div>
                  {groups.en.map(renderDownloadedModel)}
                </>
              )}
            </>
          );
        })() : (
          <div className="px-4 py-6 text-sm text-text/60 text-center">
            {t("models.noInstalled")}
          </div>
        )}
      </SettingsGroup>

      <SettingsGroup
        title={t("models.availableTitle")}
        description={t("models.availableDescription")}
      >
        {downloadableModels.length > 0 ? (() => {
          const groups = groupModels(downloadableModels);
          return (
            <>
              {groups.vi.length > 0 && (
                <>
                  <div className="px-4 pt-3 pb-1 text-xs text-text/40">
                    🇻🇳 {t("modelSelector.groupVietnamese", "Tiếng Việt")}
                  </div>
                  {groups.vi.map(renderDownloadableModel)}
                </>
              )}
              {groups.multi.length > 0 && (
                <>
                  <div className="px-4 pt-3 pb-1 text-xs text-text/40">
                    🌐 {t("modelSelector.groupMultilingual", "Đa ngôn ngữ")}
                  </div>
                  {groups.multi.map(renderDownloadableModel)}
                </>
              )}
              {groups.en.length > 0 && (
                <>
                  <div className="px-4 pt-3 pb-1 text-xs text-text/40">
                    🇬🇧 {t("modelSelector.groupEnglish", "English only")}
                  </div>
                  {groups.en.map(renderDownloadableModel)}
                </>
              )}
            </>
          );
        })() : (
          <div className="px-4 py-6 text-sm text-text/60 text-center">
            {t("models.noAvailable")}
          </div>
        )}
      </SettingsGroup>

      {diarizationModels.length > 0 && (
        <SettingsGroup
          title={t("models.diarizationTitle", "Speaker Diarization")}
          description={t("models.diarizationDescription", "Models for identifying and separating speakers in transcripts.")}
        >
          {diarizationModels.map((model) => {
            const progress = downloadProgress[model.id];
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
                      {formatModelSize(Number(model.size_mb))}
                    </p>
                  </div>
                  <div className="flex items-center gap-2 shrink-0">
                    {!model.is_downloaded && (
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
                    )}
                    {model.is_downloaded && !extractingModels.has(model.id) && (
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
          })}
        </SettingsGroup>
      )}
    </div>
  );
};

export default ModelsSettings;
