import React, { useState, useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Languages, Copy, Check, Loader2 } from "lucide-react";
import { commands } from "@/bindings";

const TRANSLATE_LANGUAGES: { code: string; label: string }[] = [
  { code: "en", label: "English" },
  { code: "vi", label: "Tiếng Việt" },
  { code: "zh-CN", label: "中文 (简体)" },
  { code: "zh-TW", label: "中文 (繁體)" },
  { code: "ja", label: "日本語" },
  { code: "ko", label: "한국어" },
  { code: "fr", label: "Français" },
  { code: "de", label: "Deutsch" },
  { code: "es", label: "Español" },
  { code: "it", label: "Italiano" },
  { code: "pt", label: "Português" },
  { code: "ru", label: "Русский" },
  { code: "th", label: "ภาษาไทย" },
  { code: "id", label: "Bahasa Indonesia" },
];

interface TranslationSectionProps {
  transcript: string;
  translateTarget: string;
}

export const TranslationSection: React.FC<TranslationSectionProps> = ({
  transcript,
  translateTarget,
}) => {
  const { t } = useTranslation();
  const [translatedText, setTranslatedText] = useState<string | null>(null);
  const [isTranslating, setIsTranslating] = useState(false);
  const [translateError, setTranslateError] = useState<string | null>(null);
  const [translatedCopied, setTranslatedCopied] = useState(false);

  const handleTranslate = useCallback(
    async (targetLang: string) => {
      if (!transcript || !targetLang) {
        setTranslatedText(null);
        setTranslateError(null);
        return;
      }
      setIsTranslating(true);
      setTranslateError(null);
      try {
        const result = await commands.translateText(
          transcript,
          "auto",
          targetLang,
        );
        if (result.status === "ok") {
          setTranslatedText(result.data);
        } else {
          setTranslateError(result.error);
          setTranslatedText(null);
        }
      } catch (err) {
        setTranslateError(
          err instanceof Error ? err.message : "Translation failed",
        );
        setTranslatedText(null);
      } finally {
        setIsTranslating(false);
      }
    },
    [transcript],
  );

  useEffect(() => {
    if (translateTarget && transcript) {
      handleTranslate(translateTarget);
    } else {
      setTranslatedText(null);
      setTranslateError(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [transcript, translateTarget]);

  const handleCopyTranslated = async () => {
    if (!translatedText) return;
    try {
      await navigator.clipboard.writeText(translatedText);
      setTranslatedCopied(true);
      setTimeout(() => setTranslatedCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  if (!translateTarget) {
    return (
      <div className="bg-dark-gray/30 rounded-lg p-4">
        <p className="text-sm whitespace-pre-wrap">{transcript}</p>
      </div>
    );
  }

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
      <div className="bg-dark-gray/30 rounded-lg p-4">
        <p className="text-xs text-mid-gray mb-2 uppercase tracking-wide">
          {t("meeting.detail.translate.original", "Original")}
        </p>
        <p className="text-sm whitespace-pre-wrap">{transcript}</p>
      </div>
      <div className="bg-dark-gray/30 rounded-lg p-4 relative">
        <div className="flex items-center justify-between mb-2">
          <p className="text-xs text-mid-gray uppercase tracking-wide">
            {TRANSLATE_LANGUAGES.find(
              (l) => l.code === translateTarget,
            )?.label ?? translateTarget}
          </p>
          {translatedText && !isTranslating && (
            <button
              onClick={handleCopyTranslated}
              className="inline-flex items-center gap-1 text-[10px] text-mid-gray hover:text-white transition-colors"
            >
              {translatedCopied ? (
                <Check className="h-3 w-3" />
              ) : (
                <Copy className="h-3 w-3" />
              )}
            </button>
          )}
        </div>
        {isTranslating ? (
          <div className="flex items-center gap-2 text-sm text-mid-gray">
            <Loader2 className="h-4 w-4 animate-spin" />
            {t("meeting.detail.translate.loading", "Translating...")}
          </div>
        ) : translateError ? (
          <p className="text-sm text-red-400">{translateError}</p>
        ) : translatedText ? (
          <p className="text-sm whitespace-pre-wrap">{translatedText}</p>
        ) : (
          <p className="text-sm text-mid-gray italic">
            {t("meeting.detail.translate.empty", "No translation yet")}
          </p>
        )}
      </div>
    </div>
  );
};

export const TranslationLanguageSelect: React.FC<{
  value: string;
  onChange: (val: string) => void;
}> = ({ value, onChange }) => {
  const { t } = useTranslation();

  return (
    <div className="flex items-center gap-1.5 text-xs text-mid-gray">
      <Languages className="h-3.5 w-3.5" />
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="bg-dark-gray/50 border border-mid-gray/30 rounded px-2 py-1 text-xs text-white hover:border-mid-gray/60 focus:outline-none focus:border-logo-primary"
      >
        <option value="">
          {t("meeting.detail.translate.none", "No translation")}
        </option>
        {TRANSLATE_LANGUAGES.map((lang) => (
          <option key={lang.code} value={lang.code}>
            {lang.label}
          </option>
        ))}
      </select>
    </div>
  );
};

export default TranslationSection;
