import React from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { Info } from "lucide-react";
import { formatDuration, useMeetingStore } from "../../../stores/meetingStore";
import { useRecordingConfigStore } from "../../../stores/recordingConfigStore";
import { useSettings } from "../../../hooks/useSettings";

/**
 * Formats a byte count as a human readable string (KB / MB / GB).
 */
function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

/**
 * Extracts the file name from a full path. Works with both POSIX and Windows
 * separators.
 */
function basename(path: string | null | undefined): string {
  if (!path) return "";
  const idx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return idx >= 0 ? path.slice(idx + 1) : path;
}

/**
 * RecordingInfoCard — shows recording file metadata.
 *
 * Phase 1: estimates file size from duration (16 kHz × 2 bytes × 1 channel).
 * Phase 3 will replace the estimate with `get_meeting_file_size`.
 */
export const RecordingInfoCard: React.FC = () => {
  const { t } = useTranslation();
  const { currentSession, recordingDuration } = useMeetingStore(
    useShallow((s) => ({
      currentSession: s.currentSession,
      recordingDuration: s.recordingDuration,
    })),
  );
  const { saveLocation, autoTranscribe, autoSummary } = useRecordingConfigStore();
  const { getSetting } = useSettings();
  const isFunasr = (getSetting("meeting_stt_engine") ?? "whisper") === "funasr";

  // WAV PCM 16-bit mono @ 16 kHz ≈ 32 000 bytes/sec.
  const estimatedSize = recordingDuration * 32_000;
  const fileName =
    basename(currentSession?.audio_path) ||
    t("recording.info.fileNamePending");

  return (
    <div className="bg-background border border-mid-gray/20 rounded-xl p-5">
      <div className="flex items-center gap-2 mb-4">
        <Info width={16} height={16} className="text-logo-primary" />
        <span className="text-sm font-semibold">
          {t("recording.info.title")}
        </span>
      </div>

      <dl className="flex flex-col gap-2.5 text-sm">
        <div className="flex items-start justify-between gap-3">
          <dt className="text-text/60 shrink-0">
            {t("recording.info.fileName")}
          </dt>
          <dd
            className="font-mono text-xs text-right break-all"
            title={currentSession?.audio_path ?? undefined}
          >
            {fileName}
          </dd>
        </div>
        <div className="flex items-start justify-between gap-3">
          <dt className="text-text/60 shrink-0">
            {t("recording.info.saveLocation")}
          </dt>
          <dd
            className="font-mono text-xs text-right break-all"
            title={saveLocation}
          >
            {saveLocation}
          </dd>
        </div>
        <div className="flex items-center justify-between gap-3">
          <dt className="text-text/60">{t("recording.info.fileSize")}</dt>
          <dd className="font-mono text-xs">{formatBytes(estimatedSize)}</dd>
        </div>
        <div className="flex items-center justify-between gap-3">
          <dt className="text-text/60">{t("recording.info.duration")}</dt>
          <dd className="font-mono text-xs">
            {formatDuration(recordingDuration)}
          </dd>
        </div>
        <div className="flex items-center justify-between gap-3">
          <dt className="text-text/60">
            {t("recording.info.autoTranscribe")}
          </dt>
          <dd
            className={`text-xs font-semibold ${
              autoTranscribe || isFunasr ? "text-green-500" : "text-text/50"
            }`}
          >
            {isFunasr
              ? t("recording.info.liveChunks", "Live chunks")
              : autoTranscribe
                ? t("recording.info.on")
                : t("recording.info.off")}
          </dd>
        </div>
        <div className="flex items-center justify-between gap-3">
          <dt className="text-text/60">{t("recording.info.autoSummary")}</dt>
          <dd
            className={`text-xs font-semibold ${
              autoSummary ? "text-green-500" : "text-text/50"
            }`}
          >
            {autoSummary ? t("recording.info.on") : t("recording.info.off")}
          </dd>
        </div>
      </dl>
    </div>
  );
};

export default RecordingInfoCard;
