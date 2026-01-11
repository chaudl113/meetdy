import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Sparkles,
  Copy,
  Check,
  Loader2,
  ChevronDown,
  ChevronUp,
  RefreshCw,
} from "lucide-react";
import { commands } from "@/bindings";

interface MeetingSummaryProps {
  sessionId: string;
  summary: string | null;
  hasSummary: boolean;
  hasTranscript: boolean;
  onSummaryGenerated: (summary: string) => void;
}

/**
 * MeetingSummary - Component for displaying and generating AI meeting summaries
 */
export const MeetingSummary: React.FC<MeetingSummaryProps> = ({
  sessionId,
  summary,
  hasSummary,
  hasTranscript,
  onSummaryGenerated,
}) => {
  const { t } = useTranslation();
  const [isGenerating, setIsGenerating] = useState(false);
  const [isExpanded, setIsExpanded] = useState(true);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleGenerateSummary = async () => {
    if (!hasTranscript) return;

    setIsGenerating(true);
    setError(null);

    try {
      const result = await commands.generateMeetingSummary(sessionId);
      if (result.status === "ok") {
        onSummaryGenerated(result.data);
      } else {
        setError(result.error);
      }
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : t("meeting.summary.generateError", "Failed to generate summary"),
      );
    } finally {
      setIsGenerating(false);
    }
  };

  const handleCopySummary = async () => {
    if (!summary) return;
    try {
      await navigator.clipboard.writeText(summary);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  return (
    <div className="space-y-2">
      {/* Header */}
      <div className="flex items-center justify-between">
        <button
          type="button"
          onClick={() => setIsExpanded(!isExpanded)}
          aria-expanded={isExpanded}
          aria-controls="meeting-summary-content"
          className="flex items-center gap-2 text-sm font-medium text-mid-gray hover:text-white transition-colors"
        >
          <Sparkles className="h-4 w-4 text-purple-400" aria-hidden="true" />
          {t("meeting.summary.title", "AI Summary")}
          {isExpanded ? (
            <ChevronUp className="h-4 w-4" aria-hidden="true" />
          ) : (
            <ChevronDown className="h-4 w-4" aria-hidden="true" />
          )}
        </button>

        <div className="flex items-center gap-2">
          {/* Regenerate button (shown when summary exists) */}
          {hasSummary && !isGenerating && (
            <button
              type="button"
              onClick={handleGenerateSummary}
              disabled={!hasTranscript || isGenerating}
              className="inline-flex items-center gap-1.5 px-2 py-1 text-xs text-mid-gray hover:text-white hover:bg-mid-gray/20 rounded transition-colors disabled:opacity-50"
              aria-label={t("meeting.summary.regenerate", "Regenerate")}
            >
              <RefreshCw className="h-3.5 w-3.5" aria-hidden="true" />
            </button>
          )}

          {/* Copy button (shown when summary exists) */}
          {hasSummary && (
            <button
              type="button"
              onClick={handleCopySummary}
              className="inline-flex items-center gap-1.5 px-2 py-1 text-xs text-mid-gray hover:text-white hover:bg-mid-gray/20 rounded transition-colors"
              aria-label={
                copied ? t("common.copied", "Copied") : t("common.copy", "Copy")
              }
            >
              {copied ? (
                <>
                  <Check className="h-3.5 w-3.5" aria-hidden="true" />
                  {t("common.copied", "Copied")}
                </>
              ) : (
                <>
                  <Copy className="h-3.5 w-3.5" aria-hidden="true" />
                  {t("common.copy", "Copy")}
                </>
              )}
            </button>
          )}
        </div>
      </div>

      {/* Content */}
      {isExpanded && (
        <div
          id="meeting-summary-content"
          className="bg-dark-gray/30 rounded-lg p-4"
        >
          {/* Error message */}
          {error && (
            <div className="mb-3 p-2 bg-red-500/10 border border-red-500/30 rounded text-sm text-red-400">
              {error}
            </div>
          )}

          {/* Generating state */}
          {isGenerating ? (
            <div className="flex items-center justify-center py-8 text-mid-gray">
              <Loader2 className="h-5 w-5 animate-spin mr-2" />
              {t("meeting.summary.generating", "Generating summary...")}
            </div>
          ) : summary ? (
            /* Summary content - render as preformatted text */
            <div className="text-sm whitespace-pre-wrap">{summary}</div>
          ) : hasTranscript ? (
            /* Generate button */
            <div className="flex flex-col items-center justify-center py-6 text-center">
              <Sparkles className="h-8 w-8 text-purple-400/50 mb-3" />
              <p className="text-sm text-mid-gray mb-4">
                {t(
                  "meeting.summary.noSummary",
                  "No summary yet. Generate an AI summary of this meeting.",
                )}
              </p>
              <button
                type="button"
                onClick={handleGenerateSummary}
                className="inline-flex items-center gap-2 px-4 py-2 bg-purple-600 hover:bg-purple-700 text-white rounded-lg transition-colors"
              >
                <Sparkles className="h-4 w-4" />
                {t("meeting.summary.generate", "Generate Summary")}
              </button>
            </div>
          ) : (
            /* No transcript available */
            <div className="flex flex-col items-center justify-center py-6 text-center">
              <p className="text-sm text-mid-gray">
                {t(
                  "meeting.summary.noTranscript",
                  "Transcript required to generate summary.",
                )}
              </p>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default MeetingSummary;
