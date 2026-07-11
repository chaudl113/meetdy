import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  Download,
  Loader2,
  Play,
  Plus,
  RotateCcw,
  Square,
  X,
} from "lucide-react";
import { useShallow } from "zustand/react/shallow";
import { useMeetingStore } from "../../stores/meetingStore";
import { RecordingMetaBar } from "./recording/RecordingMetaBar";
import { RecordingInfoCard } from "./recording/RecordingInfoCard";
import { type RecordingTab, RecordingTabs } from "./recording/RecordingTabs";
import { AISummaryPanel } from "./recording/AISummaryPanel";
import { NotesPanel } from "./recording/NotesPanel";
import { AddNoteModal } from "./recording/AddNoteModal";
import { MeetingTranscriptDisplay } from "./MeetingTranscriptDisplay";
import { MeetingInsightsPanel } from "./MeetingInsightsPanel";
import { commands, type ModelInfo } from "@/bindings";
import { LANGUAGES } from "../../lib/constants/languages";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";

/**
 * CompletedView — post-recording layout shown for processing / completed /
 * failed / interrupted statuses.
 *
 * Mirrors the structure of `RecordingView` (top status bar + meta bar +
 * two-column area) but drops live-only widgets (AudioStatsCard,
 * BottomControlsBar, recording footer) and adds a status pill + retry /
 * dismiss controls when applicable.
 */
