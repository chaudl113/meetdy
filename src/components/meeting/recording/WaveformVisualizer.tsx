import React, { useEffect, useRef } from "react";

/**
 * WaveformVisualizer — live audio strip.
 *
 * Renders a horizontal strip of vertical bars. When a `level` (0–1) is
 * provided, the newest bar is set to that level and previous values shift
 * left, producing a scrolling waveform driven by real audio peaks. If no
 * `level` is given, falls back to a random animation so the strip still
 * looks alive (used by demo / placeholder states).
 *
 * Props:
 *  - active: when false, bars stay flat (used when paused / not recording).
 *  - level:  current normalized amplitude in [0, 1]; pushed once per
 *            render. Pass the latest peak from `meeting_audio_stats`.
 *  - barCount: number of bars to render (default 60).
 *  - height: pixel height of the strip.
 */
interface WaveformVisualizerProps {
  active?: boolean;
  level?: number;
  barCount?: number;
  height?: number;
  className?: string;
}

export const WaveformVisualizer: React.FC<WaveformVisualizerProps> = ({
  active = true,
  level,
  barCount = 60,
  height = 60,
  className = "",
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const rafRef = useRef<number | null>(null);
  // Ring buffer of recent levels (0–1) for the scrolling waveform.
  const levelsRef = useRef<number[]>(Array(barCount).fill(0));

  // Push the latest level into the ring buffer whenever it changes.
  useEffect(() => {
    if (level === undefined) return;
    const clamped = Math.max(0, Math.min(1, level));
    const buf = levelsRef.current;
    buf.shift();
    buf.push(active ? clamped : 0);
  }, [level, active]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const hasLiveLevel = level !== undefined;
    let lastUpdate = 0;
    const animate = (now: number) => {
      if (now - lastUpdate > 100) {
        lastUpdate = now;
        const bars = container.children;
        for (let i = 0; i < bars.length; i++) {
          const bar = bars[i] as HTMLElement;
          if (!active) {
            bar.style.height = "8%";
            continue;
          }
          if (hasLiveLevel) {
            // Map 0–1 amplitude to 8–100% bar height for visual contrast.
            const v = levelsRef.current[i] ?? 0;
            bar.style.height = `${8 + v * 92}%`;
          } else {
            const h = 15 + Math.random() * 85;
            bar.style.height = `${h}%`;
          }
        }
      }
      rafRef.current = requestAnimationFrame(animate);
    };
    rafRef.current = requestAnimationFrame(animate);

    return () => {
      if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
    };
  }, [active, level]);

  return (
    <div
      ref={containerRef}
      className={`flex items-end justify-between gap-[2px] w-full ${className}`}
      style={{ height }}
      aria-hidden="true"
    >
      {Array.from({ length: barCount }).map((_, i) => (
        <div
          key={i}
          className="flex-1 bg-gradient-to-t from-logo-primary/40 to-logo-primary rounded-sm transition-all duration-100"
          style={{ height: "8%" }}
        />
      ))}
    </div>
  );
};

export default WaveformVisualizer;
