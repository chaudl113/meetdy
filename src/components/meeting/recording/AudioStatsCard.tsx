import React from "react";
import { useTranslation } from "react-i18next";
import { Activity } from "lucide-react";
import { WaveformVisualizer } from "./WaveformVisualizer";
import { useMeetingStore } from "../../../stores/meetingStore";

/**
 * AudioStatsCard — Phase 1 static placeholder.
 *
 * Real values (RMS / peak / SNR / noise floor) are wired in Phase 3.
 * For now we display a fixed "Good / Low / High" combo and an animated
 * waveform strip so the layout matches the design.
 */
export const AudioStatsCard: React.FC = () => {
  const { t } = useTranslation();
  const { sessionStatus } = useMeetingStore();
  const isRecording = sessionStatus === "recording";

  return (
    <div className="bg-background border border-mid-gray/20 rounded-xl p-5">
      <div className="flex items-center gap-2 mb-4">
        <Activity width={16} height={16} className="text-logo-primary" />
        <span className="text-sm font-semibold">
          {t("recording.audioStats.title")}
        </span>
      </div>

      <WaveformVisualizer active={isRecording} height={64} className="mb-4" />

      <div className="mb-4">
        <div className="flex items-center justify-between mb-1">
          <span className="text-xs text-text/60">
            {t("recording.audioStats.inputLevel")}
          </span>
          <span className="text-xs font-mono text-text/80">{"68%"}</span>
        </div>
        <div className="h-2 bg-mid-gray/15 rounded-full overflow-hidden">
          <div
            className="h-full bg-gradient-to-r from-green-500 via-green-400 to-yellow-400 rounded-full transition-all"
            style={{ width: "68%" }}
          />
        </div>
      </div>

      <div className="grid grid-cols-3 gap-3 text-center">
        <div>
          <div className="text-xs text-text/60 mb-0.5">
            {t("recording.audioStats.quality")}
          </div>
          <div className="text-sm font-semibold text-green-500">
            {t("recording.audioStats.qualityValue.good")}
          </div>
        </div>
        <div>
          <div className="text-xs text-text/60 mb-0.5">
            {t("recording.audioStats.noise")}
          </div>
          <div className="text-sm font-semibold text-text/80">
            {t("recording.audioStats.noiseValue.low")}
          </div>
        </div>
        <div>
          <div className="text-xs text-text/60 mb-0.5">
            {t("recording.audioStats.clarity")}
          </div>
          <div className="text-sm font-semibold text-text/80">
            {t("recording.audioStats.clarityValue.high")}
          </div>
        </div>
      </div>
    </div>
  );
};

export default AudioStatsCard;
