import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  CheckCircle2,
  AlertCircle,
  Loader2,
  Play,
  Plus,
  RotateCcw,
  X,
} from "lucide-react";
import { useShallow } from "zustand/react/shallow";
import { useMeetingStore } from "../../stores/meetingStore";
import { RecordingMetaBar } from "./recording/RecordingMetaBar";
import { RecordingInfoCard } from "./recording/RecordingInfoCard";
import {
  RecordingTabs,
  type RecordingTab,
} from "./recording/RecordingTabs";
import { AISummaryPanel } from "./recording/AISummaryPanel";
import { NotesPanel } from "./recording/NotesPanel";
import { AddNoteModal } from "./recording/AddNoteModal";
import { MeetingTranscriptDisplay } from "./MeetingTranscriptDisplay";

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
    <div className="w-full max-w-[1200px] flex flex-col gap-4">
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
          {["failed", "completed", "interrupted"].includes(sessionStatus) && currentSession && (
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
                {sessionStatus === "failed"
                  ? t("meeting.error.retry", "Retry")
                  : t("meeting.actions.regenerateTranscript", "Regenerate Transcript")}
              </span>
            </button>
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

        {/* Right column — info card only (no live audio stats / quick notes
            when not recording). */}
        <div className="flex flex-col gap-4">
          <RecordingInfoCard />
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
