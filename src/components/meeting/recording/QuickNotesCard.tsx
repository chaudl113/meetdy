import React from "react";
import { useTranslation } from "react-i18next";
import { StickyNote, Plus } from "lucide-react";

interface QuickNotesCardProps {
  onAddNote?: () => void;
}

/**
 * QuickNotesCard — Phase 1 placeholder for quick notes.
 *
 * Actual note CRUD is implemented in Phase 2.
 */
export const QuickNotesCard: React.FC<QuickNotesCardProps> = ({ onAddNote }) => {
  const { t } = useTranslation();
  return (
    <div className="bg-background border border-mid-gray/20 rounded-xl p-5">
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <StickyNote width={16} height={16} className="text-logo-primary" />
          <span className="text-sm font-semibold">
            {t("recording.quickNotes.title")}
          </span>
        </div>
        <button
          type="button"
          onClick={onAddNote}
          disabled={!onAddNote}
          title={onAddNote ? undefined : t("recording.comingSoon")}
          className="flex items-center gap-1 px-2 py-1 rounded-md text-xs text-logo-primary hover:bg-logo-primary/10 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <Plus width={14} height={14} />
          <span>{t("recording.quickNotes.add")}</span>
        </button>
      </div>
      <div className="text-center py-6 text-xs text-text/50">
        {t("recording.quickNotes.empty")}
      </div>
    </div>
  );
};

export default QuickNotesCard;
