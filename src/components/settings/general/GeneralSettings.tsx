import React from "react";
import { useTranslation } from "react-i18next";
import { type } from "@tauri-apps/plugin-os";
import {
  AudioLines,
  Bell,
  FileText,
  Globe,
  Info,
  MessageSquare,
  Mic,
  Save,
  Speaker,
  Type,
  Volume2,
  Zap,
} from "lucide-react";
import { MicrophoneSelector } from "../MicrophoneSelector";
import { LanguageSelector } from "../LanguageSelector";
import { ShortcutRecorder } from "../ShortcutRecorder";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { OutputDeviceSelector } from "../OutputDeviceSelector";
import { PushToTalk } from "../PushToTalk";
import { AudioFeedback } from "../AudioFeedback";
import { useSettings } from "../../../hooks/useSettings";
import { VolumeSlider } from "../VolumeSlider";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import {
  formatKeyCombination,
  type OSType,
} from "../../../lib/utils/keyboard";

const ICON = "w-4 h-4";

const ShortcutCard: React.FC<{
  label: string;
  combination: string;
  osType: OSType;
}> = ({ label, combination, osType }) => {
  const keys = formatKeyCombination(combination, osType).split("+");
  return (
    <div className="flex items-center justify-between gap-3 px-4 py-3 rounded-lg border border-mid-gray/20 bg-background">
      <div className="flex items-center gap-3 min-w-0">
        <div className="w-7 h-7 rounded-md bg-logo-primary/10 flex items-center justify-center text-logo-primary text-xs font-semibold shrink-0">
          {label.charAt(0).toUpperCase()}
        </div>
        <span className="text-sm font-medium truncate">{label}</span>
      </div>
      <div className="flex items-center gap-1 shrink-0">
        {keys.map((k, i) => (
          <kbd
            key={`${k}-${i}`}
            className="px-2 py-0.5 text-xs font-semibold bg-mid-gray/10 border border-mid-gray/40 rounded text-text"
          >
            {k}
          </kbd>
        ))}
      </div>
    </div>
  );
};

