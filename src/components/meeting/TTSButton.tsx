import React from "react";
import { Volume2, VolumeX, Loader2 } from "lucide-react";
import { useEdgeTTS } from "../../hooks/useEdgeTTS";

interface TTSButtonProps {
  getText: () => string;
  voice?: string;
  className?: string;
  title?: string;
}

export const TTSButton: React.FC<TTSButtonProps> = ({
  getText,
  voice = "vi-VN-HoaiMyNeural",
  className = "",
  title = "Đọc transcript",
}) => {
  const { speak, stop, isPlaying, isLoading } = useEdgeTTS();

  const handleClick = () => {
    if (isPlaying) {
      stop();
    } else {
      speak(getText(), voice);
    }
  };

  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={isLoading}
      title={title}
      className={`flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium rounded-lg transition-colors ${
        isPlaying
          ? "text-logo-primary bg-logo-primary/10 hover:bg-logo-primary/20"
          : "text-text/60 hover:text-text hover:bg-mid-gray/10"
      } disabled:opacity-30 ${className}`}
    >
      {isLoading ? (
        <Loader2 size={14} className="animate-spin" />
      ) : isPlaying ? (
        <VolumeX size={14} />
      ) : (
        <Volume2 size={14} />
      )}
      {isPlaying ? "Dừng" : "Đọc"}
    </button>
  );
};

export default TTSButton;
