import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Copy, Check, FileText } from "lucide-react";
import { useMeetingStore } from "../../stores/meetingStore";
import { commands } from "@/bindings";

/**
 * MeetingTranscriptDisplay - Displays the transcript text for completed meetings.
 *
 * Shows:
 * - Transcript text content when meeting is completed
 * - Copy to clipboard button
 * - Loading state while fetching transcript
 */
export const MeetingTranscriptDisplay: React.FC = () => {
  const { t } = useTranslation();
  const { currentSession, sessionStatus } = useMeetingStore();
  const [transcript, setTranscript] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isCopied, setIsCopied] = useState(false);

  // Fetch transcript when session is completed
  useEffect(() => {
    const fetchTranscript = async () => {
      console.log("[MeetingTranscriptDisplay] Checking fetch conditions:", {
        sessionId: currentSession?.id,
        sessionStatus,
        transcriptPath: currentSession?.transcript_path,
      });

      if (!currentSession?.id || sessionStatus !== "completed") {
        setTranscript(null);
        return;
      }

      setIsLoading(true);
      try {
        console.log("[MeetingTranscriptDisplay] Fetching transcript for session:", currentSession.id);
        const result = await commands.getMeetingTranscript(currentSession.id);
        console.log("[MeetingTranscriptDisplay] Fetch result:", result);
        if (result.status === "ok") {
          setTranscript(result.data);
        } else {
          console.error("[MeetingTranscriptDisplay] Fetch error:", result.error);
        }
      } catch (err) {
        console.error("Failed to fetch transcript:", err);
      } finally {
        setIsLoading(false);
      }
    };

    fetchTranscript();
  }, [currentSession?.id, currentSession?.transcript_path, sessionStatus]);

  // Handle copy to clipboard
  const handleCopy = async () => {
    if (!transcript) return;

    try {
      await navigator.clipboard.writeText(transcript);
      setIsCopied(true);
      setTimeout(() => setIsCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  // Don't render if not completed
  if (sessionStatus !== "completed") {
    return null;
  }

  // Show loading or transcript content
  return (
    <div className="mt-4 rounded-lg border border-gray-700 bg-gray-800/50">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-700">
        <div className="flex items-center gap-2 text-sm font-medium text-gray-300">
          <FileText size={16} />
          {t("meeting.transcript", "Transcript")}
        </div>
        {transcript && (
          <button
            onClick={handleCopy}
            className="flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium text-gray-400 hover:text-white hover:bg-gray-700 rounded transition-colors"
            title={t("meeting.copyTranscript", "Copy transcript")}
          >
            {isCopied ? (
              <>
                <Check size={14} className="text-green-400" />
                {t("common.copied", "Copied!")}
              </>
            ) : (
              <>
                <Copy size={14} />
                {t("common.copy", "Copy")}
              </>
            )}
          </button>
        )}
      </div>

      {/* Content */}
      <div className="p-4">
        {isLoading ? (
          <div className="flex items-center gap-2 text-sm text-gray-400">
            <span className="inline-flex h-4 w-4 rounded-full border-2 border-gray-400 border-t-transparent animate-spin"></span>
            {t("common.loading", "Loading...")}
          </div>
        ) : transcript ? (
          <p className="text-sm text-gray-200 whitespace-pre-wrap leading-relaxed">
            {transcript}
          </p>
        ) : (
          <p className="text-sm text-gray-400 italic">
            {t("meeting.noTranscript", "No transcript available")}
          </p>
        )}
      </div>
    </div>
  );
};

export default MeetingTranscriptDisplay;