export const GeneralSettings: React.FC = () => {
  const { t } = useTranslation();
  const { audioFeedbackEnabled, getSetting, updateSetting } = useSettings();

  const autoSave = getSetting("auto_save") ?? true;
  const autoTranscribe = getSetting("auto_transcribe") ?? false;
  const autoSummary = getSetting("auto_summary") ?? true;
  const notifyCompleted = getSetting("notify_completed") ?? true;
  const notifyFailed = getSetting("notify_failed") ?? true;
  const notifyAppUpdates = getSetting("notify_app_updates") ?? false;

  const [osType, setOsType] = React.useState<OSType>("unknown");
  React.useEffect(() => {
    try {
      const detected = type();
      setOsType(
        detected === "macos"
          ? "macos"
          : detected === "windows"
            ? "windows"
            : detected === "linux"
              ? "linux"
              : "unknown",
      );
    } catch {
      setOsType("unknown");
    }
  }, []);

  const bindings = getSetting("bindings") || {};
  const bindingEntries = Object.entries(bindings);

  return (
    <div className="w-full space-y-6">
      <div className="px-1">
        <h1 className="text-2xl font-semibold text-text">
          {t("settings.general.header.title", "General")}
        </h1>
        <p className="text-sm text-mid-gray mt-1">
          {t(
            "settings.general.header.subtitle",
            "Manage your application preferences and settings.",
          )}
        </p>
      </div>

      <SettingsGroup title={t("settings.general.title")}>
        <ShortcutRecorder
          shortcutId="transcribe"
          grouped={true}
          icon={<Type className={ICON} />}
        />
        <LanguageSelector
          descriptionMode="tooltip"
          grouped={true}
          icon={<Globe className={ICON} />}
        />
        <PushToTalk
          descriptionMode="tooltip"
          grouped={true}
          icon={<MessageSquare className={ICON} />}
        />
      </SettingsGroup>

      <SettingsGroup title={t("settings.sound.title")}>
        <MicrophoneSelector
          descriptionMode="tooltip"
          grouped={true}
          icon={<Mic className={ICON} />}
        />
        <AudioFeedback
          descriptionMode="tooltip"
          grouped={true}
          icon={<AudioLines className={ICON} />}
        />
        <OutputDeviceSelector
          descriptionMode="tooltip"
          grouped={true}
          disabled={!audioFeedbackEnabled}
          icon={<Speaker className={ICON} />}
        />
        <VolumeSlider
          disabled={!audioFeedbackEnabled}
          icon={<Volume2 className={ICON} />}
        />
      </SettingsGroup>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <SettingsGroup
          title={t("settings.application.title", "Application")}
        >
          <ToggleSwitch
            checked={autoSave}
            onChange={(v) => updateSetting("auto_save", v)}
            label={t("settings.application.autoSave.label", "Auto Save")}
            description={t(
              "settings.application.autoSave.description",
              "Automatically save meetings after recording.",
            )}
            descriptionMode="inline"
            grouped={true}
            icon={<Save className={ICON} />}
          />
          <ToggleSwitch
            checked={autoTranscribe}
            onChange={(v) => updateSetting("auto_transcribe", v)}
            label={t(
              "settings.application.autoTranscribe.label",
              "Auto Transcribe",
            )}
            description={t(
              "settings.application.autoTranscribe.description",
              "Start transcription automatically after recording.",
            )}
            descriptionMode="inline"
            grouped={true}
            icon={<AudioLines className={ICON} />}
          />
          <ToggleSwitch
            checked={autoSummary}
            onChange={(v) => updateSetting("auto_summary", v)}
            label={t(
              "settings.application.showSummary.label",
              "Show AI Summary by Default",
            )}
            description={t(
              "settings.application.showSummary.description",
              "Display AI summary when opening a meeting.",
            )}
            descriptionMode="inline"
            grouped={true}
            icon={<FileText className={ICON} />}
          />
        </SettingsGroup>

        <SettingsGroup
          title={t("settings.notifications.title", "Notifications")}
        >
          <ToggleSwitch
            checked={notifyCompleted}
            onChange={(v) => updateSetting("notify_completed", v)}
            label={t(
              "settings.notifications.completed.label",
              "Transcription Completed",
            )}
            description={t(
              "settings.notifications.completed.description",
              "Notify me when transcription is completed.",
            )}
            descriptionMode="inline"
            grouped={true}
            icon={<Bell className={ICON} />}
          />
          <ToggleSwitch
            checked={notifyFailed}
            onChange={(v) => updateSetting("notify_failed", v)}
            label={t(
              "settings.notifications.failed.label",
              "Transcription Failed",
            )}
            description={t(
              "settings.notifications.failed.description",
              "Notify me when transcription fails.",
            )}
            descriptionMode="inline"
            grouped={true}
            icon={<Zap className={ICON} />}
          />
          <ToggleSwitch
            checked={notifyAppUpdates}
            onChange={(v) => updateSetting("notify_app_updates", v)}
            label={t(
              "settings.notifications.appUpdates.label",
              "App Updates",
            )}
            description={t(
              "settings.notifications.appUpdates.description",
              "Notify me about new updates.",
            )}
            descriptionMode="inline"
            grouped={true}
            icon={<Info className={ICON} />}
          />
        </SettingsGroup>
      </div>

      <div className="space-y-2">
        <div className="px-4 flex items-center justify-between">
          <div>
            <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
              {t("settings.shortcuts.title", "Keyboard Shortcuts")}
            </h2>
            <p className="text-xs text-mid-gray mt-1">
              {t(
                "settings.shortcuts.subtitle",
                "Quick access to common actions.",
              )}
            </p>
          </div>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
          {bindingEntries.map(([id, b]) => {
            if (!b) return null;
            return (
              <ShortcutCard
                key={id}
                label={t(
                  `settings.general.shortcut.bindings.${id}.name`,
                  b.name,
                )}
                combination={b.current_binding}
                osType={osType}
              />
            );
          })}
        </div>
      </div>
    </div>
  );
};
