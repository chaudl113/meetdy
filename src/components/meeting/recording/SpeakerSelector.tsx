import React, { useState, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Plus, X, UserRound } from "lucide-react";
import type { Participant } from "@/bindings";
import { commands } from "@/bindings";
import {
  useSpeakerColors,
  UNKNOWN_SPEAKER_COLOR,
} from "../../../hooks/useSpeakerColors";

interface SpeakerSelectorProps {
  sessionId: string;
  participants: Participant[];
  activeSpeakerId: string | null;
  onActiveSpeakerChange: (participantId: string | null) => void;
  onParticipantAdded: (participant: Participant) => void;
}

export const SpeakerSelector: React.FC<SpeakerSelectorProps> = ({
  sessionId,
  participants,
  activeSpeakerId,
  onActiveSpeakerChange,
  onParticipantAdded,
}) => {
  const { t } = useTranslation();
  const speakerColors = useSpeakerColors(participants);
  const [isAdding, setIsAdding] = useState(false);
  const [newName, setNewName] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isAdding) {
      inputRef.current?.focus();
    }
  }, [isAdding]);

  const handleChipClick = async (participantId: string) => {
    const next = activeSpeakerId === participantId ? null : participantId;
    onActiveSpeakerChange(next);
    try {
      await commands.setActiveSpeaker(next);
    } catch (err) {
      console.warn("setActiveSpeaker failed:", err);
    }
  };

  const handleAddSubmit = async () => {
    const name = newName.trim();
    if (!name || isLoading) return;
    setIsLoading(true);
    try {
      const result = await commands.addMeetingParticipant(sessionId, name, null);
      if (result.status === "ok") {
        onParticipantAdded(result.data);
        setNewName("");
        setIsAdding(false);
        // Auto-select newly added participant as active speaker
        onActiveSpeakerChange(result.data.id);
        await commands.setActiveSpeaker(result.data.id);
      }
    } catch (err) {
      console.warn("addMeetingParticipant failed:", err);
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") handleAddSubmit();
    if (e.key === "Escape") {
      setIsAdding(false);
      setNewName("");
    }
  };

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <span className="flex items-center gap-1 text-[11px] text-text/40 shrink-0">
        <UserRound width={11} height={11} />
        {t("recording.speaker.label", "Speaker")}:
      </span>

      {/* Participant chips */}
      {participants.map((p) => {
        const color = speakerColors[p.id] ?? UNKNOWN_SPEAKER_COLOR;
        const isActive = activeSpeakerId === p.id;
        return (
          <button
            key={p.id}
            type="button"
            onClick={() => handleChipClick(p.id)}
            className={`flex items-center gap-1 px-2 py-0.5 rounded-full text-[12px] font-medium transition-all border ${
              isActive
                ? `${color.bg} ${color.text} border-transparent ring-1 ring-current`
                : "bg-transparent text-text/50 border-mid-gray/20 hover:border-mid-gray/40"
            }`}
          >
            <span
              className={`w-1.5 h-1.5 rounded-full shrink-0 ${isActive ? color.dot : "bg-mid-gray/40"}`}
            />
            {p.name}
          </button>
        );
      })}

      {/* Inline add input */}
      {isAdding ? (
        <div className="flex items-center gap-1">
          <input
            ref={inputRef}
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t("recording.speaker.namePlaceholder", "Name…")}
            disabled={isLoading}
            className="w-24 px-2 py-0.5 text-[12px] rounded-full border border-mid-gray/30 bg-background focus:outline-none focus:ring-1 focus:ring-logo-primary/50"
          />
          <button
            type="button"
            onClick={handleAddSubmit}
            disabled={!newName.trim() || isLoading}
            className="text-[11px] text-logo-primary hover:opacity-70 disabled:opacity-30"
          >
            {t("common.add", "Add")}
          </button>
          <button
            type="button"
            onClick={() => { setIsAdding(false); setNewName(""); }}
            className="text-text/30 hover:text-text/60"
          >
            <X width={12} height={12} />
          </button>
        </div>
      ) : (
        <button
          type="button"
          onClick={() => setIsAdding(true)}
          className="flex items-center gap-0.5 px-2 py-0.5 rounded-full text-[12px] text-text/40 border border-dashed border-mid-gray/30 hover:border-mid-gray/50 hover:text-text/60 transition-colors"
        >
          <Plus width={11} height={11} />
          {t("recording.speaker.add", "Add")}
        </button>
      )}
    </div>
  );
};

export default SpeakerSelector;
