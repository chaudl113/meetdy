import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import { formatDuration } from "../../../stores/meetingStore";

interface AddNoteModalProps {
  open: boolean;
  /** Timestamp (in seconds) captured at the moment the modal was opened. */
  timestampSeconds: number;
  onClose: () => void;
  onSubmit: (content: string) => void | Promise<void>;
}

/**
 * AddNoteModal — minimal modal to capture a quick textual note.
 *
 * The timestamp is supplied by the caller (typically the live recording
 * duration at the moment the user clicked "Add Note") so the note pins to
 * the exact moment regardless of how long the modal stays open.
 */
export const AddNoteModal: React.FC<AddNoteModalProps> = ({
  open,
  timestampSeconds,
  onClose,
  onSubmit,
}) => {
  const { t } = useTranslation();
  const [content, setContent] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (open) {
      setContent("");
      // Focus right after the modal renders.
      requestAnimationFrame(() => textareaRef.current?.focus());
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  const handleSubmit = async () => {
    const trimmed = content.trim();
    if (!trimmed || submitting) return;
    setSubmitting(true);
    try {
      await onSubmit(trimmed);
      onClose();
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onClick={onClose}
    >
      <div
        className="bg-background border border-mid-gray/20 rounded-xl p-5 w-full max-w-md mx-4 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between mb-3">
          <div>
            <h3 className="text-base font-semibold">
              {t("recording.addNoteModal.title")}
            </h3>
            <p className="text-xs text-text/60 mt-0.5">
              {t("recording.addNoteModal.atTime")}{" "}
              <span className="font-mono">{formatDuration(timestampSeconds)}</span>
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="w-8 h-8 rounded-md hover:bg-mid-gray/10 flex items-center justify-center text-text/70"
            aria-label={t("recording.addNoteModal.close")}
          >
            <X width={16} height={16} />
          </button>
        </div>

        <textarea
          ref={textareaRef}
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder={t("recording.addNoteModal.placeholder")}
          rows={4}
          className="w-full bg-background border border-mid-gray/30 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-logo-primary resize-none"
          onKeyDown={(e) => {
            if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
              e.preventDefault();
              handleSubmit();
            }
          }}
        />

        <div className="flex items-center justify-between mt-4">
          <span className="text-xs text-text/50">
            {t("recording.addNoteModal.hint")}
          </span>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={onClose}
              className="px-3 py-1.5 text-sm rounded-md border border-mid-gray/30 hover:bg-mid-gray/10"
            >
              {t("recording.addNoteModal.cancel")}
            </button>
            <button
              type="button"
              onClick={handleSubmit}
              disabled={!content.trim() || submitting}
              className="px-3 py-1.5 text-sm rounded-md bg-logo-primary text-white hover:bg-logo-primary/90 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {t("recording.addNoteModal.save")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default AddNoteModal;
