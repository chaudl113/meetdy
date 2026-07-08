import React from "react";
import { useTranslation } from "react-i18next";
import { Loader2, Volume2, VolumeX } from "lucide-react";
import { useEdgeTTS } from "../../hooks/useEdgeTTS";
import { useSettingsStore } from "../../stores/settingsStore";

const LANG_TO_VOICE: Record<string, string> = {
  vi: "vi-VN-HoaiMyNeural",
  en: "en-US-JennyNeural",
  ja: "ja-JP-NanamiNeural",
  zh: "zh-CN-XiaoxiaoNeural",
  ko: "ko-KR-SunHiNeural",
  de: "de-DE-KatjaNeural",
  fr: "fr-FR-DeniseNeural",
  es: "es-ES-ElviraNeural",
  it: "it-IT-ElsaNeural",
  pl: "pl-PL-ZofiaNeural",
  ru: "ru-RU-SvetlanaNeural",
};

interface TTSButtonProps {
  getText: () => string;
  voice?: string;
  className?: string;
  title?: string;
}

export const TTSButton: React.FC<TTSButtonProps> = ({
  getText,
  voice,
  className = "",
  title,
}) => {
  const { t } = useTranslation();
  const { speak, stop, isPlaying, isLoading } = useEdgeTTS();
  const appLanguage = useSettingsStore((s) => s.settings?.app_language ?? "en");
  const resolvedVoice = voice ?? LANG_TO_VOICE[appLanguage] ?? "en-US-JennyNeural";

  const handleClick = () => {
    if (isPlaying) {
      stop();
    } else {
      speak(getText(), resolvedVoice);
    }
  };

  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={isLoading}
      title={title ?? t("tts.readTranscript")}
      className={`flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium rounded-lg transition-colors ${
        isPlaying
          ? "text-logo-primary bg-logo-primary/10 hover:bg-logo-primary/20"
          : "text-text/60 hover:text-text hover:bg-mid-gray/10"
      } disabled:opacity-30 ${className}`}
    >
      {isLoading ? (
        <Loader2 size={14} className="animate-spin" />
      ) : isPlaying ? (
        <VolumeX size={14} />
      ) : (
        <Volume2 size={14} />
      )}
      {isPlaying ? t("tts.stop") : t("tts.read")}
    </button>
  );
};

export default TTSButton;
