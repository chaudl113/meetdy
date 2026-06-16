import React from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { Activity } from "lucide-react";
import { WaveformVisualizer } from "./WaveformVisualizer";
import { useMeetingStore } from "../../../stores/meetingStore";

/**
 * Buckets the live SNR (dB) into a coarse quality label used by the
 * existing translation keys. Thresholds are intentionally simple: tune in
 * one place if user feedback suggests different cut-offs.
 */
function snrToQuality(snrDb: number): "good" | "fair" | "poor" {
  if (snrDb >= 20) return "good";
  if (snrDb >= 10) return "fair";
  return "poor";
}

function noiseFloorToLabel(noiseDb: number): "low" | "medium" | "high" {
  // noiseDb is negative; closer to 0 = louder noise floor.
  if (noiseDb <= -50) return "low";
  if (noiseDb <= -35) return "medium";
  return "high";
}

function clarityToLabel(snrDb: number, peak: number): "low" | "medium" | "high" {
  // Require both a decent SNR and a non-clipping peak to call it "high".
  if (snrDb >= 20 && peak < 0.95) return "high";
  if (snrDb >= 10) return "medium";
  return "low";
}

/**
 * AudioStatsCard — live audio quality panel.
 *
 * Subscribes to `meeting_audio_stats` (via the meeting store) and renders
 * the current input level, scrolling waveform, and qualitative labels
 * (quality / noise / clarity) derived from RMS / peak / SNR / noise floor.
 */
export const AudioStatsCard: React.FC = () => {
  const { t } = useTranslation();
  const { sessionStatus, audioStats } = useMeetingStore(
    useShallow((s) => ({
      sessionStatus: s.sessionStatus,
      audioStats: s.audioStats,
    })),
  );
  const isRecording = sessionStatus === "recording";

  const peak = audioStats?.peak ?? 0;
  const rms = audioStats?.rms ?? 0;
  const snrDb = audioStats?.snr_db ?? 0;
  const noiseDb = audioStats?.noise_floor_db ?? -60;

  // Use RMS for the input-level meter (perceived loudness) and peak for the
  // waveform (instantaneous amplitude). Both are already normalized 0–1.
  const inputPct = Math.round(Math.max(0, Math.min(1, rms)) * 100);

  const qualityKey = audioStats ? snrToQuality(snrDb) : "good";
  const noiseKey = audioStats ? noiseFloorToLabel(noiseDb) : "low";
  const clarityKey = audioStats ? clarityToLabel(snrDb, peak) : "high";

  const qualityColor =
    qualityKey === "good"
      ? "text-green-500"
      : qualityKey === "fair"
        ? "text-yellow-500"
        : "text-red-500";

  return (
    <div className="bg-background border border-mid-gray/20 rounded-xl p-5">
      <div className="flex items-center gap-2 mb-4">
        <Activity width={16} height={16} className="text-logo-primary" />
        <span className="text-sm font-semibold">
          {t("recording.audioStats.title")}
        </span>
      </div>

      <WaveformVisualizer
        active={isRecording}
        level={isRecording ? peak : 0}
        height={64}
        className="mb-4"
      />

      <div className="mb-4">
        <div className="flex items-center justify-between mb-1">
          <span className="text-xs text-text/60">
            {t("recording.audioStats.inputLevel")}
          </span>
          <span className="text-xs font-mono text-text/80">{`${inputPct}%`}</span>
        </div>
        <div className="h-2 bg-mid-gray/15 rounded-full overflow-hidden">
          <div
            className="h-full bg-gradient-to-r from-green-500 via-green-400 to-yellow-400 rounded-full transition-all"
            style={{ width: `${inputPct}%` }}
          />
        </div>
      </div>

      <div className="grid grid-cols-3 gap-3 text-center">
        <div>
          <div className="text-xs text-text/60 mb-0.5">
            {t("recording.audioStats.quality")}
          </div>
          <div className={`text-sm font-semibold ${qualityColor}`}>
            {t(`recording.audioStats.qualityValue.${qualityKey}`)}
          </div>
        </div>
        <div>
          <div className="text-xs text-text/60 mb-0.5">
            {t("recording.audioStats.noise")}
          </div>
          <div className="text-sm font-semibold text-text/80">
            {t(`recording.audioStats.noiseValue.${noiseKey}`)}
          </div>
        </div>
        <div>
          <div className="text-xs text-text/60 mb-0.5">
            {t("recording.audioStats.clarity")}
          </div>
          <div className="text-sm font-semibold text-text/80">
            {t(`recording.audioStats.clarityValue.${clarityKey}`)}
          </div>
        </div>
      </div>
    </div>
  );
};

export default AudioStatsCard;
