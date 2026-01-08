import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  X,
  Clock,
  Calendar,
  FileText,
  Copy,
  Check,
  RotateCcw,
  AlertCircle,
  Trash2,
  Loader2,
} from "lucide-react";
import { commands, type MeetingSession } from "@/bindings";
import { formatDuration, useMeetingStore } from "../../stores/meetingStore";
import { AudioPlayer } from "../ui/AudioPlayer";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { MeetingSummary } from "./MeetingSummary";

interface MeetingDetailViewProps {
  session: MeetingSession;
  onClose: () => void;
}

/**
 * Formats a Unix timestamp to a localized date/time string
 */
function formatDateTime(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleString();
}

/**
 * MeetingDetailView - Displays detailed information about a meeting session
 */
export const MeetingDetailView: React.FC<MeetingDetailViewProps> = ({
  session,
  onClose,
}) => {
  const { t } = useTranslation();
  const { fetchSessions, retryTranscription } = useMeetingStore();
  const [transcript, setTranscript] = useState<string | null>(null);
  const [summary, setSummary] = useState<string | null>(null);
  const [audioUrl, setAudioUrl] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [loading, setLoading] = useState(true);
  const [isDeleting, setIsDeleting] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [isRetrying, setIsRetrying] = useState(false);
  const [currentSession, setCurrentSession] = useState(session);

  // Ref for focus trap
  const modalRef = useRef<HTMLDivElement>(null);
  const previousActiveElement = useRef<Element | null>(null);

  // Handle Escape key to close modal
  const handleKeyDown = useCallback(
    (event: KeyboardEvent) => {
      if (event.key === "Escape" && !showDeleteConfirm) {
        onClose();
      }
    },
    [onClose, showDeleteConfirm]
  );

  // Focus trap and escape key handler
  useEffect(() => {
    // Save the previously focused element
    previousActiveElement.current = document.activeElement;

    // Focus the modal
    modalRef.current?.focus();

    // Add escape key listener
    document.addEventListener("keydown", handleKeyDown);

    // Prevent body scroll
    document.body.style.overflow = "hidden";

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      document.body.style.overflow = "";

      // Restore focus to previous element
      if (previousActiveElement.current instanceof HTMLElement) {
        previousActiveElement.current.focus();
      }
    };
  }, [handleKeyDown]);

  // Focus trap: keep focus within modal
  useEffect(() => {
    const handleFocusTrap = (event: FocusEvent) => {
      if (
        modalRef.current &&
        event.target instanceof Node &&
        !modalRef.current.contains(event.target)
      ) {
        event.preventDefault();
        modalRef.current.focus();
      }
    };

    document.addEventListener("focusin", handleFocusTrap);
    return () => {
      document.removeEventListener("focusin", handleFocusTrap);
    };
  }, []);

  // Load transcript, summary, and audio URL
  useEffect(() => {
    const loadData = async () => {
      setLoading(true);

      // Load transcript
      if (currentSession.transcript_path) {
        try {
          const result = await commands.getMeetingTranscript(currentSession.id);
          if (result.status === "ok" && result.data) {
            setTranscript(result.data);
          }
        } catch (err) {
          console.error("Failed to load transcript:", err);
        }
      }

      // Load summary
      if (currentSession.summary_path) {
        try {
          const result = await commands.getMeetingSummary(currentSession.id);
          if (result.status === "ok" && result.data) {
            setSummary(result.data);
          }
        } catch (err) {
          console.error("Failed to load summary:", err);
        }
      }

      // Load audio URL
      if (currentSession.audio_path) {
        try {
          // Get the meetings directory and construct the full path
          const result = await commands.getMeetingsDirectory();
          if (result.status === "ok") {
            const fullPath = `${result.data}/${currentSession.audio_path}`;
            setAudioUrl(convertFileSrc(fullPath, "asset"));
          }
        } catch (err) {
          console.error("Failed to load audio:", err);
        }
      }

      setLoading(false);
    };

    loadData();
  }, [currentSession]);

  // Listen for meeting events to update status
  useEffect(() => {
    const setupListeners = async () => {
      const unlistenCompleted = await listen<MeetingSession>("meeting_completed", (event) => {
        if (event.payload.id === currentSession.id) {
          console.log("Meeting completed event received:", event.payload);
          setCurrentSession(event.payload);
          setIsRetrying(false);
        }
      });

      const unlistenFailed = await listen<MeetingSession>("meeting_failed", (event) => {
        if (event.payload.id === currentSession.id) {
          console.log("Meeting failed event received:", event.payload);
          setCurrentSession(event.payload);
          setIsRetrying(false);
        }
      });

      return () => {
        unlistenCompleted();
        unlistenFailed();
      };
    };

    const cleanupPromise = setupListeners();
    return () => {
      cleanupPromise.then((cleanup) => cleanup());
    };
  }, [currentSession.id]);

  const handleCopyTranscript = async () => {
    if (!transcript) return;
    try {
      await navigator.clipboard.writeText(transcript);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  const handleDelete = async () => {
    console.log("Delete button clicked, session:", currentSession.id);
    setShowDeleteConfirm(true);
  };

  const confirmDelete = async () => {
    console.log("Delete confirmed, starting...");
    setShowDeleteConfirm(false);
    setIsDeleting(true);
    try {
      console.log("Calling deleteMeetingSession...");
      const result = await commands.deleteMeetingSession(currentSession.id);
      console.log("Delete result:", result);
      if (result.status === "ok") {
        console.log("Delete successful, refreshing sessions...");
        await fetchSessions();
        onClose();
      } else {
        console.error("Failed to delete:", result.error);
      }
    } catch (err) {
      console.error("Failed to delete:", err);
    } finally {
      setIsDeleting(false);
    }
  };

  const handleRetry = async () => {
    setIsRetrying(true);
    try {
      const result = await commands.retryTranscription(currentSession.id);
      if (result.status === "ok") {
        // Update local session status
        setCurrentSession({ ...currentSession, status: "processing", error_message: null });
        setTranscript(null);
        await fetchSessions();
      } else {
        console.error("Failed to retry:", result.error);
        alert(t("meeting.detail.retryError", "Failed to retry transcription"));
      }
    } catch (err) {
      console.error("Failed to retry:", err);
      alert(t("meeting.detail.retryError", "Failed to retry transcription"));
    } finally {
      setIsRetrying(false);
    }
  };

  const statusColors = {
    idle: "text-gray-400",
    recording: "text-red-400",
    processing: "text-yellow-400",
    completed: "text-green-400",
    failed: "text-red-400",
    interrupted: "text-orange-400",
  };

  const canRetry = currentSession.status === "failed" || currentSession.status === "interrupted" || currentSession.status === "completed";

  return (
    <div
      ref={modalRef}
      tabIndex={-1}
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="meeting-detail-title"
    >
      <div className="bg-background border border-mid-gray/30 rounded-xl max-w-2xl w-full max-h-[80vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-mid-gray/20">
          <h2 id="meeting-detail-title" className="text-lg font-semibold truncate pr-4">{currentSession.title}</h2>
          <div className="flex items-center gap-2">
            {/* Retry button */}
            {canRetry && (
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  handleRetry();
                }}
                disabled={isRetrying}
                className="p-1.5 hover:bg-mid-gray/20 rounded-lg transition-colors text-mid-gray hover:text-white disabled:opacity-50"
                aria-label={t("meeting.detail.retryTranscription", "Re-transcribe")}
              >
                {isRetrying ? (
                  <Loader2 className="h-5 w-5 animate-spin" aria-hidden="true" />
                ) : (
                  <RotateCcw className="h-5 w-5" aria-hidden="true" />
                )}
              </button>
            )}
            {/* Delete button */}
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                e.preventDefault();
                handleDelete();
              }}
              disabled={isDeleting}
              className="p-1.5 hover:bg-red-500/20 rounded-lg transition-colors text-mid-gray hover:text-red-400 disabled:opacity-50"
              aria-label={t("meeting.detail.delete", "Delete")}
            >
              {isDeleting ? (
                <Loader2 className="h-5 w-5 animate-spin" aria-hidden="true" />
              ) : (
                <Trash2 className="h-5 w-5" aria-hidden="true" />
              )}
            </button>
            {/* Close button */}
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onClose();
              }}
              className="p-1.5 hover:bg-mid-gray/20 rounded-lg transition-colors"
              aria-label={t("common.close", "Close")}
            >
              <X className="h-5 w-5" aria-hidden="true" />
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          {/* Metadata */}
          <div className="flex flex-wrap gap-4 text-sm text-mid-gray">
            <div className="flex items-center gap-1.5">
              <Calendar className="h-4 w-4" />
              <span>{formatDateTime(currentSession.created_at)}</span>
            </div>
            {currentSession.duration && (
              <div className="flex items-center gap-1.5">
                <Clock className="h-4 w-4" />
                <span>{formatDuration(currentSession.duration)}</span>
              </div>
            )}
            <div className={`flex items-center gap-1.5 ${statusColors[currentSession.status]}`}>
              <span className="capitalize">{currentSession.status}</span>
            </div>
          </div>

          {/* Error message */}
          {currentSession.error_message && (
            <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3">
              <div className="flex items-start gap-2">
                <AlertCircle className="h-4 w-4 text-red-400 mt-0.5 flex-shrink-0" />
                <p className="text-sm text-red-400">{currentSession.error_message}</p>
              </div>
            </div>
          )}

          {/* Audio Player */}
          {audioUrl && (
            <div className="space-y-2">
              <h3 className="text-sm font-medium text-mid-gray flex items-center gap-2">
                <FileText className="h-4 w-4" />
                {t("meeting.detail.audio", "Audio Recording")}
              </h3>
              <AudioPlayer src={audioUrl} className="w-full" />
            </div>
          )}

          {/* AI Summary */}
          {currentSession.status === "completed" && (
            <MeetingSummary
              sessionId={currentSession.id}
              summary={summary}
              hasSummary={!!currentSession.summary_path}
              hasTranscript={!!transcript}
              onSummaryGenerated={(newSummary) => {
                setSummary(newSummary);
                setCurrentSession({
                  ...currentSession,
                  summary_path: `${currentSession.id}/summary.md`,
                });
              }}
            />
          )}

          {/* Transcript */}
          {loading ? (
            <div className="text-center py-8 text-mid-gray">
              {t("common.loading", "Loading...")}
            </div>
          ) : transcript ? (
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <h3 className="text-sm font-medium text-mid-gray">
                  {t("meeting.detail.transcript", "Transcript")}
                </h3>
                <button
                  onClick={handleCopyTranscript}
                  className="inline-flex items-center gap-1.5 px-2 py-1 text-xs text-mid-gray hover:text-white hover:bg-mid-gray/20 rounded transition-colors"
                >
                  {copied ? (
                    <>
                      <Check className="h-3.5 w-3.5" />
                      {t("common.copied", "Copied")}
                    </>
                  ) : (
                    <>
                      <Copy className="h-3.5 w-3.5" />
                      {t("common.copy", "Copy")}
                    </>
                  )}
                </button>
              </div>
              <div className="bg-dark-gray/30 rounded-lg p-4">
                <p className="text-sm whitespace-pre-wrap">{transcript}</p>
              </div>
            </div>
          ) : currentSession.status === "completed" ? (
            <div className="text-center py-8 text-mid-gray">
              {t("meeting.detail.noTranscript", "No transcript available")}
            </div>
          ) : currentSession.status === "processing" ? (
            <div className="text-center py-8 text-yellow-400">
              <Loader2 className="h-6 w-6 animate-spin mx-auto mb-2" />
              {t("meeting.detail.processing", "Transcription in progress...")}
            </div>
          ) : null}
        </div>
      </div>

      {/* Delete Confirmation Dialog */}
      {showDeleteConfirm && (
        <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-[60]">
          <div className="bg-background border border-mid-gray/30 rounded-xl p-6 max-w-sm w-full mx-4">
            <div className="flex items-center gap-3 mb-4">
              <div className="p-2 bg-red-500/20 rounded-full">
                <Trash2 className="h-5 w-5 text-red-400" />
              </div>
              <h3 className="text-lg font-semibold">
                {t("meeting.detail.deleteTitle", "Delete Meeting")}
              </h3>
            </div>
            <p className="text-mid-gray mb-6">
              {t("meeting.detail.confirmDelete", "Are you sure you want to delete this meeting? This action cannot be undone.")}
            </p>
            <div className="flex gap-3 justify-end">
              <button
                type="button"
                onClick={() => setShowDeleteConfirm(false)}
                className="px-4 py-2 rounded-lg border border-mid-gray/30 hover:bg-mid-gray/20 transition-colors"
              >
                {t("common.cancel", "Cancel")}
              </button>
              <button
                type="button"
                onClick={confirmDelete}
                className="px-4 py-2 rounded-lg bg-red-500 hover:bg-red-600 text-white transition-colors"
              >
                {t("common.delete", "Delete")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default MeetingDetailView;