export const CompletedView: React.FC = () => {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<RecordingTab>("transcript");
  const [noteModalOpen, setNoteModalOpen] = useState(false);
  const [noteTimestamp, setNoteTimestamp] = useState(0);

  // Regenerate popover state
  const [regenOpen, setRegenOpen] = useState(false);
  const [regenModel, setRegenModel] = useState<string>("");
  const [regenLanguage, setRegenLanguage] = useState<string>("");
  const [regenLoading, setRegenLoading] = useState(false);
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);
  const regenRef = useRef<HTMLDivElement>(null);

  // Export dropdown state
  const [exportOpen, setExportOpen] = useState(false);
  const exportRef = useRef<HTMLDivElement>(null);

  const {
    sessionStatus,
    currentSession,
    isLoading,
    error,
    clearError,
    retryTranscription,
    startMeeting,
    setSessionStatus,
    setCurrentSession,
    addNote,
    recordingDuration,
  } = useMeetingStore(
    useShallow((s) => ({
      sessionStatus: s.sessionStatus,
      currentSession: s.currentSession,
      isLoading: s.isLoading,
      error: s.error,
      clearError: s.clearError,
      retryTranscription: s.retryTranscription,
      startMeeting: s.startMeeting,
      setSessionStatus: s.setSessionStatus,
      setCurrentSession: s.setCurrentSession,
      addNote: s.addNote,
      recordingDuration: s.recordingDuration,
    })),
  );

  // Load available models when regenerate popover is opened
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

  // Close popovers when clicking outside
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

  const openAddNote = () => {
    setNoteTimestamp(recordingDuration);
    setNoteModalOpen(true);
  };

  const handleSaveNote = async (content: string) => {
    await addNote(noteTimestamp, content);
  };

  const handleRetry = async () => {
    clearError();
    await retryTranscription();
  };

  const handleStartNew = async () => {
    clearError();
    await startMeeting();
  };

  const handleNewMeeting = () => {
    clearError();
    setCurrentSession(null);
    setSessionStatus("idle");
  };

  const handleRegenerate = async () => {
    if (!currentSession) return;
    setRegenLoading(true);
    try {
      const result = await commands.retryTranscription(
        currentSession.id,
        regenModel || null,
        regenLanguage || null,
      );
      if (result.status === "ok") {
        setSessionStatus("processing");
        setRegenOpen(false);
      } else {
        console.error("Regenerate failed:", result.error);
      }
    } catch (err) {
      console.error("Regenerate error:", err);
    } finally {
      setRegenLoading(false);
    }
  };

  const buildMarkdown = (transcript: string, summary: string | null) => {
    const session = currentSession;
    if (!session) return transcript;
    const date = new Date(session.created_at * 1000).toLocaleString();
    const parts: string[] = [];
    parts.push(`# ${session.title}`);
    parts.push(`**Date:** ${date}`);
    parts.push("");
    if (summary) {
      parts.push("## Summary");
      parts.push(summary);
      parts.push("");
    }
    parts.push("## Transcript");
    parts.push(transcript);
    return parts.join("\n");
  };

  const handleExport = async (format: "md" | "txt") => {
    if (!currentSession) return;
    setExportOpen(false);

    // Fetch transcript
    const transcriptResult = await commands.getMeetingTranscript(
      currentSession.id,
    );
    const transcript =
      transcriptResult.status === "ok" ? (transcriptResult.data ?? "") : "";

    let content = "";
    if (format === "md") {
      const summaryResult = await commands.getMeetingSummary(currentSession.id);
      const summary =
        summaryResult.status === "ok" ? (summaryResult.data ?? null) : null;
      content = buildMarkdown(transcript, summary);
    } else {
      content = transcript;
    }

    const ext = format === "md" ? "md" : "txt";
    const filePath = await save({
      defaultPath: `${currentSession.title}.${ext}`,
      filters: [
        {
          name: format === "md" ? "Markdown" : "Text",
          extensions: [ext],
        },
      ],
    });
    if (filePath) {
      await writeTextFile(filePath, content);
    }
  };

  const errorMessage = error || currentSession?.error_message || null;

  const renderPanel = () => {
    switch (activeTab) {
      case "transcript":
        // Re-uses the existing transcript fetcher; for "processing" status
        // it will simply show its own loading / empty state.
        return (
          <div className="px-1">
            <MeetingTranscriptDisplay />
          </div>
        );
      case "summary":
        return <AISummaryPanel />;
      case "notes":
        return <NotesPanel onAddNote={openAddNote} />;
    }
  };

  // Header pill styling per status.
  const statusPillClasses: Record<string, string> = {
    processing: "bg-blue-500/10 text-blue-500",
    completed: "bg-green-500/10 text-green-500",
    failed: "bg-red-500/10 text-red-500",
    interrupted: "bg-yellow-500/10 text-yellow-500",
  };
  const pillClass =
    statusPillClasses[sessionStatus] ?? "bg-mid-gray/15 text-text/70";

  const StatusIcon =
    sessionStatus === "processing"
      ? Loader2
      : sessionStatus === "completed"
        ? CheckCircle2
        : AlertCircle;

  return (
    <div className="w-full flex flex-col gap-4">
      {/* Top status / actions bar */}
      <div className="flex items-center justify-between gap-4 px-5 py-3 bg-background border border-mid-gray/20 rounded-xl">
        <div className="flex items-center gap-3">
          <span
            className={`flex items-center gap-2 px-3 py-1.5 rounded-full ${pillClass}`}
          >
            <StatusIcon
              width={14}
              height={14}
              className={sessionStatus === "processing" ? "animate-spin" : ""}
            />
            <span className="text-xs font-semibold uppercase tracking-wide">
              {t(`meeting.status.${sessionStatus}`, sessionStatus)}
            </span>
          </span>
        </div>

        <div className="flex items-center gap-2">
          {sessionStatus === "processing" && (
            <>
              <span className="text-sm text-text/50 flex items-center gap-1.5">
                <Square width={14} height={14} className="animate-pulse" />
                {t("meeting.status.processingIndicator", "Processing...")}
              </span>
              <button
                type="button"
                onClick={() => {
                  commands.cancelOperation().catch(() => {});
                  setSessionStatus("interrupted");
                }}
                className="flex items-center gap-1.5 px-3 py-2 rounded-lg border border-mid-gray/20 text-text/60 hover:bg-mid-gray/10 text-sm font-semibold"
              >
                {t("common.cancel", "Cancel")}
              </button>
            </>
          )}
          {["failed", "completed", "interrupted"].includes(sessionStatus) &&
            currentSession && (
              <>
                {/* Retry (basic, red) for failed */}
                {sessionStatus === "failed" && (
                  <button
                    type="button"
                    onClick={handleRetry}
                    disabled={isLoading}
                    className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-red-500/10 text-red-500 hover:bg-red-500/20 disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    <RotateCcw
                      width={14}
                      height={14}
                      className={isLoading ? "animate-spin" : ""}
                    />
                    <span className="text-sm font-semibold">
                      {t("meeting.error.retry", "Retry")}
                    </span>
                  </button>
                )}
                {/* Regenerate popover */}
                <div className="relative" ref={regenRef}>
                  <button
                    type="button"
                    onClick={() => setRegenOpen((o) => !o)}
                    disabled={regenLoading}
                    className="flex items-center gap-1.5 px-3 py-2 rounded-lg border border-mid-gray/20 text-text/80 hover:bg-mid-gray/10 disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    {regenLoading ? (
                      <Loader2
                        width={14}
                        height={14}
                        className="animate-spin"
                      />
                    ) : (
                      <RotateCcw width={14} height={14} />
                    )}
                    <span className="text-sm font-semibold">
                      {t("meeting.regenerate", "Regenerate")}
                    </span>
                    <ChevronDown width={12} height={12} />
                  </button>
                  {regenOpen && (
                    <div className="absolute right-0 top-full mt-1 z-50 w-72 bg-background border border-mid-gray/20 rounded-xl shadow-xl p-4 flex flex-col gap-3">
                      <p className="text-xs text-text/60">
                        {t(
                          "meeting.regenerateDesc",
                          "Override model and language for this transcription run.",
                        )}
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
                          <option value="">
                            {t("common.default", "Default")}
                          </option>
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
                        {regenLoading && (
                          <Loader2
                            width={14}
                            height={14}
                            className="animate-spin"
                          />
                        )}
                        {t("meeting.regenerate", "Regenerate")}
                      </button>
                    </div>
                  )}
                </div>

                {/* Export dropdown */}
                <div className="relative" ref={exportRef}>
                  <button
                    type="button"
                    onClick={() => setExportOpen((o) => !o)}
                    className="flex items-center gap-1.5 px-3 py-2 rounded-lg border border-mid-gray/20 text-text/80 hover:bg-mid-gray/10"
                  >
                    <Download width={14} height={14} />
                    <span className="text-sm font-semibold">
                      {t("meeting.export", "Export")}
                    </span>
                    <ChevronDown width={12} height={12} />
                  </button>
                  {exportOpen && (
                    <div className="absolute right-0 top-full mt-1 z-50 w-48 bg-background border border-mid-gray/20 rounded-xl shadow-xl overflow-hidden">
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
              </>
            )}
          <button
            type="button"
            onClick={handleNewMeeting}
            disabled={isLoading || sessionStatus === "processing"}
            className="flex items-center gap-1.5 px-3 py-2 rounded-lg border border-mid-gray/20 text-text/80 hover:bg-mid-gray/10 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Plus width={14} height={14} />
            <span className="text-sm font-semibold">
              {t("meeting.actions.newMeeting", "New Meeting")}
            </span>
          </button>
          <button
            type="button"
            onClick={handleStartNew}
            disabled={isLoading || sessionStatus === "processing"}
            className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-logo-primary text-white hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Play width={14} height={14} fill="currentColor" />
            <span className="text-sm font-semibold">
              {t("meeting.controls.start", "Start Recording")}
            </span>
          </button>
        </div>
      </div>

      <RecordingMetaBar />

      {errorMessage && (
        <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-4 flex items-start gap-3">
          <AlertCircle className="h-5 w-5 text-red-500 flex-shrink-0 mt-0.5" />
          <div className="flex-1 min-w-0">
            <p className="text-sm text-red-400 break-words">{errorMessage}</p>
          </div>
          <button
            type="button"
            onClick={clearError}
            className="text-red-400/70 hover:text-red-400"
            aria-label={t("common.dismiss", "Dismiss")}
          >
            <X size={16} />
          </button>
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Left column — tabs */}
        <div className="lg:col-span-2 bg-background border border-mid-gray/20 rounded-xl p-5 flex flex-col gap-4 min-h-[420px]">
          <div className="flex items-center justify-between">
            <RecordingTabs active={activeTab} onChange={setActiveTab} />
          </div>
          <div className="flex-1 overflow-y-auto">{renderPanel()}</div>
        </div>

        {/* Right column — info card + insights when completed */}
        <div className="flex flex-col gap-4">
          <RecordingInfoCard />
          {sessionStatus === "completed" && currentSession?.id && (
            <MeetingInsightsPanel
              sessionId={currentSession.id}
              hasTranscript={!!currentSession.transcript_path}
            />
          )}
        </div>
      </div>

      <AddNoteModal
        open={noteModalOpen}
        timestampSeconds={noteTimestamp}
        onClose={() => setNoteModalOpen(false)}
        onSubmit={handleSaveNote}
      />
    </div>
  );
};

export default CompletedView;
