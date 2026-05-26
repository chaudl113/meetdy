import React from "react";
import { useTranslation } from "react-i18next";
import { FolderOpen } from "lucide-react";
import { useRecordingConfigStore } from "../../../stores/recordingConfigStore";

/**
 * RecordingFooter — small caption shown under the bottom controls explaining
 * where the recording will be saved.
 */
export const RecordingFooter: React.FC = () => {
  const { t } = useTranslation();
  const { saveLocation } = useRecordingConfigStore();
  return (
    <div className="flex items-center justify-center gap-1.5 text-xs text-text/50">
      <FolderOpen width={12} height={12} />
      <span>
        {t("recording.footer.saveTo")} <span className="font-mono">{saveLocation}</span>
      </span>
    </div>
  );
};

export default RecordingFooter;
