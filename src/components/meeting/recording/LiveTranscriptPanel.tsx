import React from "react";
import { useTranslation } from "react-i18next";
import { FileText } from "lucide-react";

/**
 * LiveTranscriptPanel — Phase 1 placeholder.
 *
 * Live transcript streaming is implemented in Phase 4. During Phase 1 this
 * panel shows an empty state with an explanation.
 */
export const LiveTranscriptPanel: React.FC = () => {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col items-center justify-center text-center py-16 px-6">
      <FileText className="text-text/30 mb-3" width={40} height={40} />
      <h3 className="text-base font-semibold mb-1">
        {t("recording.transcript.emptyTitle")}
      </h3>
      <p className="text-sm text-text/60 max-w-sm">
        {t("recording.transcript.emptyMessage")}
      </p>
    </div>
  );
};

export default LiveTranscriptPanel;
