import React, { useEffect, useRef } from "react";

/**
 * WaveformVisualizer — Phase 1 placeholder.
 *
 * Renders a horizontal strip of animated vertical bars to mimic a live
 * audio waveform. In Phase 3 this will consume the real
 * `meeting_audio_stats` event stream and draw a canvas-based waveform.
 *
 * Props:
 *  - active: when false, bars stay flat (used when paused / not recording).
 *  - barCount: number of bars to render (default 60).
 *  - height: pixel height of the strip.
 */
interface WaveformVisualizerProps {
  active?: boolean;
  barCount?: number;
  height?: number;
  className?: string;
}

export const WaveformVisualizer: React.FC<WaveformVisualizerProps> = ({
  active = true,
  barCount = 60,
  height = 60,
  className = "",
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const rafRef = useRef<number | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

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
          const h = 15 + Math.random() * 85;
          bar.style.height = `${h}%`;
        }
      }
      rafRef.current = requestAnimationFrame(animate);
    };
    rafRef.current = requestAnimationFrame(animate);

    return () => {
      if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
    };
  }, [active]);

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
