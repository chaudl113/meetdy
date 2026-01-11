import React, { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { AlertCircle, RotateCcw, X } from "lucide-react";
import { useMeetingStore, formatDuration } from "../../stores/meetingStore";
import { SettingsGroup } from "../ui/SettingsGroup";
import { MeetingControls } from "./MeetingControls";
import { MeetingStatusIndicator } from "./MeetingStatusIndicator";
import { MeetingTitleEditor } from "./MeetingTitleEditor";
import { MeetingTranscriptDisplay } from "./MeetingTranscriptDisplay";

/**
 * MeetingMode - Main container component for Meeting Mode functionality.
 *
 * This component serves as the root container for the meeting recording feature,
 * composing child components and connecting to the meetingStore for state management.
 *
 * Child components:
 * - MeetingControls: Start/Stop buttons and timer display
 * - MeetingStatusIndicator: Visual state indicator (recording/processing/etc)
 * - MeetingTitleEditor: Editable meeting title field
 */
export const MeetingMode: React.FC = () => {
  const { t } = useTranslation();

  // Connect to meetingStore for state
  const {
    sessionStatus,
    currentSession,
    recordingDuration,
    isLoading,
    error,
    initializeEventListeners,
    cleanupEventListeners,
    refreshStatus,
    clearError,
    retryTranscription,
  } = useMeetingStore();

  // Determine error type for specialized display
  const errorInfo = useMemo(() => {
    if (!error && !currentSession?.error_message) return null;

    const errorMessage = error || currentSession?.error_message || "";
    const lowerError = errorMessage.toLowerCase();

    // Detect error types based on common error patterns from backend
    if (
      lowerError.includes("microphone") ||
      lowerError.includes("audio device") ||
      lowerError.includes("no input")
    ) {
      return {
        type: "microphone" as const,
        title: t("meeting.error.noMicrophone.title", "Microphone Error"),
        message: t(
          "meeting.error.noMicrophone.message",
          "Unable to access microphone. Please check your microphone is connected and permissions are granted.",
        ),
        canRetry: false,
        originalMessage: errorMessage,
      };
    }

    if (
      lowerError.includes("model not") ||
      lowerError.includes("model is not loaded") ||
      lowerError.includes("not downloaded")
    ) {
      return {
        type: "model" as const,
        title: t(
          "meeting.error.modelNotLoaded.title",
          "Transcription Model Not Ready",
        ),
        message: t(
          "meeting.error.modelNotLoaded.message",
          "The transcription model is not loaded. Please download a model from Settings, then retry.",
        ),
        canRetry: true,
        originalMessage: errorMessage,
      };
    }

    if (
      lowerError.includes("transcription failed") ||
      lowerError.includes("transcription error") ||
      lowerError.includes("whisper") ||
      lowerError.includes("parakeet")
    ) {
      return {
        type: "transcription" as const,
        title: t(
          "meeting.error.transcriptionFailed.title",
          "Transcription Failed",
        ),
        message: t(
          "meeting.error.transcriptionFailed.message",
          "Failed to transcribe the recording. Your audio has been saved and you can retry.",
        ),
        canRetry: true,
        originalMessage: errorMessage,
      };
    }

    // Generic error fallback
    return {
      type: "generic" as const,
      title: t("meeting.error.generic.title", "Error"),
      message: errorMessage,
      canRetry: sessionStatus === "failed",
      originalMessage: errorMessage,
    };
  }, [error, currentSession?.error_message, sessionStatus, t]);

  // Handle retry button click
  const handleRetry = async () => {
    clearError();
    await retryTranscription();
  };

  // Initialize event listeners on mount and cleanup on unmount
  useEffect(() => {
    // Initialize listeners when component mounts
    useMeetingStore.getState().initializeEventListeners();
    useMeetingStore.getState().refreshStatus();

    // Cleanup when component unmounts
    return () => {
      useMeetingStore.getState().cleanupEventListeners();
    };
  }, []); // Empty deps - only run on mount/unmount

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("meeting.title", "Meeting Mode")}>
        {/* Status and Controls Section */}
        <div className="p-4 space-y-4">
          {/* Session Status Indicator */}
          <div className="flex items-center gap-3">
            <MeetingStatusIndicator
              status={sessionStatus}
              showLabel
              size="sm"
            />
            {sessionStatus === "recording" && (
              <span className="text-sm text-mid-gray font-mono">
                {formatDuration(recordingDuration)}
              </span>
            )}
          </div>

          {/* Current Session Title - Editable */}
          {currentSession && (
            <div className="py-1">
              <MeetingTitleEditor />
            </div>
          )}

          {/* Error Display - Enhanced with specific error types and retry */}
          {errorInfo && (
            <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-4">
              <div className="flex items-start gap-3">
                {/* Error Icon */}
                <AlertCircle className="h-5 w-5 text-red-500 flex-shrink-0 mt-0.5" />

                {/* Error Content */}
                <div className="flex-1 min-w-0">
                  {/* Error Title */}
                  <h4 className="text-sm font-medium text-red-400">
                    {errorInfo.title}
                  </h4>

                  {/* Error Message */}
                  <p className="text-sm text-red-400/80 mt-1">
                    {errorInfo.message}
                  </p>

                  {/* Show original error message for debugging if different */}
                  {errorInfo.type !== "generic" &&
                    errorInfo.originalMessage !== errorInfo.message && (
                      <p className="text-xs text-red-400/60 mt-2 font-mono">
                        {errorInfo.originalMessage}
                      </p>
                    )}

                  {/* Action Buttons */}
                  <div className="flex items-center gap-3 mt-3">
                    {/* Retry Button - only show if retryable and session is in failed state */}
                    {errorInfo.canRetry &&
                      sessionStatus === "failed" &&
                      currentSession && (
                        <button
                          onClick={handleRetry}
                          disabled={isLoading}
                          className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-red-400 hover:text-red-300 bg-red-500/20 hover:bg-red-500/30 rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                          <RotateCcw
                            size={14}
                            className={isLoading ? "animate-spin" : ""}
                          />
                          {t("meeting.error.retry", "Retry")}
                        </button>
                      )}

                    {/* Dismiss Button */}
                    <button
                      onClick={clearError}
                      className="inline-flex items-center gap-1 text-sm text-red-400/70 hover:text-red-400 transition-colors"
                    >
                      <X size={14} />
                      {t("common.dismiss", "Dismiss")}
                    </button>
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Loading Indicator */}
          {isLoading && (
            <div className="flex items-center gap-2 text-sm text-mid-gray">
              <span className="inline-flex h-4 w-4 rounded-full border-2 border-gray-400 border-t-transparent animate-spin"></span>
              <span>{t("common.loading", "Loading...")}</span>
            </div>
          )}

          {/* Meeting Controls - Start/Stop button and timer */}
          <MeetingControls />

          {/* Transcript Display - shown when completed */}
          <MeetingTranscriptDisplay />
        </div>
      </SettingsGroup>
    </div>
  );
};

export default MeetingMode;
