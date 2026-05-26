import React from "react";
import { useTranslation } from "react-i18next";
import { Sparkles } from "lucide-react";

/**
 * AISummaryPanel — Phase 1 placeholder.
 *
 * Live AI summary is implemented in Phase 6. During recording this panel
 * shows an empty state.
 */
export const AISummaryPanel: React.FC = () => {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col items-center justify-center text-center py-16 px-6">
      <Sparkles className="text-text/30 mb-3" width={40} height={40} />
      <h3 className="text-base font-semibold mb-1">
        {t("recording.summary.emptyTitle")}
      </h3>
      <p className="text-sm text-text/60 max-w-sm">
        {t("recording.summary.emptyMessage")}
      </p>
    </div>
  );
};

export default AISummaryPanel;
