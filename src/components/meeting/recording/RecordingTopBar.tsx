import React from "react";
import { useTranslation } from "react-i18next";
import { Pause, Square, PenLine, MoreHorizontal } from "lucide-react";
import { formatDuration, useMeetingStore } from "../../../stores/meetingStore";

interface RecordingTopBarProps {
  onAddNote?: () => void;
  onStop?: () => void;
}

/**
 * RecordingTopBar — top action bar of the RecordingView.
 *
 * Shows the live status pill, elapsed timer, and the primary actions
 * (Pause, Stop, Add Note). In Phase 1 the Pause and Add Note buttons
 * are disabled — they become functional in Phase 5 and Phase 2.
 */
export const RecordingTopBar: React.FC<RecordingTopBarProps> = ({
  onAddNote,
  onStop,
}) => {
  const { t } = useTranslation();
  const { sessionStatus, recordingDuration, stopMeeting, isLoading } =
    useMeetingStore();

  const isRecording = sessionStatus === "recording";

  const handleStop = async () => {
    if (onStop) {
      onStop();
      return;
    }
    await stopMeeting();
  };

  return (
    <div className="flex items-center justify-between gap-4 px-5 py-3 bg-background border border-mid-gray/20 rounded-xl">
      <div className="flex items-center gap-3">
        <span className="relative flex items-center gap-2 px-3 py-1.5 rounded-full bg-red-500/10 text-red-500">
          <span className="relative flex h-2 w-2">
            {isRecording && (
              <span className="absolute inline-flex h-full w-full rounded-full bg-red-500 opacity-75 animate-ping" />
            )}
            <span className="relative inline-flex h-2 w-2 rounded-full bg-red-500" />
          </span>
          <span className="text-xs font-semibold uppercase tracking-wide">
            {t("recording.status.recording")}
          </span>
        </span>
        <span className="font-mono text-lg font-semibold tabular-nums">
          {formatDuration(recordingDuration)}
        </span>
      </div>

      <div className="flex items-center gap-2">
        <button
          type="button"
          disabled
          title={t("recording.comingSoon")}
          className="flex items-center gap-1.5 px-3 py-2 rounded-lg border border-mid-gray/20 text-text/50 cursor-not-allowed"
        >
          <Pause width={16} height={16} />
          <span className="text-sm">{t("recording.actions.pause")}</span>
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
        <button
          type="button"
          className="flex items-center justify-center w-9 h-9 rounded-lg border border-mid-gray/20 text-text/70 hover:bg-mid-gray/10"
          aria-label={t("recording.actions.more")}
        >
          <MoreHorizontal width={18} height={18} />
        </button>
      </div>
    </div>
  );
};

export default RecordingTopBar;
