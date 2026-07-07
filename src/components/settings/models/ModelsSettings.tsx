import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import {
  Check,
  CheckCircle2,
  ChevronDown,
  Download,
  Info,
  Loader2,
  MoreVertical,
  Server,
  Trash2,
} from "lucide-react";
import { commands, type FunasrRuntimeStatus, type ModelInfo } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { ProgressBar } from "../../shared";
import { formatModelSize } from "../../../lib/utils/format";
import {
  getTranslatedModelDescription,
  getTranslatedModelName,
} from "../../../lib/utils/modelTranslation";

const FUNASR_MODEL_OPTIONS = [
  {
    value: "sensevoice",
    label: "SenseVoice",
    description: "Fast for Chinese/English/Japanese/Korean; can drift on Vietnamese",
  },
  {
    value: "paraformer",
    label: "Paraformer",
    description: "Chinese-focused general ASR",
  },
  {
    value: "paraformer-en",
    label: "Paraformer EN",
    description: "English-focused ASR",
  },
  {
    value: "fun-asr-nano",
    label: "FunASR Nano",
    description: "Recommended for Vietnamese and multilingual meetings",
  },
];

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

const SectionHeader: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => (
  <h2 className="px-1 text-xs font-medium text-logo-primary uppercase tracking-wide">
    {children}
  </h2>
);

