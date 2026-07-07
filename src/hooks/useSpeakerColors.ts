import { useMemo } from "react";
import type { Participant } from "@/bindings";

/**
 * 8-color palette for speaker attribution.
 * Colors are chosen for sufficient contrast on both light and dark backgrounds.
 * Uses Tailwind v4 utility classes — matches the existing Tag color pattern.
 */
export const SPEAKER_COLORS = [
  {
    dot: "bg-blue-500",
    text: "text-blue-500",
    bg: "bg-blue-500/10",
    border: "border-l-blue-500",
  },
  {
    dot: "bg-amber-500",
    text: "text-amber-500",
    bg: "bg-amber-500/10",
    border: "border-l-amber-500",
  },
  {
    dot: "bg-green-500",
    text: "text-green-500",
    bg: "bg-green-500/10",
    border: "border-l-green-500",
  },
  {
    dot: "bg-violet-500",
    text: "text-violet-500",
    bg: "bg-violet-500/10",
    border: "border-l-violet-500",
  },
  {
    dot: "bg-rose-500",
    text: "text-rose-500",
    bg: "bg-rose-500/10",
    border: "border-l-rose-500",
  },
  {
    dot: "bg-cyan-500",
    text: "text-cyan-500",
    bg: "bg-cyan-500/10",
    border: "border-l-cyan-500",
  },
  {
    dot: "bg-orange-500",
    text: "text-orange-500",
    bg: "bg-orange-500/10",
    border: "border-l-orange-500",
  },
  {
    dot: "bg-pink-500",
    text: "text-pink-500",
    bg: "bg-pink-500/10",
    border: "border-l-pink-500",
  },
] as const;

export type SpeakerColor = {
  dot: string;
  text: string;
  bg: string;
  border: string;
};

export const UNKNOWN_SPEAKER_COLOR: SpeakerColor = {
  dot: "bg-mid-gray/50",
  text: "text-mid-gray",
  bg: "bg-mid-gray/5",
  border: "border-l-mid-gray/40",
};

/**
 * Returns a stable map of participantId → SpeakerColor.
 * Color is auto-assigned by participant position in the array.
 */
export function useSpeakerColors(
  participants: Participant[],
): Record<string, SpeakerColor> {
  return useMemo(() => {
    const map: Record<string, SpeakerColor> = {};
    participants.forEach((p, i) => {
      map[p.id] = SPEAKER_COLORS[i % SPEAKER_COLORS.length];
    });
    return map;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [participants.map((p) => p.id).join(",")]);
}
