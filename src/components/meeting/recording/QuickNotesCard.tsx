import React from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { StickyNote, Plus, Trash2 } from "lucide-react";
import { formatDuration, useMeetingStore } from "../../../stores/meetingStore";

interface QuickNotesCardProps {
  onAddNote?: () => void;
}

/**
 * QuickNotesCard — compact list of notes shown in the right column of
 * RecordingView. Displays the 5 most recent notes; full list lives in
 * the dedicated Notes tab.
 */
export const QuickNotesCard: React.FC<QuickNotesCardProps> = ({ onAddNote }) => {
  const { t } = useTranslation();
  const { notes, deleteNote } = useMeetingStore(
    useShallow((s) => ({ notes: s.notes, deleteNote: s.deleteNote })),
  );

  // Show the latest 5 notes, newest first.
  const recent = [...notes].slice(-5).reverse();

  return (
    <div className="bg-background border border-mid-gray/20 rounded-xl p-5">
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <StickyNote width={16} height={16} className="text-logo-primary" />
          <span className="text-sm font-semibold">
            {t("recording.quickNotes.title")}
          </span>
          {notes.length > 0 && (
            <span className="text-xs text-text/50">({notes.length})</span>
          )}
        </div>
        <button
          type="button"
          onClick={onAddNote}
          disabled={!onAddNote}
          className="flex items-center gap-1 px-2 py-1 rounded-md text-xs text-logo-primary hover:bg-logo-primary/10 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <Plus width={14} height={14} />
          <span>{t("recording.quickNotes.add")}</span>
        </button>
      </div>

      {recent.length === 0 ? (
        <div className="text-center py-6 text-xs text-text/50">
          {t("recording.quickNotes.empty")}
        </div>
      ) : (
        <ul className="flex flex-col gap-2">
          {recent.map((note) => (
            <li
              key={note.id}
              className="group flex items-start gap-2 p-2 rounded-md hover:bg-mid-gray/5"
            >
              <span className="font-mono text-[11px] text-logo-primary mt-0.5 shrink-0">
                {formatDuration(note.timestamp_seconds)}
              </span>
              <span className="flex-1 text-sm text-text/80 whitespace-pre-wrap break-words">
                {note.content}
              </span>
              <button
                type="button"
                onClick={() => deleteNote(note.id)}
                className="opacity-0 group-hover:opacity-100 text-text/50 hover:text-red-500 transition-opacity"
                aria-label={t("recording.quickNotes.delete")}
              >
                <Trash2 width={14} height={14} />
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};

export default QuickNotesCard;
