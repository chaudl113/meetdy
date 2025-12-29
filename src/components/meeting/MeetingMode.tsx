import React, { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useMeetingStore, formatDuration } from "../../stores/meetingStore";
import { SettingsGroup } from "../ui/SettingsGroup";
import { MeetingControls } from "./MeetingControls";
import { MeetingStatusIndicator } from "./MeetingStatusIndicator";

/**
 * MeetingMode - Main container component for Meeting Mode functionality.
 *
 * This component serves as the root container for the meeting recording feature,
 * composing child components and connecting to the meetingStore for state management.
 *
 * Child components:
 * - MeetingControls: Start/Stop buttons and timer display
 * - MeetingStatusIndicator: Visual state indicator (recording/processing/etc)
 * - MeetingTitleEditor: Editable meeting title field (to be implemented)
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
  } = useMeetingStore();

  // Initialize event listeners on mount and cleanup on unmount
  useEffect(() => {
    initializeEventListeners();
    refreshStatus();

    return () => {
      cleanupEventListeners();
    };
  }, [initializeEventListeners, cleanupEventListeners, refreshStatus]);

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("meeting.title", "Meeting Mode")}>
        {/* Status and Controls Section */}
        <div className="p-4 space-y-4">
          {/* Session Status Indicator */}
          <div className="flex items-center gap-3">
            <MeetingStatusIndicator status={sessionStatus} showLabel size="sm" />
            {sessionStatus === "recording" && (
              <span className="text-sm text-mid-gray font-mono">
                {formatDuration(recordingDuration)}
              </span>
            )}
          </div>

          {/* Current Session Info */}
          {currentSession && (
            <div className="text-sm text-mid-gray">
              <p>{currentSession.title}</p>
            </div>
          )}

          {/* Error Display */}
          {error && (
            <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3">
              <div className="flex items-start justify-between gap-2">
                <p className="text-sm text-red-400">{error}</p>
                <button
                  onClick={clearError}
                  className="text-red-400 hover:text-red-300 text-xs"
                >
                  {t("common.dismiss", "Dismiss")}
                </button>
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
        </div>
      </SettingsGroup>
    </div>
  );
};

export default MeetingMode;
