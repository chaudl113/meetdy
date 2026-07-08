import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import { commands } from "@/bindings";

interface DiarizationToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const DiarizationToggle: React.FC<DiarizationToggleProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const [hasDiarizationModel, setHasDiarizationModel] = useState<boolean | null>(null);

    const enabled = getSetting("diarization_enabled") || false;

    useEffect(() => {
      commands.getAvailableModels().then((result) => {
        if (result.status === "ok") {
          const has = result.data.some(
            (m) => m.engine_type === "Diarization" && m.is_downloaded,
          );
          setHasDiarizationModel(has);
        }
      });
    }, []);

    const noModel = hasDiarizationModel === false;

    return (
      <div>
        <ToggleSwitch
          checked={enabled}
          onChange={(value) => updateSetting("diarization_enabled", value)}
          isUpdating={isUpdating("diarization_enabled")}
          label={t("settings.advanced.diarization.label")}
          description={t("settings.advanced.diarization.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
          disabled={noModel}
        />
        {noModel && (
          <p className="px-4 pb-2 text-xs text-yellow-500/80">
            {t("settings.advanced.diarization.noModelWarning", "Install a speaker diarization model first")}
          </p>
        )}
      </div>
    );
  },
);
