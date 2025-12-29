import React from "react";
import { useTranslation } from "react-i18next";
import { Check, AlertCircle, Loader2 } from "lucide-react";
import type { MeetingStatus } from "@/bindings";

interface MeetingStatusIndicatorProps {
  /** The current meeting session status */
  status: MeetingStatus;
  /** Optional: Show the status label text */
  showLabel?: boolean;
  /** Optional: Size variant for the indicator */
  size?: "sm" | "md" | "lg";
  /** Optional: Additional CSS classes */
  className?: string;
}

/**
 * MeetingStatusIndicator - Visual indicator for meeting session state.
 *
 * Displays different visual indicators based on the current meeting status:
 * - Recording: Red pulsing dot
 * - Processing: Spinning loader
 * - Completed: Green checkmark
 * - Failed: Red error icon
 * - Idle: Gray dot
 */
export const MeetingStatusIndicator: React.FC<MeetingStatusIndicatorProps> = ({
  status,
  showLabel = false,
  size = "md",
  className = "",
}) => {
  const { t } = useTranslation();

  // Size configurations
  const sizeClasses = {
    sm: {
      container: "h-3 w-3",
      icon: 12,
      text: "text-xs",
    },
    md: {
      container: "h-4 w-4",
      icon: 14,
      text: "text-sm",
    },
    lg: {
      container: "h-5 w-5",
      icon: 18,
      text: "text-base",
    },
  };

  const currentSize = sizeClasses[size];

  // Render the appropriate indicator based on status
  const renderIndicator = () => {
    switch (status) {
      case "recording":
        // Red pulsing dot for recording state
        return (
          <span className={`flex ${currentSize.container} relative`}>
            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-red-400 opacity-75"></span>
            <span className="relative inline-flex rounded-full h-full w-full bg-red-500"></span>
          </span>
        );

      case "processing":
        // Spinning loader for processing state
        return (
          <Loader2
            size={currentSize.icon}
            className="animate-spin text-yellow-500"
          />
        );

      case "completed":
        // Green checkmark for completed state
        return (
          <span
            className={`flex items-center justify-center ${currentSize.container} rounded-full bg-green-500`}
          >
            <Check
              size={currentSize.icon - 4}
              className="text-white"
              strokeWidth={3}
            />
          </span>
        );

      case "failed":
        // Red error icon for failed state
        return <AlertCircle size={currentSize.icon} className="text-red-500" />;

      case "idle":
      default:
        // Gray dot for idle state
        return (
          <span
            className={`inline-flex ${currentSize.container} rounded-full bg-gray-400`}
          ></span>
        );
    }
  };

  // Get the status label text
  const getStatusLabel = (): string => {
    switch (status) {
      case "recording":
        return t("meeting.status.recording", "Recording");
      case "processing":
        return t("meeting.status.processing", "Processing");
      case "completed":
        return t("meeting.status.completed", "Completed");
      case "failed":
        return t("meeting.status.failed", "Failed");
      case "idle":
      default:
        return t("meeting.status.idle", "Ready");
    }
  };

  // Get the status color for the label
  const getStatusLabelColor = (): string => {
    switch (status) {
      case "recording":
        return "text-red-500";
      case "processing":
        return "text-yellow-500";
      case "completed":
        return "text-green-500";
      case "failed":
        return "text-red-500";
      case "idle":
      default:
        return "text-gray-400";
    }
  };

  return (
    <div className={`flex items-center gap-2 ${className}`}>
      {renderIndicator()}
      {showLabel && (
        <span
          className={`font-medium ${currentSize.text} ${getStatusLabelColor()}`}
        >
          {getStatusLabel()}
        </span>
      )}
    </div>
  );
};

export default MeetingStatusIndicator;
