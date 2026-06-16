import React from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { Pause, Play, Square, PenLine, AlertTriangle, X } from "lucide-react";
import { formatDuration, useMeetingStore } from "../../../stores/meetingStore";

interface RecordingTopBarProps {
  onAddNote?: () => void;
  onStop?: () => void;
}

/**
 * RecordingTopBar — top action bar of the RecordingView.
 *
  * Shows the live status pill, elapsed timer, and the primary recording
  * actions (Stop, Add Note).
 */
export const RecordingTopBar: React.FC<RecordingTopBarProps> = ({
  onAddNote,
  onStop,
}) => {
  const { t } = useTranslation();
  const {
    sessionStatus,
    recordingDuration,
    isPaused,
    stopMeeting,
    pauseMeeting,
    resumeMeeting,
    isLoading,
    sttError,
    clearSttError,
  } = useMeetingStore(
    useShallow((s) => ({
      sessionStatus: s.sessionStatus,
      recordingDuration: s.recordingDuration,
      isPaused: s.isPaused,
      stopMeeting: s.stopMeeting,
      pauseMeeting: s.pauseMeeting,
      resumeMeeting: s.resumeMeeting,
      isLoading: s.isLoading,
      sttError: s.sttError,
      clearSttError: s.clearSttError,
    })),
  );

  const isRecording = sessionStatus === "recording";

  const handleStop = async () => {
    if (onStop) {
      onStop();
      return;
    }
    await stopMeeting();
  };

  const handlePauseToggle = async () => {
    if (isPaused) {
      await resumeMeeting();
    } else {
      await pauseMeeting();
    }
  };

  return (
    <div className="flex flex-col gap-2">
    {sttError && (
      <div className="flex items-center gap-2 px-4 py-2 bg-red-500/10 border border-red-500/30 rounded-xl text-red-400 text-sm">
        <AlertTriangle width={14} height={14} className="shrink-0" />
        <span className="flex-1">{sttError}</span>
        <button
          type="button"
          onClick={clearSttError}
          className="shrink-0 hover:text-red-300"
        >
          <X width={14} height={14} />
        </button>
      </div>
    )}
    <div className="flex items-center justify-between gap-4 px-5 py-3 bg-background border border-mid-gray/20 rounded-xl">
      <div className="flex items-center gap-3">
        <span className="relative flex items-center gap-2 px-3 py-1.5 rounded-full bg-red-500/10 text-red-500">
          <span className="relative flex h-2 w-2">
            {isRecording && !isPaused && (
              <span className="absolute inline-flex h-full w-full rounded-full bg-red-500 opacity-75 animate-ping" />
            )}
            <span className="relative inline-flex h-2 w-2 rounded-full bg-red-500" />
          </span>
          <span className="text-xs font-semibold uppercase tracking-wide">
            {isPaused ? t("recording.status.paused", "Paused") : t("recording.status.recording")}
          </span>
        </span>
        <span className="font-mono text-lg font-semibold tabular-nums">
          {formatDuration(recordingDuration)}
        </span>
      </div>

      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={handlePauseToggle}
          disabled={isLoading || !isRecording}
          className="flex items-center gap-1.5 px-3 py-2 rounded-lg border border-mid-gray/20 text-text/80 hover:bg-mid-gray/10 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isPaused ? <Play width={16} height={16} /> : <Pause width={16} height={16} />}
          <span className="text-sm">
            {isPaused ? t("recording.actions.resume", "Resume") : t("recording.actions.pause")}
          </span>
        </button>
        <button
          type="button"
          onClick={handleStop}
          disabled={isLoading || !isRecording}
          className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-red-500 text-white hover:bg-red-600 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <Square width={14} height={14} fill="currentColor" />
          <span className="text-sm font-semibold">
            {t("recording.actions.stop")}
          </span>
        </button>
        <button
          type="button"
          onClick={onAddNote}
          disabled={!onAddNote}
          title={onAddNote ? undefined : t("recording.comingSoon")}
          className="flex items-center gap-1.5 px-3 py-2 rounded-lg border border-mid-gray/20 text-text/80 hover:bg-mid-gray/10 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <PenLine width={16} height={16} />
          <span className="text-sm">{t("recording.actions.addNote")}</span>
        </button>
      </div>
    </div>
    </div>
  );
};

export default RecordingTopBar;
