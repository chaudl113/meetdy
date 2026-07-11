import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { ShowOverlay } from "../ShowOverlay";
import { TranslateToEnglish } from "../TranslateToEnglish";
import { ModelUnloadTimeoutSetting } from "../ModelUnloadTimeout";
import { CustomWords } from "../CustomWords";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { StartHidden } from "../StartHidden";
import { AutostartToggle } from "../AutostartToggle";
import { PasteMethodSetting } from "../PasteMethod";
import { ClipboardHandlingSetting } from "../ClipboardHandling";
import { DiarizationToggle } from "../DiarizationToggle";
import { commands } from "@/bindings";
import { useSettingsStore } from "../../../stores/settingsStore";

export const AdvancedSettings: React.FC = () => {
  const { t } = useTranslation();
  const settings = useSettingsStore((s) => s.settings);
  const [sonioxKey, setSonioxKey] = useState(settings?.soniox_api_key ?? "");
  const [funasrUrl, setFunasrUrl] = useState(
    settings?.funasr_base_url ?? "http://localhost:8000",
  );

  return (
    <div className="w-full space-y-6">
      <SettingsGroup title={t("settings.advanced.title")}>
        <StartHidden descriptionMode="tooltip" grouped={true} />
        <AutostartToggle descriptionMode="tooltip" grouped={true} />
        <ShowOverlay descriptionMode="tooltip" grouped={true} />
        <PasteMethodSetting descriptionMode="tooltip" grouped={true} />
        <ClipboardHandlingSetting descriptionMode="tooltip" grouped={true} />
        <TranslateToEnglish descriptionMode="tooltip" grouped={true} />
        <ModelUnloadTimeoutSetting descriptionMode="tooltip" grouped={true} />
        <CustomWords descriptionMode="tooltip" grouped />
        <DiarizationToggle descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

      <SettingsGroup
        title={t("settings.advanced.cloudServices", "Cloud / Local Services")}
      >
        <div className="px-4 py-3 space-y-3">
          <div className="space-y-1">
            <label className="text-sm font-medium text-text/80">
              {t("settings.advanced.sonioxApiKey", "Soniox API Key")}
            </label>
            <input
              type="password"
              value={sonioxKey}
              onChange={(e) => setSonioxKey(e.target.value)}
              onBlur={() => commands.changeSonioxApiKeySetting(sonioxKey)}
              placeholder={t(
                "settings.advanced.sonioxApiKeyPlaceholder",
                "Enter Soniox API key",
              )}
              className="w-full px-3 py-2 text-sm rounded-lg bg-mid-gray/10 border border-mid-gray/20 text-text placeholder-text/30 focus:outline-none focus:border-logo-primary/50"
            />
          </div>
          <div className="space-y-1">
            <label className="text-sm font-medium text-text/80">
              {t("settings.advanced.funasrUrl", "FunASR Server URL")}
            </label>
            <input
              type="text"
              value={funasrUrl}
              onChange={(e) => setFunasrUrl(e.target.value)}
              onBlur={() => commands.changeFunasrBaseUrlSetting(funasrUrl)}
              placeholder="http://localhost:8000"
              className="w-full px-3 py-2 text-sm rounded-lg bg-mid-gray/10 border border-mid-gray/20 text-text placeholder-text/30 focus:outline-none focus:border-logo-primary/50"
            />
          </div>
        </div>
      </SettingsGroup>
    </div>
  );
};
