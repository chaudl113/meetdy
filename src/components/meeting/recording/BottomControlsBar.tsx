import React from "react";
import { useTranslation } from "react-i18next";
import {
  Mic,
  Monitor,
  Volume2,
  Settings,
  Square,
  type LucideIcon,
} from "lucide-react";
import { useMeetingStore } from "../../../stores/meetingStore";
import { useRecordingConfigStore } from "../../../stores/recordingConfigStore";

/**
 * BottomControlsBar — sticky bottom bar with the audio source toggles
 * (mic / system / noise cancellation) and the End Meeting action.
 *
 * Phase 1 renders the source states as read-only indicators derived from the
 * configuration captured on the StartMeeting screen. Live toggling and the
 * noise cancellation pipeline are wired in Phase 5.
 */
export const BottomControlsBar: React.FC = () => {
  const { t } = useTranslation();
  const { sessionStatus, isLoading, stopMeeting } = useMeetingStore();
  const { audioSource } = useRecordingConfigStore();

  const micOn = audioSource === "microphone_only" || audioSource === "mixed";
  const systemOn = audioSource === "system_only" || audioSource === "mixed";
  const isRecording = sessionStatus === "recording";

  const handleEnd = async () => {
    await stopMeeting();
  };

  return (
    <div className="flex items-center justify-between gap-3 px-5 py-3 bg-background border border-mid-gray/20 rounded-xl">
      <div className="flex items-center gap-2">
        <SourceChip
          icon={Mic}
          label={t("recording.bottom.microphone")}
          active={micOn}
          statusLabel={
            micOn ? t("recording.bottom.on") : t("recording.bottom.off")
          }
        />
        <SourceChip
          icon={Monitor}
          label={t("recording.bottom.systemAudio")}
          active={systemOn}
          statusLabel={
            systemOn ? t("recording.bottom.on") : t("recording.bottom.off")
          }
        />
        <button
          type="button"
          disabled
          title={t("recording.comingSoon")}
          className="flex items-center gap-1.5 px-3 py-2 rounded-lg border border-mid-gray/20 text-text/50 cursor-not-allowed"
        >
          <Volume2 width={16} height={16} />
          <span className="text-sm">
            {t("recording.bottom.noiseCancellation")}
          </span>
        </button>
      </div>

      <div className="flex items-center gap-2">
        <button
          type="button"
          className="flex items-center justify-center w-9 h-9 rounded-lg border border-mid-gray/20 text-text/70 hover:bg-mid-gray/10"
          aria-label={t("recording.bottom.settings")}
        >
          <Settings width={18} height={18} />
        </button>
        <button
          type="button"
          onClick={handleEnd}
          disabled={isLoading || !isRecording}
          className="flex items-center gap-1.5 px-4 py-2 rounded-lg bg-red-500 text-white hover:bg-red-600 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <Square width={14} height={14} fill="currentColor" />
          <span className="text-sm font-semibold">
            {t("recording.bottom.endMeeting")}
          </span>
        </button>
      </div>
    </div>
  );
};

interface SourceChipProps {
  icon: LucideIcon;
  label: string;
  active: boolean;
  statusLabel: string;
}

const SourceChip: React.FC<SourceChipProps> = ({
  icon: Icon,
  label,
  active,
  statusLabel,
}) => (
  <div
    className={`flex items-center gap-1.5 px-3 py-2 rounded-lg border ${
      active
        ? "border-logo-primary/40 bg-logo-primary/5 text-logo-primary"
        : "border-mid-gray/20 text-text/50"
    }`}
  >
    <Icon width={16} height={16} />
    <span className="text-sm font-medium">{label}</span>
    <span
      className={`text-[10px] font-semibold px-1.5 py-0.5 rounded ${
        active ? "bg-logo-primary/15" : "bg-mid-gray/15"
      }`}
    >
      {statusLabel}
    </span>
  </div>
);

export default BottomControlsBar;
