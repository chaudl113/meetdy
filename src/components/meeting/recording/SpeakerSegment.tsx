import React from "react";
import type { SpeakerColor } from "../../../hooks/useSpeakerColors";
import { UNKNOWN_SPEAKER_COLOR } from "../../../hooks/useSpeakerColors";

const formatMs = (ms: number): string => {
  const total = Math.floor(ms / 1000);
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
};

interface SpeakerSegmentProps {
  text: string;
  startMs: number;
  speakerName?: string | null;
  color?: SpeakerColor;
  showSpeakerLabel: boolean; // false nếu cùng speaker với segment trước
  active?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}

export const SpeakerSegment: React.FC<SpeakerSegmentProps> = ({
  text,
  startMs,
  speakerName,
  color = UNKNOWN_SPEAKER_COLOR,
  showSpeakerLabel,
  active = false,
  disabled = false,
  onClick,
}) => {
  return (
    <button
      type="button"
      disabled={disabled || !onClick}
      onClick={onClick}
      className={`block w-full text-left pl-3 border-l-2 rounded-r-md transition-colors ${
        active ? "bg-logo-primary/10" : "bg-transparent"
      } ${disabled || !onClick ? "cursor-default" : "cursor-pointer hover:bg-mid-gray/10"} ${color.border} ${showSpeakerLabel ? "mt-3" : "mt-1"}`}
    >
      {showSpeakerLabel && (
        <div className="flex items-center gap-1.5 mb-0.5">
          <span className={`inline-block w-2 h-2 rounded-full shrink-0 ${color.dot}`} />
          <span className={`text-[12px] font-semibold ${color.text}`}>
            {speakerName ?? "Unknown"}
          </span>
          <span className="text-[11px] font-mono text-text/30 ml-1">
            {formatMs(startMs)}
          </span>
        </div>
      )}
      <p className="text-[15px] leading-7 text-text whitespace-pre-wrap">
        {text}
      </p>
    </button>
  );
};

export default SpeakerSegment;
