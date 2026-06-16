import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { Check, Copy, Loader2, RefreshCw, Sparkles } from "lucide-react";
import { commands } from "@/bindings";
import { useMeetingStore } from "../../../stores/meetingStore";
import { useRecordingConfigStore } from "../../../stores/recordingConfigStore";
import { LANGUAGES } from "../../../lib/constants/languages";

/** AI summary panel for completed meetings, with manual generate/regenerate. */
export const AISummaryPanel: React.FC = () => {
  const { t } = useTranslation();
  const { currentSession, sessionStatus, setCurrentSession } = useMeetingStore(
    useShallow((s) => ({
      currentSession: s.currentSession,
      sessionStatus: s.sessionStatus,
      setCurrentSession: s.setCurrentSession,
    })),
  );
  const { summaryLanguage, setSummaryLanguage } = useRecordingConfigStore();
  const [summary, setSummary] = useState<string>("");
  const [isLoading, setIsLoading] = useState(false);
  const [isGenerating, setIsGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const canGenerate =
    sessionStatus === "completed" && !!currentSession?.id && !!currentSession?.transcript_path;

  useEffect(() => {
    const loadSummary = async () => {
      if (!currentSession?.id || !currentSession.summary_path) {
        setSummary("");
        return;
      }

      setIsLoading(true);
      setError(null);
      try {
        const result = await commands.getMeetingSummary(currentSession.id);
        if (result.status === "ok") {
          setSummary(result.data ?? "");
        } else {
          setError(result.error);
        }
      } finally {
        setIsLoading(false);
      }
    };

    loadSummary();
  }, [currentSession?.id, currentSession?.summary_path]);

  const handleGenerate = async () => {
    if (!currentSession?.id || !canGenerate) return;
    setIsGenerating(true);
    setError(null);
    try {
      const result = await commands.generateMeetingSummary(
        currentSession.id,
        summaryLanguage === "auto" ? null : summaryLanguage,
      );
      if (result.status === "ok") {
        setSummary(result.data);
        setCurrentSession({
          ...currentSession,
          summary_path: currentSession.summary_path ?? `${currentSession.id}/summary.md`,
        });
      } else {
        setError(result.error);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to generate summary");
    } finally {
      setIsGenerating(false);
    }
  };

  const handleCopy = async () => {
    if (!summary) return;
    await navigator.clipboard.writeText(summary);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  if (sessionStatus === "recording") {
    return (
      <div className="flex flex-col items-center justify-center text-center py-16 px-6">
        <Sparkles className="text-text/30 mb-3" width={40} height={40} />
        <h3 className="text-base font-semibold mb-1">
          {t("recording.summary.emptyTitle")}
        </h3>
        <p className="text-sm text-text/60 max-w-sm">
          {t("recording.summary.emptyMessage")}
        </p>
      </div>
    );
  }

  return (
    <div className="rounded-xl border border-mid-gray/20 bg-background shadow-sm overflow-hidden">
      <div className="flex items-center justify-between px-4 py-3 border-b border-mid-gray/15 bg-mid-gray/[0.04]">
        <div className="flex items-center gap-2 text-sm font-semibold text-text">
          <Sparkles size={16} className="text-logo-primary" />
          {t("meeting.summary.title", "AI Summary")}
        </div>
        <div className="flex items-center gap-2">
          <select
            value={summaryLanguage}
            onChange={(e) => setSummaryLanguage(e.target.value)}
            className="h-8 rounded-lg border border-mid-gray/20 bg-background px-2 text-xs text-text/80 outline-none focus:border-logo-primary/60"
            title={t("meeting.summary.outputLanguage", "Summary language")}
          >
            <option value="auto">
              {t("meeting.summary.language.auto", "Auto")}
            </option>
            {LANGUAGES.filter((l) => l.value !== "auto").map((language) => (
              <option key={language.value} value={language.value}>
                {language.label}
              </option>
            ))}
          </select>
          {summary && (
            <button
              type="button"
              onClick={handleCopy}
              className="inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs text-text/60 hover:text-text hover:bg-mid-gray/10 rounded-lg"
            >
              {copied ? <Check size={14} className="text-green-500" /> : <Copy size={14} />}
              {copied ? t("common.copied", "Copied") : t("common.copy", "Copy")}
            </button>
          )}
          <button
            type="button"
            onClick={handleGenerate}
            disabled={!canGenerate || isGenerating}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-semibold text-white bg-logo-primary hover:opacity-90 rounded-lg disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isGenerating ? (
              <Loader2 size={14} className="animate-spin" />
            ) : summary ? (
              <RefreshCw size={14} />
            ) : (
              <Sparkles size={14} />
            )}
            {summary
              ? t("meeting.summary.regenerate", "Regenerate")
              : t("meeting.summary.generate", "Generate Summary")}
          </button>
        </div>
      </div>

      <div className="p-5">
        {error && (
          <div className="mb-4 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-500">
            {error}
          </div>
        )}
        {isLoading || isGenerating ? (
          <div className="flex items-center gap-2 text-sm text-text/60">
            <Loader2 size={16} className="animate-spin" />
            {isGenerating
              ? t("meeting.summary.generating", "Generating summary...")
              : t("common.loading", "Loading...")}
          </div>
        ) : summary ? (
          <div className="text-[15px] text-text whitespace-pre-wrap leading-7">
            {summary}
          </div>
        ) : (
          <div className="flex flex-col items-center justify-center py-10 text-center">
            <Sparkles className="text-text/30 mb-3" width={36} height={36} />
            <h3 className="text-base font-semibold mb-1">
              {t("meeting.summary.noSummary", "No summary yet")}
            </h3>
            <p className="text-sm text-text/60 max-w-sm mb-4">
              {canGenerate
                ? t(
                    "meeting.summary.generateHint",
                    "Generate an AI summary from the completed transcript.",
                  )
                : t(
                    "meeting.summary.noTranscript",
                    "Transcript required to generate summary.",
                  )}
            </p>
            <button
              type="button"
              onClick={handleGenerate}
              disabled={!canGenerate || isGenerating}
              className="inline-flex items-center gap-2 px-4 py-2 bg-logo-primary text-white rounded-lg hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <Sparkles size={16} />
              {t("meeting.summary.generate", "Generate Summary")}
            </button>
          </div>
        )}
      </div>
    </div>
  );
};

export default AISummaryPanel;
