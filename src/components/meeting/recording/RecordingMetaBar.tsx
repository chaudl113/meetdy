import React from "react";
import { useTranslation } from "react-i18next";
import { Clock, Languages, Gauge } from "lucide-react";
import { formatDuration, useMeetingStore } from "../../../stores/meetingStore";
import { useRecordingConfigStore } from "../../../stores/recordingConfigStore";
import { useSettings } from "../../../hooks/useSettings";
import { LANGUAGES } from "../../../lib/constants/languages";
import { MeetingTitleEditor } from "../MeetingTitleEditor";

/**
 * RecordingMetaBar — secondary header row with editable title and meta info
 * (duration, language, quality).
 */
export const RecordingMetaBar: React.FC = () => {
  const { t } = useTranslation();
  const { recordingDuration, currentSession } = useMeetingStore();
  const { recordingQuality } = useRecordingConfigStore();
  const { getSetting } = useSettings();

  const languageValue = getSetting("selected_language") || "auto";
  const languageLabel =
    languageValue === "auto"
      ? t("recording.meta.autoLanguage")
      : LANGUAGES.find((l) => l.value === languageValue)?.label ?? languageValue;

  const qualityLabel = t(`recording.quality.${recordingQuality}`);

  return (
    <div className="flex items-center justify-between gap-4 px-5 py-3 bg-background border border-mid-gray/20 rounded-xl">
      <div className="min-w-0 flex-1">
        {currentSession ? (
          <MeetingTitleEditor />
        ) : (
          <span className="text-sm text-text/50">
            {t("recording.meta.untitled")}
          </span>
        )}
      </div>
      <div className="flex items-center gap-4 shrink-0">
        <div className="flex items-center gap-1.5 text-sm text-text/70">
          <Clock width={14} height={14} />
          <span className="font-mono tabular-nums">
            {formatDuration(recordingDuration)}
          </span>
        </div>
        <div className="flex items-center gap-1.5 text-sm text-text/70">
          <Languages width={14} height={14} />
          <span>{languageLabel}</span>
        </div>
        <div className="flex items-center gap-1.5 text-sm text-text/70">
          <Gauge width={14} height={14} />
          <span>{qualityLabel}</span>
        </div>
      </div>
    </div>
  );
};

export default RecordingMetaBar;