const InstalledModelCard: React.FC<{
  model: ModelInfo;
  isActive: boolean;
  isBusy: boolean;
  onActivate: () => void;
  onDelete: () => void;
}> = ({ model, isActive, isBusy, onActivate, onDelete }) => {
  const { t } = useTranslation();
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [menuOpen]);

  return (
    <div className="flex items-start justify-between gap-4 p-4 rounded-lg border border-mid-gray/20 bg-background">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <p className="text-sm font-semibold truncate">
            {getTranslatedModelName(model, t)}
          </p>
          {isActive && (
            <span className="inline-flex items-center gap-1 text-xs font-medium text-logo-primary bg-logo-primary/10 px-2 py-0.5 rounded">
              <Check size={12} />
              {t("modelSelector.active", "Active")}
            </span>
          )}
        </div>
        <p className="text-xs text-text/60 italic mt-1">
          {getTranslatedModelDescription(model, t)}
        </p>
        <div className="flex items-center gap-4 mt-2 text-xs text-text/50 tabular-nums">
          <span>{formatModelSize(Number(model.size_mb))}</span>
        </div>
      </div>
      <div className="flex items-center gap-2 shrink-0">
        <span className="inline-flex items-center gap-1 text-xs font-medium text-emerald-600 bg-emerald-500/10 px-2 py-1 rounded">
          <CheckCircle2 size={12} />
          {t("models.upToDate", "Up to date")}
        </span>
        <div className="relative" ref={menuRef}>
          <button
            type="button"
            onClick={() => setMenuOpen((v) => !v)}
            disabled={isBusy}
            className="w-8 h-8 flex items-center justify-center rounded-lg border border-mid-gray/30 bg-background text-text/70 hover:text-logo-primary hover:border-logo-primary disabled:opacity-50"
            aria-label="More actions"
          >
            <MoreVertical size={14} />
          </button>
          {menuOpen && (
            <div className="absolute right-0 top-full mt-1 min-w-[160px] rounded-lg border border-mid-gray/20 bg-background shadow-lg z-20 overflow-hidden">
              {!isActive && (
                <button
                  type="button"
                  onClick={() => {
                    setMenuOpen(false);
                    onActivate();
                  }}
                  className="w-full text-left px-3 py-2 text-sm hover:bg-logo-primary/10 flex items-center gap-2"
                >
                  <Check size={14} />
                  {t("models.setActive", "Set as active")}
                </button>
              )}
              {!isActive && (
                <button
                  type="button"
                  onClick={() => {
                    setMenuOpen(false);
                    onDelete();
                  }}
                  className="w-full text-left px-3 py-2 text-sm text-red-500 hover:bg-red-500/10 flex items-center gap-2"
                >
                  <Trash2 size={14} />
                  {t("modelSelector.delete", "Delete")}
                </button>
              )}
              {isActive && (
                <div className="px-3 py-2 text-xs text-text/50">
                  {t(
                    "models.activeCannotDelete",
                    "Active model cannot be deleted.",
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

const DownloadableModelCard: React.FC<{
  model: ModelInfo;
  progress?: DownloadProgress;
  isExtracting: boolean;
  onDownload: () => void;
}> = ({ model, progress, isExtracting, onDownload }) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const isDownloading = !!progress;

  return (
    <div className="rounded-lg border border-mid-gray/20 bg-background">
      <div className="flex items-start justify-between gap-4 p-4">
        <div className="min-w-0 flex-1">
          <p className="text-sm font-semibold truncate">
            {getTranslatedModelName(model, t)}
          </p>
          <p className="text-xs text-text/60 italic mt-1">
            {getTranslatedModelDescription(model, t)}
          </p>
          <p className="text-xs text-text/50 mt-2 tabular-nums">
            {t("modelSelector.downloadSize", "Download size")} ·{" "}
            {formatModelSize(Number(model.size_mb))}
          </p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <button
            type="button"
            disabled={isDownloading || isExtracting}
            onClick={onDownload}
            className="inline-flex items-center gap-1.5 text-xs font-medium px-3 h-8 rounded-lg bg-logo-primary hover:bg-logo-primary/90 text-white transition-colors disabled:opacity-50"
          >
            <Download size={12} />
            {isDownloading
              ? `${Math.max(0, Math.min(100, Math.round(progress.percentage)))}%`
              : isExtracting
                ? t("modelSelector.extractingGeneric", "Extracting…")
                : t("modelSelector.download", "Download")}
          </button>
          <button
            type="button"
            onClick={() => setExpanded((v) => !v)}
            className="w-8 h-8 flex items-center justify-center rounded-lg border border-mid-gray/30 bg-background text-text/70 hover:text-logo-primary hover:border-logo-primary"
            aria-label={expanded ? "Collapse details" : "Expand details"}
            aria-expanded={expanded}
          >
            <ChevronDown
              size={14}
              className={`transition-transform ${expanded ? "rotate-180" : ""}`}
            />
          </button>
        </div>
      </div>
      {isDownloading && (
        <div className="px-4 pb-3">
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
      {expanded && (
        <div className="px-4 pb-4 pt-2 border-t border-mid-gray/10 text-xs text-text/70 space-y-1">
          <div>
            <span className="text-text/50">{t("models.idLabel")}</span>{" "}
            <span className="font-mono">{model.id}</span>
          </div>
          <div>
            <span className="text-text/50">
              {t("models.engine", "Engine")}:
            </span>{" "}
            {model.engine_type}
          </div>
          <div className="flex gap-4">
            <span>
              <span className="text-text/50">
                {t("models.accuracy", "Accuracy")}:
              </span>{" "}
              {model.accuracy_score}/10
            </span>
            <span>
              <span className="text-text/50">
                {t("models.speed", "Speed")}:
              </span>{" "}
              {model.speed_score}/10
            </span>
          </div>
        </div>
      )}
    </div>
  );
};

const FunasrRuntimeCard: React.FC<{
  status: FunasrRuntimeStatus | null;
  setupStatus: string | null;
  isSettingUp: boolean;
  selectedModel: string;
  onModelChange: (model: string) => void;
  onSetup: () => void;
  onDelete: () => void;
}> = ({
  status,
  setupStatus,
  isSettingUp,
  selectedModel,
  onModelChange,
  onSetup,
  onDelete,
}) => {
  const { t } = useTranslation();
  const installed = status?.installed ?? false;
  const serverRunning = status?.server_running ?? false;
  const downloadPercentage = status?.download_percentage ?? null;
  const clampedPercentage =
    downloadPercentage === null
      ? null
      : Math.max(0, Math.min(100, downloadPercentage));
  const roundedPercentage =
    clampedPercentage === null ? null : Math.round(clampedPercentage);
  const selectedModelInfo =
    FUNASR_MODEL_OPTIONS.find((option) => option.value === selectedModel) ??
    FUNASR_MODEL_OPTIONS[0];

  return (
    <div className="rounded-lg border border-mid-gray/20 bg-background p-4">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <Server size={16} className="text-logo-primary" />
            <p className="text-sm font-semibold">
              {t("models.funasr.title", "FunASR Local Runtime")}
            </p>
            <span
              className={`inline-flex items-center gap-1 text-xs font-medium px-2 py-0.5 rounded ${
                installed
                  ? "bg-emerald-500/10 text-emerald-600"
                  : "bg-mid-gray/20 text-text/50"
              }`}
            >
              {installed
                ? t("models.funasr.installed", "Installed")
                : t("models.funasr.notInstalled", "Not installed")}
            </span>
          </div>
          <p className="text-xs text-text/60 italic mt-1">
            {t(
              "models.funasr.description",
              "Local batch transcription service for Meeting mode. Download/setup this before selecting FunASR in meetings.",
            )}
          </p>
          <div className="mt-3 grid gap-3">
            <label className="grid gap-1.5 max-w-sm">
              <span className="text-xs font-medium text-text/70">
                {t("models.funasr.model", "Model")}
              </span>
              <select
                value={selectedModel}
                disabled={isSettingUp}
                onChange={(event) => onModelChange(event.target.value)}
                className="w-full appearance-none rounded-lg border border-mid-gray/30 bg-background px-3 py-2 text-sm text-text focus:outline-none focus:border-logo-primary disabled:opacity-50"
              >
                {FUNASR_MODEL_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label} - {option.value}
                  </option>
                ))}
              </select>
              <span className="text-xs text-text/50">
                {selectedModelInfo.description}
              </span>
              {selectedModel === "sensevoice" && (
                <span className="text-xs text-amber-600">
                  {t(
                    "models.funasr.senseVoiceVietnameseWarning",
                    "Not recommended for Vietnamese: it can auto-detect speech as Chinese.",
                  )}
                </span>
              )}
              <span className="text-xs text-text/50">
                {t(
                  "models.funasr.modelSetupHint",
                  "After changing model, press Start/Verify to download or load that selected model.",
                )}
              </span>
            </label>
          </div>
          <div className="mt-3 text-xs text-text/50 space-y-1">
            <div>
              {t("models.funasr.server", "Server")}:{" "}
              <span className={serverRunning ? "text-emerald-600" : ""}>
                {serverRunning
                  ? t("models.funasr.running", "Running")
                  : t("models.funasr.stopped", "Stopped")}
              </span>
            </div>
            {status?.log_path && (
              <div className="break-all">
                {t("models.funasr.log", "Log")}:{" "}
                <span className="font-mono">{status.log_path}</span>
              </div>
            )}
            {(setupStatus || status?.message) && (
              <div className="text-logo-primary">
                {setupStatus || status?.message}
              </div>
            )}
            {clampedPercentage !== null && !serverRunning && (
              <div className="pt-2 space-y-2">
                <div className="flex items-center justify-between gap-3">
                  <span className="text-xs font-medium text-text/70">
                    {t("models.funasr.downloading", "Downloading model")}
                  </span>
                  <span className="rounded bg-logo-primary/10 px-2 py-0.5 text-xs font-semibold tabular-nums text-logo-primary">
                    {roundedPercentage}%
                  </span>
                </div>
                <div className="h-2 w-full overflow-hidden rounded-full bg-mid-gray/20">
                  <div
                    className="h-full rounded-full bg-logo-primary transition-[width] duration-300"
                    style={{ width: `${clampedPercentage}%` }}
                  />
                </div>
              </div>
            )}
          </div>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {installed && (
            <button
              type="button"
              disabled={isSettingUp}
              onClick={onDelete}
              className="inline-flex items-center gap-1.5 text-xs font-medium px-3 h-8 rounded-lg border border-red-500/30 text-red-500 hover:bg-red-500/10 disabled:opacity-50"
            >
              <Trash2 size={12} />
              {t("modelSelector.delete", "Delete")}
            </button>
          )}
          <button
            type="button"
            disabled={isSettingUp}
            onClick={onSetup}
            className="inline-flex items-center gap-1.5 text-xs font-medium px-3 h-8 rounded-lg bg-logo-primary hover:bg-logo-primary/90 text-white transition-colors disabled:opacity-50"
          >
            {isSettingUp ? (
              <Loader2 size={12} className="animate-spin" />
            ) : (
              <Download size={12} />
            )}
            {installed
              ? serverRunning
                ? t("models.funasr.verify", "Verify")
                : t("models.funasr.start", "Start")
              : t("models.funasr.setup", "Setup")}
          </button>
        </div>
      </div>
    </div>
  );
};

export const ModelsSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting } = useSettings();
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
  const [funasrStatus, setFunasrStatus] = useState<FunasrRuntimeStatus | null>(
    null,
  );
  const [funasrSetupStatus, setFunasrSetupStatus] = useState<string | null>(
    null,
  );
  const [isFunasrSettingUp, setIsFunasrSettingUp] = useState(false);
  const selectedFunasrModel = getSetting("funasr_model") || "fun-asr-nano";

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

  const loadFunasrStatus = async () => {
    const result = await commands.getFunasrRuntimeStatus();
    if (result.status === "ok") {
      setFunasrStatus(result.data);
    }
  };

  useEffect(() => {
    loadModels();
    loadCurrentModel();
    loadFunasrStatus();

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

    const funasrSetupUnlisten = listen<string>(
      "funasr_setup_status",
      (event) => {
        setFunasrSetupStatus(event.payload);
      },
    );

    return () => {
      stateUnlisten.then((fn) => fn());
      progressUnlisten.then((fn) => fn());
      completeUnlisten.then((fn) => fn());
      extractStartUnlisten.then((fn) => fn());
      extractDoneUnlisten.then((fn) => fn());
      extractFailUnlisten.then((fn) => fn());
      funasrSetupUnlisten.then((fn) => fn());
    };
  }, []);

  const shouldPollFunasrStatus =
    isFunasrSettingUp ||
    (funasrStatus?.download_percentage !== null &&
      funasrStatus?.download_percentage !== undefined &&
      !funasrStatus?.server_running);

  useEffect(() => {
    if (!shouldPollFunasrStatus) return;
    const interval = window.setInterval(() => {
      loadFunasrStatus();
    }, 2000);
    return () => window.clearInterval(interval);
  }, [shouldPollFunasrStatus]);

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

  const handleSetupFunasr = async () => {
    setErrorMessage(null);
    setFunasrSetupStatus(
      funasrStatus?.server_running
        ? `Verifying FunASR model '${selectedFunasrModel}'...`
        : `Starting FunASR with model '${selectedFunasrModel}'...`,
    );
    setIsFunasrSettingUp(true);
    const result = await commands.setupFunasrRuntime();
    if (result.status === "error") {
      setErrorMessage(result.error);
      setFunasrSetupStatus(`FunASR setup failed: ${result.error}`);
    } else {
      setFunasrSetupStatus(`FunASR verified with model '${selectedFunasrModel}'.`);
    }
    setIsFunasrSettingUp(false);
    await loadFunasrStatus();
  };

  const handleFunasrModelChange = async (model: string) => {
    setErrorMessage(null);
    setFunasrSetupStatus(
      `Selected '${model}'. Press Start/Verify to load this model.`,
    );
    await updateSetting("funasr_model", model);
    await loadFunasrStatus();
  };

  const handleDeleteFunasr = async () => {
    setErrorMessage(null);
    const result = await commands.deleteFunasrRuntime();
    if (result.status === "error") {
      setErrorMessage(result.error);
    }
    setFunasrSetupStatus(null);
    await loadFunasrStatus();
  };

  const downloadedModels = models.filter((m) => m.is_downloaded);
  const downloadableModels = models.filter((m) => !m.is_downloaded);

  const navigateToGeneral = () => {
    window.dispatchEvent(
      new CustomEvent("app:navigate", { detail: "general" }),
    );
  };

  return (
    <div className="w-full space-y-6">
      <div className="px-1">
        <h1 className="text-2xl font-semibold text-text">
          {t("models.headerTitle", "Models")}
        </h1>
        <p className="text-sm text-mid-gray mt-1">
          {t(
            "models.headerSubtitle",
            "Manage transcription models on your device. Higher quality models provide better accuracy but require more storage.",
          )}
        </p>
      </div>

      {errorMessage && (
        <div className="px-4 py-3 rounded-lg bg-red-500/10 border border-red-500/30 text-sm text-red-500">
          {errorMessage}
        </div>
      )}

      <div className="space-y-2">
        <SectionHeader>
          {t("models.installedTitle", "Installed Models")}
        </SectionHeader>
        {downloadedModels.length > 0 ? (
          <div className="space-y-3">
            {downloadedModels.map((m) => (
              <InstalledModelCard
                key={m.id}
                model={m}
                isActive={m.id === currentModelId}
                isBusy={busyModelId === m.id}
                onActivate={() => handleSelect(m.id)}
                onDelete={() => handleDelete(m.id)}
              />
            ))}
          </div>
        ) : (
          <div className="px-4 py-6 text-sm text-text/60 text-center rounded-lg border border-mid-gray/20 bg-background">
            {t("models.noInstalled", "No models installed yet.")}
          </div>
        )}
      </div>

      <div className="space-y-2">
        <SectionHeader>
          {t("models.availableTitle", "Available to Download")}
        </SectionHeader>
        <FunasrRuntimeCard
          status={funasrStatus}
          setupStatus={funasrSetupStatus}
          isSettingUp={isFunasrSettingUp}
          selectedModel={selectedFunasrModel}
          onModelChange={handleFunasrModelChange}
          onSetup={handleSetupFunasr}
          onDelete={handleDeleteFunasr}
        />
        {downloadableModels.length > 0 ? (
          <div className="space-y-3">
            {downloadableModels.map((m) => (
              <DownloadableModelCard
                key={m.id}
                model={m}
                progress={downloadProgress.get(m.id)}
                isExtracting={extractingModels.has(m.id)}
                onDownload={() => handleDownload(m.id)}
              />
            ))}
          </div>
        ) : (
          <div className="px-4 py-6 text-sm text-text/60 text-center rounded-lg border border-mid-gray/20 bg-background">
            {t("models.noAvailable", "All available models are installed.")}
          </div>
        )}
      </div>

      <div className="flex items-start gap-3 px-4 py-3 rounded-lg bg-logo-primary/5 border border-logo-primary/20 text-sm text-text/80">
        <Info size={16} className="text-logo-primary shrink-0 mt-0.5" />
        <p>
          {t(
            "models.footerInfo",
            "Larger models deliver higher accuracy but take more time to download and require more storage.",
          )}{" "}
          {t("models.footerChangeIn", "You can change the active model in")}{" "}
          <button
            type="button"
            onClick={navigateToGeneral}
            className="text-logo-primary hover:underline font-medium"
          >
            {t("sidebar.general", "General")}
          </button>{" "}
          {t("models.footerSettings", "settings.")}
        </p>
      </div>
    </div>
  );
};

export default ModelsSettings;
