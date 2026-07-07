import React from "react";
import ResetIcon from "../icons/ResetIcon";

interface ResetButtonProps {
  onClick: () => void;
  disabled?: boolean;
  className?: string;
  ariaLabel?: string;
  children?: React.ReactNode;
}

export const ResetButton: React.FC<ResetButtonProps> = React.memo(
  ({ onClick, disabled = false, className = "", ariaLabel, children }) => (
    <button
      type="button"
      aria-label={ariaLabel}
      className={`w-8 h-8 flex items-center justify-center rounded-lg border bg-background transition-all duration-150 ${
        disabled
          ? "opacity-50 cursor-not-allowed text-text/40 border-mid-gray/20"
          : "border-mid-gray/30 text-text/70 hover:text-logo-primary hover:border-logo-primary hover:bg-logo-primary/5 active:translate-y-[1px] hover:cursor-pointer"
      } ${className}`}
      onClick={onClick}
      disabled={disabled}
    >
      {children ?? <ResetIcon />}
    </button>
  ),
);
