import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Virtuoso } from "react-virtuoso";
import { useTranslation } from "react-i18next";
import {
  AlertCircle,
  Calendar,
  Check,
  ChevronDown,
  Clock,
  Copy,
  Download,
  FileText,
  Languages,
  Loader2,
  RotateCcw,
  Search,
  Trash2,
  X,
} from "lucide-react";
import { TTSButton } from "./TTSButton";
import { useShallow } from "zustand/react/shallow";
import { commands, type MeetingSession, type ModelInfo, type Participant, type TranscriptSegment } from "@/bindings";
import { formatDuration, useMeetingStore } from "../../stores/meetingStore";
import { AudioPlayer, type AudioPlayerHandle } from "../ui/AudioPlayer";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { MeetingSummary } from "./MeetingSummary";
import { MeetingInsightsPanel } from "./MeetingInsightsPanel";
import { useSettings } from "../../hooks/useSettings";
import { isAiConfigured } from "../../lib/utils/aiConfig";
import { SpeakerSegment } from "./recording/SpeakerSegment";
import {
  UNKNOWN_SPEAKER_COLOR,
  useSpeakerColors,
} from "../../hooks/useSpeakerColors";
import { LANGUAGES } from "../../lib/constants/languages";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";


interface MeetingDetailViewProps {
  session: MeetingSession;
  onClose: () => void;
}

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
  const { fetchSessions } = useMeetingStore(
    useShallow((s) => ({
      fetchSessions: s.fetchSessions,
    })),
  );
  const { settings } = useSettings();
  const aiConfigured = isAiConfigured(settings);
  const [transcript, setTranscript] = useState<string | null>(null);
  const [summary, setSummary] = useState<string | null>(null);
  const [audioUrl, setAudioUrl] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [loading, setLoading] = useState(true);
  const [isDeleting, setIsDeleting] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [isRetrying, setIsRetrying] = useState(false);
  const [currentSession, setCurrentSession] = useState(session);
  const [audioTime, setAudioTime] = useState(0);
  const [transcriptSearch, setTranscriptSearch] = useState("");

  // Regenerate popover
  const [regenOpen, setRegenOpen] = useState(false);
  const [regenModel, setRegenModel] = useState<string>("");
  const [regenLanguage, setRegenLanguage] = useState<string>("");
  const [regenLoading, setRegenLoading] = useState(false);
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);
  const regenRef = useRef<HTMLDivElement>(null);

  // Export dropdown
  const [exportOpen, setExportOpen] = useState(false);
  const exportRef = useRef<HTMLDivElement>(null);

  const [transcriptSegments, setTranscriptSegments] = useState<TranscriptSegment[]>([]);
  const [transcriptParticipants, setTranscriptParticipants] = useState<Participant[]>([]);
  const audioPlayerRef = useRef<AudioPlayerHandle>(null);
  const segmentRefs = useRef<Record<string, HTMLDivElement | null>>({});

  const segSpeakerColors = useSpeakerColors(transcriptParticipants);
  const segParticipantMap = Object.fromEntries(
    transcriptParticipants.map((p) => [p.id, p.name]),
  );
  const transcriptQuery = transcriptSearch.trim().toLowerCase();
  const hasTimedSegments = transcriptSegments.some(
    (segment) => segment.start_ms > 0 || segment.end_ms > 0,
  );
  const canSeekSegment = (segment: TranscriptSegment) =>
    !!audioUrl && segment.end_ms > segment.start_ms;
  const visibleTranscriptSegments = useMemo(() => {
    if (!transcriptQuery) return transcriptSegments;
    return transcriptSegments.filter((segment) =>
      segment.text.toLowerCase().includes(transcriptQuery),
    );
  }, [transcriptQuery, transcriptSegments]);
  const plainTranscriptMatches =
    !transcriptQuery || (transcript ?? "").toLowerCase().includes(transcriptQuery);
  const activeSegmentId = useMemo(() => {
    if (!hasTimedSegments || transcriptSegments.length === 0) return null;
    const currentMs = audioTime * 1000;
    for (let index = 0; index < transcriptSegments.length; index += 1) {
      const segment = transcriptSegments[index];
      if (!canSeekSegment(segment)) continue;
      const next = transcriptSegments[index + 1];
      const startMs = Math.max(0, segment.start_ms);
      const endMs =
        segment.end_ms > startMs
          ? segment.end_ms
          : next?.start_ms && next.start_ms > startMs
            ? next.start_ms
            : startMs + 8000;
      if (currentMs >= startMs && currentMs < endMs) return segment.id;
    }
    return null;
  }, [audioTime, audioUrl, hasTimedSegments, transcriptSegments]);

  // Translation state
  const [translateTarget, setTranslateTarget] = useState<string>("");
  const [translatedText, setTranslatedText] = useState<string | null>(null);
  const [isTranslating, setIsTranslating] = useState(false);
  const [translateError, setTranslateError] = useState<string | null>(null);
  const [translatedCopied, setTranslatedCopied] = useState(false);


  // Load available models when regen popover opens
  useEffect(() => {
    if (regenOpen && availableModels.length === 0) {
      commands.getAvailableModels().then((r) => {
        if (r.status === "ok") {
          setAvailableModels(
            r.data.filter(
              (m) => m.is_downloaded && m.engine_type !== "Diarization",
            ),
          );
        }
      });
    }
  }, [regenOpen, availableModels.length]);

  // Close popovers on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (regenRef.current && !regenRef.current.contains(e.target as Node)) {
        setRegenOpen(false);
      }
      if (exportRef.current && !exportRef.current.contains(e.target as Node)) {
        setExportOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

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
    [onClose, showDeleteConfirm],
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

      // Load transcript segments for speaker-colored display
      try {
        const segResult = await commands.getMeetingTranscriptSegments(
          currentSession.id,
        );
        if (segResult.status === "ok" && segResult.data.length > 0) {
          setTranscriptSegments(segResult.data);
          const pResult = await commands.listMeetingParticipants(
            currentSession.id,
          );
          if (pResult.status === "ok") setTranscriptParticipants(pResult.data);
        }
      } catch {
        // Segments not available, fall back to plain transcript
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
      const unlistenCompleted = await listen<MeetingSession>(
        "meeting_completed",
        (event) => {
          if (event.payload.id === currentSession.id) {
            console.log("Meeting completed event received:", event.payload);
            setCurrentSession(event.payload);
            setIsRetrying(false);
          }
        },
      );

      const unlistenFailed = await listen<MeetingSession>(
        "meeting_failed",
        (event) => {
          if (event.payload.id === currentSession.id) {
            console.log("Meeting failed event received:", event.payload);
            setCurrentSession(event.payload);
            setIsRetrying(false);
          }
        },
      );

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

  useEffect(() => {
    if (!activeSegmentId || transcriptQuery) return;
    segmentRefs.current[activeSegmentId]?.scrollIntoView({
      behavior: "smooth",
      block: "nearest",
    });
  }, [activeSegmentId, transcriptQuery]);

  const handleSegmentSeek = (segment: TranscriptSegment) => {
    if (!canSeekSegment(segment)) return;
    audioPlayerRef.current?.seekTo(Math.max(0, segment.start_ms / 1000), true);
  };

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

  // Re-translate when transcript changes if a target language was already selected
  useEffect(() => {
    if (translateTarget && transcript) {
      handleTranslate(translateTarget);
    } else {
      setTranslatedText(null);
      setTranslateError(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [transcript]);

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
      const result = await commands.retryTranscription(currentSession.id, null, null);
      if (result.status === "ok") {
        // Update local session status
        setCurrentSession({
          ...currentSession,
          status: "processing",
          error_message: null,
        });
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

  const handleRegenerate = async () => {
    setRegenLoading(true);
    try {
      const result = await commands.retryTranscription(
        currentSession.id,
        regenModel || null,
        regenLanguage || null,
      );
      if (result.status === "ok") {
        setCurrentSession({
          ...currentSession,
          status: "processing",
          error_message: null,
        });
        setTranscript(null);
        setRegenOpen(false);
        await fetchSessions();
      } else {
        console.error("Regenerate failed:", result.error);
      }
    } catch (err) {
      console.error("Regenerate error:", err);
    } finally {
      setRegenLoading(false);
    }
  };

  const buildMarkdown = (tr: string, sum: string | null) => {
    const date = new Date(currentSession.created_at * 1000).toLocaleString();
    const parts: string[] = [];
    parts.push(`# ${currentSession.title}`);
    parts.push(`**Date:** ${date}`);
    parts.push("");
    if (sum) {
      parts.push("## Summary");
      parts.push(sum);
      parts.push("");
    }
    parts.push("## Transcript");
    parts.push(tr);
    return parts.join("\n");
  };

  const handleExport = async (format: "md" | "txt") => {
    setExportOpen(false);
    const tr = transcript ?? "";
    let content = "";
    if (format === "md") {
      const summaryResult = await commands.getMeetingSummary(currentSession.id);
      const sum = summaryResult.status === "ok" ? (summaryResult.data ?? null) : null;
      content = buildMarkdown(tr, sum);
    } else {
      content = tr;
    }
    const ext = format === "md" ? "md" : "txt";
    const filePath = await save({
      defaultPath: `${currentSession.title}.${ext}`,
      filters: [{ name: format === "md" ? "Markdown" : "Text", extensions: [ext] }],
    });
    if (filePath) {
      await writeTextFile(filePath, content);
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

  const canRetry =
    currentSession.status === "failed" ||
    currentSession.status === "interrupted" ||
    currentSession.status === "completed";

  return (
    <div
      ref={modalRef}
      tabIndex={-1}
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="meeting-detail-title"
    >
      <div className="bg-background border border-mid-gray/30 rounded-xl max-w-3xl w-full max-h-[85vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-mid-gray/20">
          <h2
            id="meeting-detail-title"
            className="text-lg font-semibold truncate pr-4"
          >
            {currentSession.title}
          </h2>
          <div className="flex items-center gap-2">
            {/* Regenerate popover */}
            {canRetry && (
              <div className="relative" ref={regenRef}>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    setRegenOpen((o) => !o);
                  }}
                  disabled={regenLoading}
                  className="flex items-center gap-1 p-1.5 hover:bg-mid-gray/20 rounded-lg transition-colors text-mid-gray hover:text-white disabled:opacity-50"
                  aria-label={t("meeting.regenerate", "Regenerate")}
                >
                  {regenLoading ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <RotateCcw className="h-4 w-4" />
                  )}
                  <ChevronDown className="h-3 w-3" />
                </button>
                {regenOpen && (
                  <div className="absolute right-0 top-full mt-1 z-[60] w-72 bg-background border border-mid-gray/20 rounded-xl shadow-xl p-4 flex flex-col gap-3">
                    <p className="text-xs text-text/60">
                      {t("meeting.regenerateDesc", "Override model and language for this run.")}
                    </p>
                    <div>
                      <label className="block text-xs font-medium mb-1">
                        {t("meeting.regenerateModel", "Model")}
                      </label>
                      <select
                        value={regenModel}
                        onChange={(e) => setRegenModel(e.target.value)}
                        className="w-full bg-background border border-mid-gray/30 rounded-lg px-2 py-1.5 text-sm focus:outline-none focus:border-logo-primary"
                      >
                        <option value="">{t("common.default", "Default")}</option>
                        {availableModels.map((m) => (
                          <option key={m.id} value={m.id}>
                            {m.name}
                          </option>
                        ))}
                      </select>
                    </div>
                    <div>
                      <label className="block text-xs font-medium mb-1">
                        {t("meeting.regenerateLanguage", "Language")}
                      </label>
                      <select
                        value={regenLanguage}
                        onChange={(e) => setRegenLanguage(e.target.value)}
                        className="w-full bg-background border border-mid-gray/30 rounded-lg px-2 py-1.5 text-sm focus:outline-none focus:border-logo-primary"
                      >
                        {LANGUAGES.map((l) => (
                          <option key={l.value} value={l.value}>
                            {l.label}
                          </option>
                        ))}
                      </select>
                    </div>
                    <button
                      type="button"
                      onClick={handleRegenerate}
                      disabled={regenLoading}
                      className="w-full flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg bg-logo-primary text-white hover:opacity-90 disabled:opacity-50 text-sm font-semibold"
                    >
                      {regenLoading && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
                      {t("meeting.regenerate", "Regenerate")}
                    </button>
                  </div>
                )}
              </div>
            )}
            {/* Export dropdown */}
            <div className="relative" ref={exportRef}>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  setExportOpen((o) => !o);
                }}
                className="p-1.5 hover:bg-mid-gray/20 rounded-lg transition-colors text-mid-gray hover:text-white"
                aria-label={t("meeting.export", "Export")}
              >
                <Download className="h-4 w-4" />
              </button>
              {exportOpen && (
                <div className="absolute right-0 top-full mt-1 z-[60] w-48 bg-background border border-mid-gray/20 rounded-xl shadow-xl overflow-hidden">
                  <button
                    type="button"
                    onClick={() => handleExport("md")}
                    className="w-full px-4 py-2.5 text-sm text-left hover:bg-mid-gray/10"
                  >
                    {t("meeting.exportMarkdown", "Export as Markdown")}
                  </button>
                  <button
                    type="button"
                    onClick={() => handleExport("txt")}
                    className="w-full px-4 py-2.5 text-sm text-left hover:bg-mid-gray/10"
                  >
                    {t("meeting.exportText", "Export as Text")}
                  </button>
                </div>
              )}
            </div>
            {/* Retry button (for failed sessions) */}
            {canRetry && currentSession.status === "failed" && (
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
            <div
              className={`flex items-center gap-1.5 ${statusColors[currentSession.status]}`}
            >
              <span className="capitalize">{currentSession.status}</span>
            </div>
          </div>

          {/* Error message */}
          {currentSession.error_message && (
            <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3">
              <div className="flex items-start gap-2">
                <AlertCircle className="h-4 w-4 text-red-400 mt-0.5 flex-shrink-0" />
                <p className="text-sm text-red-400">
                  {currentSession.error_message}
                </p>
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
              <AudioPlayer
                ref={audioPlayerRef}
                src={audioUrl}
                className="w-full"
                onTimeChange={setAudioTime}
              />
            </div>
          )}

          {/* AI Summary - only shown when AI post-processing is configured */}
          {currentSession.status === "completed" && aiConfigured && (
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

          {/* AI Insights - structured key points, action items, participants, tags */}
          {currentSession.status === "completed" && (
            <MeetingInsightsPanel
              sessionId={currentSession.id}
              hasTranscript={!!transcript}
            />
          )}

          {/* Transcript */}
          {loading ? (
            <div className="text-center py-8 text-mid-gray">
              {t("common.loading", "Loading...")}
            </div>
          ) : transcript ? (
            <div className="space-y-2">
              <div className="flex items-center justify-between gap-2 flex-wrap">
                <h3 className="text-sm font-medium text-mid-gray">
                  {t("meeting.detail.transcript", "Transcript")}
                </h3>
                <div className="flex items-center gap-2">
                  <div className="relative">
                    <Search className="absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-mid-gray" />
                    <input
                      type="search"
                      value={transcriptSearch}
                      onChange={(event) => setTranscriptSearch(event.target.value)}
                      placeholder={t("meeting.detail.searchTranscript", "Search transcript")}
                      className="h-7 w-44 rounded border border-mid-gray/30 bg-dark-gray/50 pl-7 pr-2 text-xs text-white placeholder:text-mid-gray focus:border-logo-primary focus:outline-none"
                    />
                  </div>
                  <div className="flex items-center gap-1.5 text-xs text-mid-gray">
                    <Languages className="h-3.5 w-3.5" />
                    <select
                      value={translateTarget}
                      onChange={(e) => {
                        const val = e.target.value;
                        setTranslateTarget(val);
                        handleTranslate(val);
                      }}
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
                  <TTSButton
                    getText={() =>
                      transcriptSegments.length > 0
                        ? transcriptSegments.map((s) => s.text).join(" ")
                        : transcript ?? ""
                    }
                  />
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
              </div>
              {translateTarget ? (
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
                        {t(
                          "meeting.detail.translate.loading",
                          "Translating...",
                        )}
                      </div>
                    ) : translateError ? (
                      <p className="text-sm text-red-400">{translateError}</p>
                    ) : translatedText ? (
                      <p className="text-sm whitespace-pre-wrap">
                        {translatedText}
                      </p>
                    ) : (
                      <p className="text-sm text-mid-gray italic">
                        {t(
                          "meeting.detail.translate.empty",
                          "No translation yet",
                        )}
                      </p>
                    )}
                  </div>
                </div>
              ) : (
                <div className="bg-dark-gray/30 rounded-lg p-4">
                  {transcriptSegments.length > 0 ? (
                    <div className="flex flex-col">
                      {visibleTranscriptSegments.length === 0 ? (
                        <p className="py-6 text-center text-sm text-mid-gray">
                          {t(
                            "meeting.detail.noTranscriptMatches",
                            "No transcript matches",
                          )}
                        </p>
                      ) : (
                        <Virtuoso
                          data={visibleTranscriptSegments}
                          style={{ height: "400px" }}
                          itemContent={(index, seg) => {
                            const sourceIndex = transcriptSegments.findIndex(
                              (segment) => segment.id === seg.id,
                            );
                            const prevSpeakerId =
                              sourceIndex > 0
                                ? transcriptSegments[sourceIndex - 1].speaker_id
                                : undefined;
                            const showLabel = seg.speaker_id !== prevSpeakerId;
                            const color = seg.speaker_id
                              ? (segSpeakerColors[seg.speaker_id] ??
                                UNKNOWN_SPEAKER_COLOR)
                              : UNKNOWN_SPEAKER_COLOR;
                            return (
                              <div
                                key={seg.id}
                                ref={(node) => {
                                  segmentRefs.current[seg.id] = node;
                                }}
                              >
                                <SpeakerSegment
                                  text={seg.text}
                                  startMs={seg.start_ms}
                                  speakerName={
                                    seg.speaker_id
                                      ? segParticipantMap[seg.speaker_id]
                                      : null
                                  }
                                  color={color}
                                  showSpeakerLabel={showLabel}
                                  active={activeSegmentId === seg.id}
                                  disabled={!canSeekSegment(seg)}
                                  onClick={() => handleSegmentSeek(seg)}
                                />
                              </div>
                            );
                          }}
                        />
                      )}
                    </div>
                  ) : plainTranscriptMatches ? (
                    <p className="text-sm text-text/70 whitespace-pre-wrap leading-relaxed">
                      {transcript}
                    </p>
                  ) : (
                    <p className="py-6 text-center text-sm text-mid-gray">
                      {t(
                        "meeting.detail.noTranscriptMatches",
                        "No transcript matches",
                      )}
                    </p>
                  )}
                </div>
              )}
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
              {t(
                "meeting.detail.confirmDelete",
                "Are you sure you want to delete this meeting? This action cannot be undone.",
              )}
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
