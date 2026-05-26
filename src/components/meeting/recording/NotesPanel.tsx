import React from "react";
import { useTranslation } from "react-i18next";
import { StickyNote } from "lucide-react";

/**
 * NotesPanel — Phase 1 placeholder.
 *
 * Note CRUD is implemented in Phase 2.
 */
export const NotesPanel: React.FC = () => {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col items-center justify-center text-center py-16 px-6">
      <StickyNote className="text-text/30 mb-3" width={40} height={40} />
      <h3 className="text-base font-semibold mb-1">
        {t("recording.notes.emptyTitle")}
      </h3>
      <p className="text-sm text-text/60 max-w-sm">
        {t("recording.notes.emptyMessage")}
      </p>
    </div>
  );
};

export default NotesPanel;
