import React from "react";
import { useTranslation } from "react-i18next";
import { Play, Square, RotateCcw } from "lucide-react";
import { useMeetingStore, formatDuration } from "../../stores/meetingStore";
import { Button } from "../ui/Button";

/**
 * MeetingControls - Controls component for meeting recording.
 *
 * Provides:
 * - Start/Stop recording button that changes based on session status
 * - Recording timer display during active recording
 * - Retry button for failed transcriptions
 * - Visual feedback for button states (loading, disabled)
 */
export const MeetingControls: React.FC = () => {
  const { t } = useTranslation();

  // Connect to meetingStore for state and actions
  const {
    sessionStatus,
    recordingDuration,
    isLoading,
    startMeeting,
    stopMeeting,
    retryTranscription,
  } = useMeetingStore();

  // Determine button states based on session status
  const isIdle = sessionStatus === "idle";
  const isRecording = sessionStatus === "recording";
  const isProcessing = sessionStatus === "processing";
  const isCompleted = sessionStatus === "completed";
  const isFailed = sessionStatus === "failed";

  // Handle start recording
  const handleStart = async () => {
    await startMeeting();
  };

  // Handle stop recording
  const handleStop = async () => {
    await stopMeeting();
  };

  // Handle retry transcription
  const handleRetry = async () => {
    await retryTranscription();
  };

  return (
    <div className="flex flex-col gap-4">
      {/* Recording Timer Display */}
      {isRecording && (
        <div className="flex items-center justify-center gap-3 py-4">
          {/* Pulsing recording indicator */}
          <span className="flex h-3 w-3">
            <span className="animate-ping absolute inline-flex h-3 w-3 rounded-full bg-red-400 opacity-75"></span>
            <span className="relative inline-flex rounded-full h-3 w-3 bg-red-500"></span>
          </span>
          {/* Timer display */}
          <span className="text-2xl font-mono font-semibold text-primary">
            {formatDuration(recordingDuration)}
          </span>
        </div>
      )}

      {/* Processing indicator */}
      {isProcessing && (
        <div className="flex items-center justify-center gap-3 py-4">
          <span className="inline-flex h-4 w-4 rounded-full border-2 border-yellow-500 border-t-transparent animate-spin"></span>
          <span className="text-sm text-mid-gray">
            {t("meeting.processing", "Processing transcription...")}
          </span>
        </div>
      )}

      {/* Control Buttons */}
      <div className="flex items-center justify-center gap-3">
        {/* Start Recording Button - shown when idle or completed */}
        {(isIdle || isCompleted) && (
          <Button
            variant="primary"
            size="lg"
            onClick={handleStart}
            disabled={isLoading}
            className="flex items-center gap-2 min-w-[160px]"
          >
            <Play size={18} />
            <span>{t("meeting.start", "Start Recording")}</span>
          </Button>
        )}

        {/* Stop Recording Button - shown when recording */}
        {isRecording && (
          <Button
            variant="danger"
            size="lg"
            onClick={handleStop}
            disabled={isLoading}
            className="flex items-center gap-2 min-w-[160px]"
          >
            <Square size={18} />
            <span>{t("meeting.stop", "Stop Recording")}</span>
          </Button>
        )}

        {/* Retry Transcription Button - shown when failed */}
        {isFailed && (
          <div className="flex flex-col items-center gap-3">
            <Button
              variant="secondary"
              size="lg"
              onClick={handleRetry}
              disabled={isLoading}
              className="flex items-center gap-2 min-w-[160px]"
            >
              <RotateCcw size={18} />
              <span>{t("meeting.retry", "Retry Transcription")}</span>
            </Button>
            <Button
              variant="primary"
              size="lg"
              onClick={handleStart}
              disabled={isLoading}
              className="flex items-center gap-2 min-w-[160px]"
            >
              <Play size={18} />
              <span>{t("meeting.newRecording", "New Recording")}</span>
            </Button>
          </div>
        )}

        {/* Disabled state during processing - show waiting message */}
        {isProcessing && (
          <Button
            variant="secondary"
            size="lg"
            disabled={true}
            className="flex items-center gap-2 min-w-[160px] opacity-50"
          >
            <span className="inline-flex h-4 w-4 rounded-full border-2 border-current border-t-transparent animate-spin"></span>
            <span>{t("meeting.processing_short", "Processing...")}</span>
          </Button>
        )}
      </div>

      {/* Duration display after recording stopped (non-recording states) */}
      {!isIdle && !isRecording && recordingDuration > 0 && (
        <div className="flex items-center justify-center text-sm text-mid-gray">
          <span>
            {t("meeting.recordedDuration", "Recorded")}:{" "}
            {formatDuration(recordingDuration)}
          </span>
        </div>
      )}
    </div>
  );
};

export default MeetingControls;
