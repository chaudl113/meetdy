import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, type ModelInfo } from "@/bindings";
import ModelCard from "./ModelCard";
import MeetdyTextLogo from "../icons/MeetdyTextLogo";
import PermissionsStep from "./PermissionsStep";

interface OnboardingProps {
  onModelSelected: () => void;
}

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

type OnboardingStep = "permissions" | "model";

const Onboarding: React.FC<OnboardingProps> = ({ onModelSelected }) => {
  const { t } = useTranslation();
  const [step, setStep] = useState<OnboardingStep>("permissions");
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);
  const [downloading, setDownloading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadModels();
  }, []);

  const loadModels = async () => {
    try {
      const result = await commands.getAvailableModels();
      if (result.status === "ok") {
        // Only show downloadable STT models for onboarding (exclude diarization)
        setAvailableModels(
          result.data.filter(
            (m) => !m.is_downloaded && m.engine_type !== "Diarization"
          )
        );
      } else {
        setError(t("onboarding.errors.loadModels"));
      }
    } catch (err) {
      console.error("Failed to load models:", err);
      setError(t("onboarding.errors.loadModels"));
    }
  };

  const handleDownloadModel = async (modelId: string) => {
    setDownloading(true);
    setError(null);

    // Immediately transition to main app - download will continue in footer
    onModelSelected();

    try {
      const result = await commands.downloadModel(modelId);
      if (result.status === "error") {
        console.error("Download failed:", result.error);
        setError(t("onboarding.errors.downloadModel", { error: result.error }));
        setDownloading(false);
      }
    } catch (err) {
      console.error("Download failed:", err);
      setError(t("onboarding.errors.downloadModel", { error: String(err) }));
      setDownloading(false);
    }
  };

  const recommendedModels = availableModels.filter((m) => m.is_recommended);
  const otherModels = availableModels.filter((m) => !m.is_recommended);
  const otherGroups = groupModels(otherModels);

  if (step === "permissions") {
    return (
      <div className="h-screen w-screen flex flex-col items-center justify-center p-8">
        <div className="w-full max-w-md">
          <div className="flex flex-col items-center gap-2 mb-8">
            <MeetdyTextLogo width={200} />
          </div>
          <PermissionsStep onContinue={() => setStep("model")} />
        </div>
      </div>
    );
  }

  return (
    <div className="h-screen w-screen flex flex-col p-6 gap-4 inset-0">
      <div className="flex flex-col items-center gap-2 shrink-0">
        <MeetdyTextLogo width={200} />
        <p className="text-text/70 max-w-md font-medium mx-auto">
          {t("onboarding.subtitle")}
        </p>
      </div>

      <div className="max-w-[600px] w-full mx-auto text-center flex-1 flex flex-col min-h-0">
        {error && (
          <div className="bg-red-500/10 border border-red-500/20 rounded-lg p-4 mb-4 shrink-0">
            <p className="text-red-400 text-sm">{error}</p>
          </div>
        )}

        <div className="flex flex-col gap-4 overflow-y-auto min-h-0 flex-1 pr-1">
          {/* Recommended models */}
          {recommendedModels.map((model) => (
            <ModelCard
              key={model.id}
              model={model}
              variant="featured"
              disabled={downloading}
              onSelect={handleDownloadModel}
            />
          ))}

          {/* Other models grouped by language */}
          {otherGroups.vi.length > 0 && (
            <>
              <div className="text-xs text-text/40 text-left px-1">
                🇻🇳 {t("modelSelector.groupVietnamese", "Tiếng Việt")}
              </div>
              <div className="grid grid-cols-2 gap-2">
                {otherGroups.vi
                  .sort((a, b) => Number(a.size_mb) - Number(b.size_mb))
                  .map((model) => (
                    <ModelCard
                      key={model.id}
                      model={model}
                      disabled={downloading}
                      onSelect={handleDownloadModel}
                    />
                  ))}
              </div>
            </>
          )}

          {otherGroups.multi.length > 0 && (
            <>
              <div className="text-xs text-text/40 text-left px-1">
                🌐 {t("modelSelector.groupMultilingual", "Đa ngôn ngữ")}
              </div>
              <div className="grid grid-cols-2 gap-2">
                {otherGroups.multi
                  .sort((a, b) => Number(a.size_mb) - Number(b.size_mb))
                  .map((model) => (
                    <ModelCard
                      key={model.id}
                      model={model}
                      disabled={downloading}
                      onSelect={handleDownloadModel}
                    />
                  ))}
              </div>
            </>
          )}

          {otherGroups.en.length > 0 && (
            <>
              <div className="text-xs text-text/40 text-left px-1">
                🇬🇧 {t("modelSelector.groupEnglish", "English only")}
              </div>
              <div className="grid grid-cols-2 gap-2">
                {otherGroups.en
                  .sort((a, b) => Number(a.size_mb) - Number(b.size_mb))
                  .map((model) => (
                    <ModelCard
                      key={model.id}
                      model={model}
                      disabled={downloading}
                      onSelect={handleDownloadModel}
                    />
                  ))}
              </div>
            </>
          )}
        </div>

        <div className="shrink-0 pt-2 flex flex-col items-center gap-1">
          <button
            type="button"
            onClick={onModelSelected}
            className="text-sm text-text/40 hover:text-text/60 transition-colors"
          >
            {t("onboarding.skipDownload", "Skip — download later")}
          </button>
          <p className="text-xs text-text/30">
            {t("onboarding.skipNote", "You can download models later in Settings")}
          </p>
        </div>
      </div>
    </div>
  );
};

export default Onboarding;
