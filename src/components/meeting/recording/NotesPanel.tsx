import React from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { StickyNote, Plus, Trash2 } from "lucide-react";
import { formatDuration, useMeetingStore } from "../../../stores/meetingStore";

interface NotesPanelProps {
  onAddNote?: () => void;
}

/**
 * NotesPanel — full notes list shown in the dedicated Notes tab.
 *
 * Notes are listed chronologically (by recording timestamp). Each row shows
 * the in-recording timestamp, the body and a delete action.
 */
export const NotesPanel: React.FC<NotesPanelProps> = ({ onAddNote }) => {
  const { t } = useTranslation();
  const { notes, deleteNote } = useMeetingStore(
    useShallow((s) => ({ notes: s.notes, deleteNote: s.deleteNote })),
  );

  if (notes.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center text-center py-16 px-6">
        <StickyNote className="text-text/30 mb-3" width={40} height={40} />
        <h3 className="text-base font-semibold mb-1">
          {t("recording.notes.emptyTitle")}
        </h3>
        <p className="text-sm text-text/60 max-w-sm mb-4">
          {t("recording.notes.emptyMessage")}
        </p>
        {onAddNote && (
          <button
            type="button"
            onClick={onAddNote}
            className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-logo-primary text-white hover:bg-logo-primary/90"
          >
            <Plus width={16} height={16} />
            <span className="text-sm font-medium">
              {t("recording.notes.addFirst")}
            </span>
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      {notes.map((note) => (
        <div
          key={note.id}
          className="group flex items-start gap-3 p-3 rounded-lg border border-mid-gray/15 hover:border-mid-gray/30 transition-colors"
        >
          <span className="font-mono text-xs text-logo-primary mt-0.5 shrink-0 px-2 py-0.5 rounded bg-logo-primary/10">
            {formatDuration(note.timestamp_seconds)}
          </span>
          <p className="flex-1 text-sm text-text/85 whitespace-pre-wrap break-words">
            {note.content}
          </p>
          <button
            type="button"
            onClick={() => deleteNote(note.id)}
            className="opacity-0 group-hover:opacity-100 text-text/50 hover:text-red-500 transition-opacity"
            aria-label={t("recording.quickNotes.delete")}
          >
            <Trash2 width={16} height={16} />
          </button>
        </div>
      ))}
    </div>
  );
};

export default NotesPanel;
